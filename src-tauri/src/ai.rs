// PILLVYN AI Assistant backend (v4.0.0 — GapGPT Gemma 4 26B + Full Dynamic Island Agent):
// - Built-in GapGPT API key out-of-the-box for all users + optional Windows DPAPI custom key override
// - Real-time SSE token & tool-call streaming from GapGPT (`gapgpt-gemma-26b-a4b`, `reasoning_effort=none`)
// - 32 structured tools covering full Windows automation + 100% of PILLVYN Dynamic Island features
// - Hybrid bilingual Persian & English STT: local hot whisper.cpp (Small-Q5 / Base) + instant GapGPT Cloud fallback
// - Localhost DSP asset server for RNNoise WASM + ONNX Runtime Web + Silero VAD ONNX
// - Spelled-letter + phonetic + Levenshtein bilingual wake-word engine ("Hello PILLVYN" / "سلام پیلوین")
// - Broadcast-clean streaming Edge Neural TTS (Persian & Multilingual English MP3)
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::os::windows::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering::SeqCst};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use serde_json::{json, Value};

use crate::{ai_tools, audio, hit, net_http, settings, shelf, store, sys};

const CREATE_NO_WINDOW: u32 = 0x0800_0000;
const GAPGPT_CHAT_URL: &str = "https://api.gapgpt.ir/v1/chat/completions";
const GAPGPT_STT_URL: &str = "https://api.gapgpt.ir/v1/audio/transcriptions";
pub const DEFAULT_MODEL: &str = "gapgpt-gemma-26b-a4b";
const MAX_HISTORY: usize = 24;

// Built-in shared PILLVYN v4 GapGPT key (runtime-unmasked; overrideable via PILLVYN_GAPGPT_KEY or Settings -> AI)
const BUILTIN_KEY_ENC: &[u8] = &[
    0x29, 0x0c, 0x59, 0xd6, 0xfa, 0xc3, 0xcf, 0xc3, 0x93, 0xa2, 0xee, 0x99, 0xa7, 0x6c, 0x77, 0x6f,
    0x18, 0x47, 0x77, 0x09, 0x2f, 0x2f, 0x1d, 0xea, 0xe5, 0xf4, 0xd6, 0xfa, 0xf5, 0xe2, 0xb9, 0x83,
    0xcf, 0x75, 0x72, 0x53, 0x4c, 0x4e, 0x02, 0x61, 0x56, 0x24, 0x4b, 0xc2, 0xfa, 0xf7, 0xf8, 0xf5,
    0xfb, 0xa5, 0xa9,
];

const URL_RNNOISE_JS: &str =
    "https://cdn.jsdelivr.net/npm/@jitsi/rnnoise-wasm@0.2.1/dist/rnnoise.js";
const URL_RNNOISE_WASM: &str =
    "https://cdn.jsdelivr.net/npm/@jitsi/rnnoise-wasm@0.2.1/dist/rnnoise.wasm";
const URL_ORT_JS: &str = "https://cdn.jsdelivr.net/npm/onnxruntime-web@1.20.1/dist/ort.wasm.min.js";
const URL_ORT_MJS: &str =
    "https://cdn.jsdelivr.net/npm/onnxruntime-web@1.20.1/dist/ort-wasm-simd-threaded.mjs";
const URL_ORT_WASM: &str =
    "https://cdn.jsdelivr.net/npm/onnxruntime-web@1.20.1/dist/ort-wasm-simd-threaded.wasm";
const URL_SILERO_ONNX: &str =
    "https://raw.githubusercontent.com/snakers4/silero-vad/master/src/silero_vad/data/silero_vad.onnx";
const URL_WHISPER_ZIP: &str =
    "https://github.com/ggml-org/whisper.cpp/releases/download/b5907/whisper-bin-x64.zip";

const MODEL_BASE_URL: &str =
    "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-base.bin";
const MODEL_SMALL_URL: &str =
    "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-small-q5_1.bin";

static ASSET_PORT: OnceLock<u16> = OnceLock::new();
static INSTALLING: AtomicBool = AtomicBool::new(false);
static HISTORY: Mutex<Vec<Value>> = Mutex::new(Vec::new());

struct WhisperProc {
    child: Child,
    port: u16,
    model_path: PathBuf,
}
static WHISPER: Mutex<Option<WhisperProc>> = Mutex::new(None);

// ---------------------------------------------------------------- paths & DPAPI key
pub fn ai_root() -> PathBuf {
    let base = std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(sys::data_dir);
    let dir = base.join("PILLVYN").join("ai");
    let _ = std::fs::create_dir_all(dir.join("dsp"));
    let _ = std::fs::create_dir_all(dir.join("whisper"));
    let _ = std::fs::create_dir_all(dir.join("models"));
    dir
}

fn key_file() -> PathBuf {
    sys::data_dir().join("ai_key.bin")
}

pub fn set_api_key(raw: &str) -> bool {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        let _ = std::fs::remove_file(key_file());
        settings::broadcast();
        return true;
    }
    let Some(enc) = store::dpapi_protect(trimmed.as_bytes()) else {
        return false;
    };
    let tmp = key_file().with_extension("tmp");
    let ok = std::fs::write(&tmp, enc).is_ok() && std::fs::rename(&tmp, key_file()).is_ok();
    if ok {
        settings::broadcast();
    }
    ok
}

fn default_api_key() -> String {
    if let Some(baked) = option_env!("PILLVYN_GAPGPT_KEY") {
        let t = baked.trim();
        if !t.is_empty() {
            return t.to_string();
        }
    }
    let bytes: Vec<u8> = BUILTIN_KEY_ENC
        .iter()
        .enumerate()
        .map(|(i, &b)| b ^ ((0x5Au16 + (i as u16) * 13) as u8))
        .collect();
    String::from_utf8(bytes).unwrap_or_default()
}

fn custom_api_key() -> Option<String> {
    if let Ok(env_k) =
        std::env::var("PILLVYN_GAPGPT_KEY").or_else(|_| std::env::var("GAPGPT_API_KEY"))
    {
        let t = env_k.trim();
        if !t.is_empty() {
            return Some(t.to_string());
        }
    }
    let enc = std::fs::read(key_file()).ok()?;
    let plain = store::dpapi_unprotect(&enc)?;
    let text = String::from_utf8(plain).ok()?;
    let t = text.trim();
    // Ignore legacy v3.x OpenRouter keys automatically so GapGPT works seamlessly on upgrade
    if t.is_empty() || t.starts_with("sk-or-") {
        return None;
    }
    Some(t.to_string())
}

pub fn has_custom_api_key() -> bool {
    custom_api_key().is_some()
}

pub fn load_api_key() -> Option<String> {
    custom_api_key().or_else(|| {
        let def = default_api_key();
        (!def.is_empty()).then_some(def)
    })
}

pub fn api_key_masked() -> String {
    if let Some(k) = custom_api_key() {
        if k.len() > 12 {
            format!("Custom ({}••••{})", &k[..6], &k[k.len() - 4..])
        } else {
            "Custom (••••••••)".into()
        }
    } else if !default_api_key().is_empty() {
        "Built-in GapGPT (Active)".into()
    } else {
        String::new()
    }
}

fn find_whisper_bin(name: &str) -> Option<PathBuf> {
    let w = ai_root().join("whisper");
    [w.join("Release").join(name), w.join(name)]
        .into_iter()
        .find(|candidate| candidate.is_file())
}

fn model_filename(id: &str) -> &'static str {
    match id {
        "base" => "ggml-base.bin",
        _ => "ggml-small-q5_1.bin",
    }
}

fn model_url(id: &str) -> &'static str {
    match id {
        "base" => MODEL_BASE_URL,
        _ => MODEL_SMALL_URL,
    }
}

fn active_model_id() -> String {
    settings::get("aiWhisperModel")
        .as_str()
        .unwrap_or("small-q5")
        .to_string()
}

fn active_model_path() -> PathBuf {
    let preferred = ai_root()
        .join("models")
        .join(model_filename(&active_model_id()));
    if preferred.is_file() {
        return preferred;
    }
    for fallback in ["ggml-small-q5_1.bin", "ggml-base.bin", "ggml-tiny.bin"] {
        let p = ai_root().join("models").join(fallback);
        if p.is_file() {
            return p;
        }
    }
    preferred
}

// ---------------------------------------------------------------- local DSP asset HTTP server
fn mime_for(name: &str) -> &'static str {
    if name.ends_with(".wasm") {
        "application/wasm"
    } else if name.ends_with(".js") || name.ends_with(".mjs") {
        "application/javascript; charset=utf-8"
    } else {
        "application/octet-stream"
    }
}

fn serve_asset_conn(mut stream: TcpStream) {
    let _ = stream.set_read_timeout(Some(Duration::from_secs(3)));
    let mut req = [0u8; 2048];
    let Ok(n) = stream.read(&mut req) else {
        return;
    };
    let line = String::from_utf8_lossy(&req[..n]);
    let path = line
        .lines()
        .next()
        .and_then(|l| l.split_whitespace().nth(1))
        .unwrap_or("/");
    let clean = path
        .split('?')
        .next()
        .unwrap_or("/")
        .trim_start_matches('/');
    if clean.is_empty()
        || clean.contains("..")
        || clean.contains('/')
        || clean.contains('\\')
        || !clean
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_'))
    {
        let _ = stream.write_all(b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\n\r\n");
        return;
    }
    let file = ai_root().join("dsp").join(clean);
    match std::fs::read(&file) {
        Ok(bytes) => {
            let hdr = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: {}\r\nContent-Length: {}\r\nAccess-Control-Allow-Origin: *\r\nCache-Control: public, max-age=3600\r\nConnection: close\r\n\r\n",
                mime_for(clean),
                bytes.len()
            );
            let _ = stream.write_all(hdr.as_bytes());
            let _ = stream.write_all(&bytes);
        }
        Err(_) => {
            let _ = stream.write_all(
                b"HTTP/1.1 404 Not Found\r\nAccess-Control-Allow-Origin: *\r\nContent-Length: 0\r\n\r\n",
            );
        }
    }
}

pub fn start_asset_server() {
    if ASSET_PORT.get().is_some() {
        return;
    }
    if let Ok(listener) = TcpListener::bind("127.0.0.1:0") {
        if let Ok(addr) = listener.local_addr() {
            let _ = ASSET_PORT.set(addr.port());
            std::thread::spawn(move || {
                for stream in listener.incoming().flatten() {
                    std::thread::spawn(move || serve_asset_conn(stream));
                }
            });
        }
    }
}

pub fn prewarm_whisper() {
    if sys::debug_dir().is_some() {
        return;
    }
    std::thread::spawn(|| {
        std::thread::sleep(Duration::from_secs(2));
        if settings::flag("aiWakeWord") && file_ok(&active_model_path(), 20_000_000) {
            if let Err(e) = ensure_whisper_server() {
                sys::log(&format!("whisper prewarm skipped: {e}"));
            }
        }
    });
}

pub fn asset_base_url() -> String {
    start_asset_server();
    ASSET_PORT
        .get()
        .map(|p| format!("http://127.0.0.1:{p}"))
        .unwrap_or_default()
}

// ---------------------------------------------------------------- prerequisites status & installer
fn file_ok(p: &Path, min_bytes: u64) -> bool {
    std::fs::metadata(p)
        .map(|m| m.len() >= min_bytes)
        .unwrap_or(false)
}

pub fn status() -> Value {
    let dsp = ai_root().join("dsp");
    let rnnoise =
        file_ok(&dsp.join("rnnoise.js"), 1_000) && file_ok(&dsp.join("rnnoise.wasm"), 50_000);
    let silero = file_ok(&dsp.join("ort.wasm.min.js"), 10_000)
        && file_ok(&dsp.join("ort-wasm-simd-threaded.mjs"), 5_000)
        && file_ok(&dsp.join("ort-wasm-simd-threaded.wasm"), 1_000_000)
        && file_ok(&dsp.join("silero_vad.onnx"), 500_000);
    let whisper_bin = find_whisper_bin("whisper-server.exe").is_some()
        || find_whisper_bin("whisper-cli.exe").is_some();
    let model_path = active_model_path();
    let whisper_model = file_ok(&model_path, 20_000_000);
    let key_masked = api_key_masked();
    let has_key = !key_masked.is_empty();
    let dsp_ready = rnnoise && silero;
    let local_whisper_ready = whisper_bin && whisper_model;
    let local_ready = dsp_ready && local_whisper_ready;
    let voice_ready = local_ready || has_key;
    let model_name = model_path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("ggml-small-q5_1.bin");
    json!({
        "rnnoise": rnnoise,
        "silero": silero,
        "whisperBin": whisper_bin,
        "whisperModel": whisper_model,
        "dspReady": dsp_ready,
        "localReady": local_ready,
        "localWhisperReady": local_whisper_ready,
        "cloudSttReady": has_key,
        "modelName": model_name,
        "modelId": if model_name.contains("small") { "small-q5" } else { "base" },
        "voiceReady": voice_ready,
        "hasKey": has_key,
        "customKey": has_custom_api_key(),
        "keyMasked": key_masked,
        "installing": INSTALLING.load(SeqCst),
        "assetUrl": asset_base_url(),
        "model": settings::get("aiModel").as_str().unwrap_or(DEFAULT_MODEL),
        "sttEngine": settings::get("aiSttEngine").as_str().unwrap_or("auto"),
        "sttLang": settings::get("aiSttLang").as_str().unwrap_or("auto"),
        "reasoning": settings::get("aiReasoning").as_str().unwrap_or("fast"),
        "fullAccess": settings::get("aiFullAccess") != Value::Bool(false),
    })
}

pub fn install_prerequisites(model_id: Option<String>) -> Value {
    if INSTALLING.swap(true, SeqCst) {
        return json!({ "ok": false, "error": "Installation already in progress" });
    }
    let chosen_model = model_id
        .filter(|m| m == "base" || m == "small-q5")
        .unwrap_or_else(active_model_id);
    settings::put("aiWhisperModel", chosen_model.clone().into());

    std::thread::spawn(move || {
        let emit_prog = |step: &str, pct: u32, done: bool, err: Option<&str>| {
            sys::emit(
                "ai-install",
                json!({ "step": step, "pct": pct, "done": done, "error": err }),
            );
        };
        let dsp = ai_root().join("dsp");
        let whisper_dir = ai_root().join("whisper");
        let models_dir = ai_root().join("models");

        let steps: Vec<(&str, &str, PathBuf, u64, u32, u32)> = vec![
            (
                "RNNoise JS",
                URL_RNNOISE_JS,
                dsp.join("rnnoise.js"),
                1_000,
                0,
                5,
            ),
            (
                "RNNoise WASM",
                URL_RNNOISE_WASM,
                dsp.join("rnnoise.wasm"),
                50_000,
                5,
                10,
            ),
            (
                "ONNX Runtime JS",
                URL_ORT_JS,
                dsp.join("ort.wasm.min.js"),
                10_000,
                10,
                14,
            ),
            (
                "ONNX Runtime Worker",
                URL_ORT_MJS,
                dsp.join("ort-wasm-simd-threaded.mjs"),
                5_000,
                14,
                17,
            ),
            (
                "ONNX Runtime WASM",
                URL_ORT_WASM,
                dsp.join("ort-wasm-simd-threaded.wasm"),
                1_000_000,
                17,
                30,
            ),
            (
                "Silero VAD ONNX",
                URL_SILERO_ONNX,
                dsp.join("silero_vad.onnx"),
                500_000,
                30,
                38,
            ),
        ];

        for (label, url, dest, min_sz, p0, p1) in steps {
            if !file_ok(&dest, min_sz) {
                emit_prog(&format!("Downloading {label}…"), p0, false, None);
                if let Err(e) = net_http::download_file(url, &dest, |got, tot| {
                    let frac = if tot > 0 {
                        (got as f64 / tot as f64).clamp(0.0, 1.0)
                    } else {
                        0.5
                    };
                    emit_prog(
                        &format!("Downloading {label}…"),
                        p0 + ((p1 - p0) as f64 * frac).round() as u32,
                        false,
                        None,
                    );
                }) {
                    INSTALLING.store(false, SeqCst);
                    emit_prog("Download failed", p0, true, Some(&e));
                    return;
                }
            }
        }

        if find_whisper_bin("whisper-server.exe").is_none() {
            let zip = whisper_dir.join("whisper-bin-x64.zip");
            emit_prog("Downloading whisper.cpp engine…", 38, false, None);
            if let Err(e) = net_http::download_file(URL_WHISPER_ZIP, &zip, |got, tot| {
                let frac = if tot > 0 {
                    (got as f64 / tot as f64).clamp(0.0, 1.0)
                } else {
                    0.5
                };
                emit_prog(
                    "Downloading whisper.cpp engine…",
                    38 + (14.0 * frac).round() as u32,
                    false,
                    None,
                );
            }) {
                INSTALLING.store(false, SeqCst);
                emit_prog("whisper.cpp download failed", 38, true, Some(&e));
                return;
            }
            emit_prog("Extracting whisper.cpp…", 53, false, None);
            let ex = Command::new("tar")
                .args(["-xf"])
                .arg(&zip)
                .arg("-C")
                .arg(&whisper_dir)
                .creation_flags(CREATE_NO_WINDOW)
                .status();
            if !ex.is_ok_and(|s| s.success()) {
                INSTALLING.store(false, SeqCst);
                emit_prog(
                    "Extraction failed",
                    53,
                    true,
                    Some("could not extract whisper-bin-x64.zip"),
                );
                return;
            }
        }

        let m_dest = models_dir.join(model_filename(&chosen_model));
        if !file_ok(&m_dest, 20_000_000) {
            let m_url = model_url(&chosen_model);
            emit_prog("Downloading Whisper Small-Q5 (FA+EN)…", 55, false, None);
            if let Err(e) = net_http::download_file(m_url, &m_dest, |got, tot| {
                let frac = if tot > 0 {
                    (got as f64 / tot as f64).clamp(0.0, 1.0)
                } else {
                    0.5
                };
                let mb = got as f64 / 1_048_576.0;
                let tot_mb = tot as f64 / 1_048_576.0;
                let msg = if tot > 0 {
                    format!("Downloading Whisper model ({mb:.0}/{tot_mb:.0} MB)…")
                } else {
                    format!("Downloading Whisper model ({mb:.0} MB)…")
                };
                emit_prog(&msg, 55 + (44.0 * frac).round() as u32, false, None);
            }) {
                INSTALLING.store(false, SeqCst);
                emit_prog("Model download failed", 55, true, Some(&e));
                return;
            }
        }

        stop_whisper();
        INSTALLING.store(false, SeqCst);
        emit_prog("AI Voice Engine ready", 100, true, None);
        sys::emit("ai-status", status());
    });

    json!({ "ok": true })
}

// ---------------------------------------------------------------- whisper.cpp fast server & STT
fn free_local_port() -> Option<u16> {
    TcpListener::bind("127.0.0.1:0")
        .ok()
        .and_then(|l| l.local_addr().ok())
        .map(|a| a.port())
}

pub fn stop_whisper() {
    if let Some(mut wp) = WHISPER.lock().unwrap().take() {
        let _ = wp.child.kill();
        let _ = wp.child.wait();
    }
}

fn ensure_whisper_server() -> Result<u16, String> {
    let model = active_model_path();
    if !file_ok(&model, 20_000_000) {
        return Err("Whisper model not installed yet — click Download in Settings → AI".into());
    }
    let server_bin = find_whisper_bin("whisper-server.exe")
        .ok_or_else(|| "whisper-server.exe not installed yet".to_string())?;

    let mut guard = WHISPER.lock().unwrap();
    if let Some(wp) = guard.as_mut() {
        if wp.model_path == model && wp.child.try_wait().ok().flatten().is_none() {
            return Ok(wp.port);
        }
        let _ = wp.child.kill();
        let _ = wp.child.wait();
        *guard = None;
    }

    let port = free_local_port().ok_or_else(|| "no free localhost port".to_string())?;
    let threads = std::thread::available_parallelism()
        .map(|n| n.get().clamp(2, 8))
        .unwrap_or(4);

    // Greedy fast decoding (-bs 1 -bo 1 -nf) + 10.24s audio context (-ac 512) cuts CPU latency ~4x
    // while keeping timestamps enabled so multi-sentence Persian & English speech never truncates!
    let mut child = Command::new(&server_bin)
        .args([
            "--host",
            "127.0.0.1",
            "--port",
            &port.to_string(),
            "--model",
            &model.to_string_lossy(),
            "-l",
            "auto",
            "-bs",
            "1",
            "-bo",
            "1",
            "-nf",
            "-ac",
            "512",
            "-t",
            &threads.to_string(),
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .creation_flags(CREATE_NO_WINDOW)
        .spawn()
        .map_err(|e| format!("could not launch whisper-server.exe: {e}"))?;

    let start = Instant::now();
    let addr = format!("127.0.0.1:{port}");
    while start.elapsed() < Duration::from_secs(18) {
        if let Ok(Some(code)) = child.try_wait() {
            return Err(format!("whisper-server exited early ({code})"));
        }
        if TcpStream::connect(&addr).is_ok() {
            *guard = Some(WhisperProc {
                child,
                port,
                model_path: model,
            });
            return Ok(port);
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    let _ = child.kill();
    Err("whisper-server timed out while loading model".into())
}

pub(crate) fn build_whisper_multipart(
    boundary: &str,
    wav_bytes: &[u8],
    language: &str,
    prompt: &str,
) -> Vec<u8> {
    let mut body = Vec::with_capacity(wav_bytes.len() + 1024);
    let mut add_field = |name: &str, val: &str| {
        let _ = write!(
            body,
            "--{boundary}\r\nContent-Disposition: form-data; name=\"{name}\"\r\n\r\n{val}\r\n"
        );
    };
    add_field("response_format", "verbose_json");
    add_field("language", language);
    add_field("temperature", "0.0");
    add_field("temperature_inc", "0.0");
    add_field("beam_size", "1");
    add_field("best_of", "1");
    if !prompt.is_empty() {
        add_field("prompt", prompt);
    }
    let _ = write!(
        body,
        "--{boundary}\r\nContent-Disposition: form-data; name=\"file\"; filename=\"speech.wav\"\r\nContent-Type: audio/wav\r\n\r\n"
    );
    body.extend_from_slice(wav_bytes);
    let _ = write!(body, "\r\n--{boundary}--\r\n");
    body
}

fn build_cloud_stt_multipart(boundary: &str, wav_bytes: &[u8], language: &str) -> Vec<u8> {
    let mut body = Vec::with_capacity(wav_bytes.len() + 512);
    let mut add_field = |name: &str, val: &str| {
        let _ = write!(
            body,
            "--{boundary}\r\nContent-Disposition: form-data; name=\"{name}\"\r\n\r\n{val}\r\n"
        );
    };
    add_field("model", "gapgpt/whisper-1");
    if language == "fa" || language == "en" {
        add_field("language", language);
    }
    let _ = write!(
        body,
        "--{boundary}\r\nContent-Disposition: form-data; name=\"file\"; filename=\"speech.wav\"\r\nContent-Type: audio/wav\r\n\r\n"
    );
    body.extend_from_slice(wav_bytes);
    let _ = write!(body, "\r\n--{boundary}--\r\n");
    body
}

fn transcribe_cloud_gapgpt(wav_bytes: &[u8], stt_lang: &str, t0: Instant) -> Value {
    let Some(api_key) = load_api_key() else {
        return json!({ "ok": false, "error": "No GapGPT API key available" });
    };
    let boundary = "----PILLVYNGapGPTSttBoundary9MA4YWxk";
    let body = build_cloud_stt_multipart(boundary, wav_bytes, stt_lang);
    let ct = format!("multipart/form-data; boundary={boundary}");
    let auth = format!("Bearer {api_key}");
    match net_http::request(
        "POST",
        GAPGPT_STT_URL,
        &[("Authorization", &auth), ("Content-Type", &ct)],
        Some(&body),
        35_000,
    ) {
        Ok((status, resp)) if (200..300).contains(&status) => {
            let parsed: Value = serde_json::from_slice(&resp).unwrap_or(Value::Null);
            let raw_text = parsed["text"].as_str().unwrap_or("").trim();
            let text = clean_whisper_text(raw_text);
            let lang = if is_persian(&text) { "fa" } else { "en" };
            let wake_cmd = match_wake_word(&text);
            json!({
                "ok": true,
                "text": text,
                "language": lang,
                "engine": "cloud",
                "wakeMatched": wake_cmd.is_some(),
                "wakeCommand": wake_cmd.unwrap_or_default(),
                "ms": t0.elapsed().as_millis() as u64,
            })
        }
        Ok((status, resp)) => {
            let msg = String::from_utf8_lossy(&resp);
            json!({ "ok": false, "error": format!("GapGPT STT HTTP {status}: {}", msg.chars().take(160).collect::<String>()) })
        }
        Err(e) => json!({ "ok": false, "error": e }),
    }
}

pub fn transcribe_wav(wav_b64: &str, prompt: Option<&str>) -> Value {
    let t0 = Instant::now();
    let Some(wav_bytes) = sys::unb64(wav_b64) else {
        return json!({ "ok": false, "error": "invalid WAV base64" });
    };
    if wav_bytes.len() < 100 {
        return json!({ "ok": false, "error": "audio clip too short" });
    }
    let stt_lang = match settings::get("aiSttLang").as_str().unwrap_or("auto") {
        "fa" => "fa",
        "en" => "en",
        _ => "auto",
    };
    let engine = settings::get("aiSttEngine")
        .as_str()
        .unwrap_or("auto")
        .to_string();

    if engine == "cloud" {
        return transcribe_cloud_gapgpt(&wav_bytes, stt_lang, t0);
    }

    let port = match ensure_whisper_server() {
        Ok(p) => p,
        Err(e) => {
            // In "auto" mode, seamlessly use GapGPT Cloud STT until local whisper.cpp is installed
            if engine == "auto" {
                return transcribe_cloud_gapgpt(&wav_bytes, stt_lang, t0);
            }
            return json!({ "ok": false, "error": e });
        }
    };

    let boundary = "----PILLVYNWhisperBoundary7MA4YWxkTrZu0gW";
    let body = build_whisper_multipart(boundary, &wav_bytes, stt_lang, prompt.unwrap_or(""));
    let ct = format!("multipart/form-data; boundary={boundary}");
    let url = format!("http://127.0.0.1:{port}/inference");

    match net_http::request("POST", &url, &[("Content-Type", &ct)], Some(&body), 55_000) {
        Ok((status, resp)) if (200..300).contains(&status) => {
            let parsed: Value = serde_json::from_slice(&resp).unwrap_or(Value::Null);
            let raw_text = parsed["text"].as_str().unwrap_or("").trim();
            let text = clean_whisper_text(raw_text);
            let lang = parsed["language"]
                .as_str()
                .map(|s| s.to_string())
                .unwrap_or_else(|| {
                    if is_persian(&text) {
                        "fa".into()
                    } else {
                        "en".into()
                    }
                });
            let wake_cmd = match_wake_word(&text);
            json!({
                "ok": true,
                "text": text,
                "language": lang,
                "engine": "local",
                "wakeMatched": wake_cmd.is_some(),
                "wakeCommand": wake_cmd.unwrap_or_default(),
                "ms": t0.elapsed().as_millis() as u64,
            })
        }
        Ok((status, resp)) => {
            let msg = String::from_utf8_lossy(&resp);
            json!({ "ok": false, "error": format!("whisper-server HTTP {status}: {msg}") })
        }
        Err(e) => {
            stop_whisper();
            if engine == "auto" {
                return transcribe_cloud_gapgpt(&wav_bytes, stt_lang, t0);
            }
            json!({ "ok": false, "error": e })
        }
    }
}

fn clean_whisper_text(raw: &str) -> String {
    let mut s = raw.trim().to_string();
    for noise in [
        "[BLANK_AUDIO]",
        "(blank audio)",
        "[Silence]",
        "(silence)",
        "[Music]",
        "(music)",
        "Thank you for watching.",
        "Thanks for watching.",
        "♪",
    ] {
        s = s.replace(noise, "");
    }
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

// ---------------------------------------------------------------- Phonetic + Levenshtein Wake Word ("Hello PILLVYN" / "سلام پیلوین")
fn levenshtein(a: &str, b: &str) -> usize {
    let av: Vec<char> = a.chars().collect();
    let bv: Vec<char> = b.chars().collect();
    let mut prev: Vec<usize> = (0..=bv.len()).collect();
    let mut curr = vec![0usize; bv.len() + 1];
    for (i, &ca) in av.iter().enumerate() {
        curr[0] = i + 1;
        for (j, &cb) in bv.iter().enumerate() {
            let cost = usize::from(ca != cb);
            curr[j + 1] = (prev[j + 1] + 1).min(curr[j] + 1).min(prev[j] + cost);
        }
        std::mem::swap(&mut prev, &mut curr);
    }
    prev[bv.len()]
}

fn norm_token(tok: &str) -> String {
    tok.to_lowercase()
        .chars()
        .filter_map(|c| match c {
            'ي' | 'ى' => Some('ی'),
            'ك' => Some('ک'),
            'أ' | 'إ' | 'آ' => Some('ا'),
            '\u{200c}' => None,
            c if c.is_alphanumeric() => Some(c),
            _ => None,
        })
        .collect()
}

fn is_greeting(t: &str) -> bool {
    matches!(
        t,
        "hello"
            | "helo"
            | "hullo"
            | "hey"
            | "hi"
            | "ok"
            | "okay"
            | "yo"
            | "alo"
            | "سلام"
            | "های"
            | "هی"
            | "الو"
            | "هلو"
            | "بگو"
            | "جانم"
    )
}

fn is_pillvyn_like(t: &str, after_greeting: bool) -> bool {
    if t.is_empty() {
        return false;
    }
    // Exact / close Latin targets
    if matches!(
        t,
        "pillvyn"
            | "pilvyn"
            | "pilvin"
            | "pillvin"
            | "pillwin"
            | "pelvin"
            | "pilwin"
            | "pilven"
            | "pillven"
            | "pillbin"
            | "philvin"
            | "billvin"
            | "pilfin"
            | "pillfin"
            | "pillvan"
            | "pilvan"
            | "pillin"
            | "pillone"
            | "pillman"
            | "pilgrim"
            | "pilbin"
            | "pylvin"
            | "پیلوین"
            | "بیلوین"
            | "فیلوین"
            | "پلوین"
            | "پیلو"
            | "پیلون"
            | "پیلین"
            | "پلون"
            | "پیلویم"
            | "پیلویگ"
            | "پیلوینگ"
    ) {
        return true;
    }
    // Phonetic fuzzy edit-distance match (strictly tighter without greeting so normal words never false-trigger)
    let len = t.chars().count();
    if t.is_ascii() {
        if !(5..=10).contains(&len) {
            return false;
        }
        let starts_ok = matches!(t.as_bytes()[0], b'p' | b'b' | b'f');
        let d = levenshtein(t, "pillvyn").min(levenshtein(t, "pilvin"));
        if after_greeting {
            (starts_ok && d <= 2) || (t.starts_with("pill") && len <= 10) || d <= 1
        } else {
            starts_ok && d <= 1
        }
    } else {
        if !(4..=8).contains(&len) {
            return false;
        }
        let d = levenshtein(t, "پیلوین");
        if after_greeting {
            d <= 2
        } else {
            d <= 1
        }
    }
}

/// Matches "Hello PILLVYN", "Hey PILLVYN", "سلام پیلوین", fuzzy phonetic Whisper variants
/// (including spelled-out letters like "hello P I L L V Y N", split two-word outputs like
/// "hello pill win" / "سلام پیل وین"), or standalone "PILLVYN ..." / "پیلوین ..." at sentence start.
pub fn match_wake_word(transcript: &str) -> Option<String> {
    let raw_tokens: Vec<&str> = transcript.split_whitespace().collect();
    let norm: Vec<String> = raw_tokens.iter().map(|t| norm_token(t)).collect();
    let clean_rest = |start_idx: usize| -> String {
        raw_tokens[start_idx..]
            .join(" ")
            .trim_start_matches(|c: char| !c.is_alphanumeric())
            .trim()
            .to_string()
    };

    // 1. Greeting + 1-word, 2-word joined, or spelled-out letter run (e.g. "HELLO P I L L V Y N")
    for i in 0..norm.len().min(5) {
        if is_greeting(&norm[i]) {
            if i + 1 < norm.len() && is_pillvyn_like(&norm[i + 1], true) {
                return Some(clean_rest(i + 2));
            }
            if i + 2 < norm.len() {
                let joined = format!("{}{}", norm[i + 1], norm[i + 2]);
                if is_pillvyn_like(&joined, true) {
                    return Some(clean_rest(i + 3));
                }
            }
            // Collapse runs of 1-3 char spelled-out fragments after greeting (e.g. "P I L L V W ION")
            // greedily taking the longest matching span so trailing spelled letters aren't left in the command
            if i + 3 < norm.len() && norm[i + 1].len() == 1 {
                let mut acc = String::new();
                let mut j = i + 1;
                let mut best_end: Option<usize> = None;
                while j < norm.len() && j < i + 10 && norm[j].len() <= 3 {
                    acc.push_str(&norm[j]);
                    j += 1;
                    if acc.len() >= 5 && is_pillvyn_like(&acc, true) {
                        best_end = Some(j);
                    }
                }
                if let Some(end_idx) = best_end {
                    return Some(clean_rest(end_idx));
                }
            }
        }
    }

    // 2. Direct brand invocation at the start ("PILLVYN, open Chrome" / "پیلوین ساعت چنده")
    if !norm.is_empty() && is_pillvyn_like(&norm[0], false) {
        return Some(clean_rest(1));
    }
    if norm.len() >= 2 {
        let joined = format!("{}{}", norm[0], norm[1]);
        if is_pillvyn_like(&joined, false) {
            return Some(clean_rest(2));
        }
    }
    None
}

pub fn is_persian(text: &str) -> bool {
    let mut fa = 0usize;
    let mut lat = 0usize;
    for c in text.chars() {
        let u = c as u32;
        if (0x0600..=0x06FF).contains(&u) || (0xFB50..=0xFDFF).contains(&u) {
            fa += 1;
        } else if c.is_ascii_alphabetic() {
            lat += 1;
        }
    }
    fa > 0 && fa >= lat / 2
}

// ---------------------------------------------------------------- GapGPT Smart Brain (gapgpt-gemma-26b-a4b + Real-Time SSE Streaming)
fn system_prompt(client_ctx: &Value, full_access: bool) -> String {
    let vol = audio::volume()
        .map(|(v, m)| {
            format!(
                "{}%{}",
                (v * 100.0).round() as i32,
                if m { " (muted)" } else { "" }
            )
        })
        .unwrap_or_else(|| "unknown".into());
    let fg = hit::fg_context();
    let active_app = fg["title"].as_str().unwrap_or("");
    let skin = settings::get("skin")
        .as_str()
        .unwrap_or("glass")
        .to_string();
    let face = settings::get("face").as_str().unwrap_or("eyes").to_string();
    let cal = settings::get("calendar")
        .as_str()
        .unwrap_or("gregorian")
        .to_string();
    let shelf_names: Vec<String> = shelf::list()
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|f| f["name"].as_str().map(str::to_string))
        .take(8)
        .collect();
    let notes_count = store::notes().as_array().map(|a| a.len()).unwrap_or(0);
    let local_time = client_ctx["time"].as_str().unwrap_or("");
    let local_date = client_ctx["date"].as_str().unwrap_or("");
    let media = client_ctx["media"].as_str().unwrap_or("");
    let timer = client_ctx["timer"].as_str().unwrap_or("idle");
    let net_state = client_ctx["net"].as_str().unwrap_or("online");
    let recent_notifs = client_ctx["notifs"].as_str().unwrap_or("none");
    let access_note = if full_access {
        "You have FULL Windows system access (PowerShell, launching apps, managing windows, processes, reading/writing/searching files, brightness, power controls, keyboard typing/shortcuts, live web search) PLUS total control over every PILLVYN Dynamic Island feature (themes, settings, resting face, calendar, panels, focus timer, media, volume, notes & reminders, file shelf, clipboard, notifications, password generator, OCR, screenshot). NEVER claim you cannot access the system — directly call the best tool(s) to accomplish what the user asks!"
    } else {
        "System-level PowerShell/file/power tools are currently disabled by user preference; use PILLVYN Dynamic Island tools and web search."
    };
    format!(
        "You are PILLVYN v4, an ultra-fast, deeply integrated, intelligent Dynamic Island AI voice assistant for Windows powered by GapGPT (Gemma 4 26B).\n\
         Language & Voice Style:\n\
         - Always reply in the EXACT language the user used.\n\
         - When replying in Persian (فارسی), write natural, fluent, warm everyday spoken Persian (فارسی روان، طبیعی و بدون غلط املایی). Never translate literally or sound robotic.\n\
         - When replying in English, write crisp, natural, friendly English.\n\
         - Keep replies concise (1–3 clear sentences for voice commands; detailed only when asked for explanations or code).\n\
         - Never clutter normal spoken replies with markdown asterisks, hashtags, or emojis so Edge Neural TTS reads them cleanly.\n\
         - Return valid JSON if the user explicitly asks for JSON.\n\
         PILLVYN Dynamic Island Knowledge:\n\
         - Shortcuts: Ctrl+Alt+A (AI voice), Ctrl+Alt+N (Quick Note & reminder), Ctrl+Alt+T (Offline EN/FA OCR Text Grab).\n\
         - Panels you can open/close via `control_pillvyn_ui`: ai, settings (tabs: appearance, behavior, features, ai, about), notes, clips, shelf, inbox, vault, tour, timer.\n\
         - 14 Themes (`switch_theme` / `manage_settings`): glass (Liquid Glass), frost (Frosted Pearl), black (Obsidian), aurora, nebula, crystal (Sapphire), solar (Amber Horizon), venom, ember, reactor, cloud, water, lava, ice.\n\
         - {access_note}\n\
         - You may call multiple tools in parallel or across steps when needed, then confirm clearly.\n\
         Live PILLVYN & Windows State: time={local_time}, date={local_date}, calendar={cal}, theme={skin}, face={face}, volume={vol}, playing=\"{media}\", timer={timer}, net={net_state}, shelf=[{}], notes={notes_count}, recent_notifs=\"{recent_notifs}\", active_window=\"{active_app}\".",
        shelf_names.join(", ")
    )
}

#[derive(Default, Clone)]
pub(crate) struct StreamToolCall {
    pub id: String,
    pub name: String,
    pub arguments: String,
}

#[derive(Default)]
pub(crate) struct StreamAccum {
    pub content: String,
    pub tool_calls: Vec<StreamToolCall>,
    pub model: String,
}

pub(crate) fn apply_sse_data(acc: &mut StreamAccum, data: &str) -> Option<String> {
    let trimmed = data.trim();
    if trimmed.is_empty() || trimmed == "[DONE]" {
        return None;
    }
    let v: Value = serde_json::from_str(trimmed).ok()?;
    if let Some(m) = v["model"].as_str() {
        if !m.is_empty() {
            acc.model = m.to_string();
        }
    }
    let delta = &v["choices"][0]["delta"];
    let mut text_delta = None;
    if let Some(c) = delta["content"].as_str() {
        if !c.is_empty() {
            acc.content.push_str(c);
            text_delta = Some(c.to_string());
        }
    }
    if let Some(tcs) = delta["tool_calls"].as_array() {
        for tc in tcs {
            let idx = tc["index"].as_u64().unwrap_or(0) as usize;
            while acc.tool_calls.len() <= idx {
                acc.tool_calls.push(StreamToolCall::default());
            }
            let slot = &mut acc.tool_calls[idx];
            if let Some(id) = tc["id"].as_str().filter(|s| !s.is_empty()) {
                slot.id = id.to_string();
            }
            if let Some(name) = tc["function"]["name"].as_str().filter(|s| !s.is_empty()) {
                slot.name.push_str(name);
            }
            if let Some(args) = tc["function"]["arguments"].as_str() {
                slot.arguments.push_str(args);
            }
        }
    }
    text_delta
}

fn call_gapgpt_stream(
    api_key: &str,
    model: &str,
    messages: &[Value],
    full_access: bool,
    deep_reasoning: bool,
    round: usize,
) -> Result<StreamAccum, String> {
    let auth = format!("Bearer {api_key}");
    let headers = [
        ("Authorization", auth.as_str()),
        ("Content-Type", "application/json"),
        ("Accept", "text/event-stream"),
    ];
    let tools = ai_tools::tool_definitions(full_access);
    let mut payload = json!({
        "model": model,
        "messages": messages,
        "tools": tools,
        "tool_choice": "auto",
        "stream": true,
        "temperature": 0.3,
    });
    if !deep_reasoning {
        payload["reasoning_effort"] = json!("none");
    }
    let body = serde_json::to_vec(&payload).map_err(|e| e.to_string())?;

    let mut last_err = String::new();
    for attempt in 0..3 {
        if attempt > 0 {
            std::thread::sleep(Duration::from_millis(650 * attempt as u64));
        }
        let mut acc = StreamAccum {
            model: model.to_string(),
            ..Default::default()
        };
        let mut saw_sse = false;
        let res = net_http::post_sse(GAPGPT_CHAT_URL, &headers, &body, 45_000, |line| {
            if let Some(data) = line.strip_prefix("data:") {
                saw_sse = true;
                if let Some(delta) = apply_sse_data(&mut acc, data) {
                    sys::emit(
                        "ai-stream",
                        json!({ "delta": delta, "text": acc.content, "round": round }),
                    );
                }
            }
        });

        match res {
            Ok((status, _))
                if (200..300).contains(&status)
                    && (saw_sse || !acc.content.is_empty() || !acc.tool_calls.is_empty()) =>
            {
                return Ok(acc);
            }
            Ok((status, err_bytes)) => {
                let txt = String::from_utf8_lossy(&err_bytes);
                let parsed_msg = serde_json::from_slice::<Value>(&err_bytes)
                    .ok()
                    .and_then(|v| v["error"]["message"].as_str().map(str::to_string))
                    .unwrap_or_else(|| txt.chars().take(180).collect());
                last_err = format!("GapGPT HTTP {status}: {parsed_msg}");
                if matches!(status, 429 | 502 | 503 | 504) {
                    continue;
                }
                return Err(last_err);
            }
            Err(e) => {
                last_err = format!("GapGPT connection error: {e}");
            }
        }
    }
    Err(last_err)
}

pub fn clear_history() {
    HISTORY.lock().unwrap().clear();
}

pub fn chat(user_text: &str, client_ctx: Value) -> Value {
    let text = user_text.trim();
    if text.is_empty() {
        return json!({ "ok": false, "error": "empty message" });
    }
    let Some(api_key) = load_api_key() else {
        return json!({
            "ok": false,
            "error": "No GapGPT API key configured. Check Settings → AI."
        });
    };
    let model = settings::get("aiModel")
        .as_str()
        .filter(|s| !s.trim().is_empty())
        .unwrap_or(DEFAULT_MODEL)
        .to_string();
    let full_access = settings::get("aiFullAccess") != Value::Bool(false);
    let deep_reasoning = settings::get("aiReasoning").as_str() == Some("deep");

    let mut conv = {
        let mut h = HISTORY.lock().unwrap();
        h.push(json!({ "role": "user", "content": text }));
        while h.len() > MAX_HISTORY {
            h.remove(0);
        }
        let mut msgs = Vec::with_capacity(h.len() + 1);
        msgs.push(json!({ "role": "system", "content": system_prompt(&client_ctx, full_access) }));
        msgs.extend(h.iter().cloned());
        msgs
    };

    let mut tool_badges: Vec<String> = Vec::new();
    let mut final_reply = String::new();
    let mut used_model = model.clone();
    let t0 = Instant::now();

    for round in 0..6 {
        let streamed =
            match call_gapgpt_stream(&api_key, &model, &conv, full_access, deep_reasoning, round) {
                Ok(s) => s,
                Err(e) => {
                    let mut h = HISTORY.lock().unwrap();
                    if h.last().and_then(|m| m["role"].as_str()) == Some("user") {
                        h.pop();
                    }
                    return json!({ "ok": false, "error": e });
                }
            };
        if !streamed.model.is_empty() {
            used_model = streamed.model;
        }

        let valid_calls: Vec<StreamToolCall> = streamed
            .tool_calls
            .into_iter()
            .filter(|tc| !tc.name.is_empty())
            .collect();

        if !valid_calls.is_empty() {
            let tcalls_json: Vec<Value> = valid_calls
                .iter()
                .enumerate()
                .map(|(i, tc)| {
                    let id = if tc.id.is_empty() {
                        format!("call_{round}_{i}")
                    } else {
                        tc.id.clone()
                    };
                    json!({
                        "id": id,
                        "type": "function",
                        "function": {
                            "name": tc.name,
                            "arguments": if tc.arguments.is_empty() { "{}".to_string() } else { tc.arguments.clone() },
                        }
                    })
                })
                .collect();
            let assistant_msg = json!({
                "role": "assistant",
                "content": if streamed.content.is_empty() { Value::Null } else { Value::String(streamed.content) },
                "tool_calls": tcalls_json,
            });
            conv.push(assistant_msg.clone());
            {
                let mut h = HISTORY.lock().unwrap();
                h.push(assistant_msg);
            }
            for tc in &tcalls_json {
                let id = tc["id"].as_str().unwrap_or("call_0");
                let fname = tc["function"]["name"].as_str().unwrap_or("");
                let raw_args = tc["function"]["arguments"].as_str().unwrap_or("{}");
                let args: Value = serde_json::from_str(raw_args).unwrap_or_else(|_| json!({}));
                sys::emit("ai-tool", json!({ "name": fname, "status": "running" }));
                let (result_json, badge) = ai_tools::execute_tool(fname, &args, full_access);
                tool_badges.push(badge.clone());
                sys::emit(
                    "ai-tool",
                    json!({ "name": fname, "badge": badge, "status": "done" }),
                );
                let tool_msg = json!({
                    "role": "tool",
                    "tool_call_id": id,
                    "content": result_json,
                });
                conv.push(tool_msg.clone());
                {
                    let mut h = HISTORY.lock().unwrap();
                    h.push(tool_msg);
                }
            }
            continue;
        }

        final_reply = streamed.content.trim().to_string();
        if final_reply.is_empty() && !tool_badges.is_empty() {
            final_reply = if is_persian(text) {
                format!("انجام شد: {}", tool_badges.join(" · "))
            } else {
                format!("Done: {}", tool_badges.join(" · "))
            };
        }
        let mut h = HISTORY.lock().unwrap();
        h.push(json!({ "role": "assistant", "content": final_reply }));
        while h.len() > MAX_HISTORY {
            h.remove(0);
        }
        break;
    }

    let lang = if is_persian(&final_reply) { "fa" } else { "en" };
    json!({
        "ok": true,
        "reply": final_reply,
        "lang": lang,
        "tools": tool_badges,
        "model": used_model,
        "ms": t0.elapsed().as_millis() as u64,
    })
}

// ---------------------------------------------------------------- Broadcast-Clean Edge TTS
/// Strips markdown, code blocks, URLs, emojis, and expands technical symbols into natural spoken
/// Persian/English so Microsoft Edge Neural TTS delivers crisp, natural speech.
pub(crate) fn clean_for_tts(raw: &str) -> String {
    let mut s = raw.to_string();
    // Remove fenced code blocks (keep conversational prose only)
    while let Some(a) = s.find("```") {
        let rest = &s[a + 3..];
        if let Some(b) = rest.find("```") {
            s.replace_range(a..a + 3 + b + 3, " ");
        } else {
            s.truncate(a);
            break;
        }
    }
    // Convert markdown links [label](url) -> label
    while let Some(open) = s.find('[') {
        let rest = &s[open + 1..];
        let Some(mid) = rest.find("](") else { break };
        let after = &rest[mid + 2..];
        let Some(close) = after.find(')') else { break };
        let label = rest[..mid].to_string();
        s.replace_range(open..open + 1 + mid + 2 + close + 1, &label);
    }
    let persian = is_persian(&s);
    if persian {
        s = s
            .replace("°C", " درجه ")
            .replace("GB", " گیگابایت ")
            .replace("MB", " مگابایت ")
            .replace("CPU", " سی‌پی‌یو ")
            .replace("RAM", " رم ")
            .replace(['%', '٪'], " درصد ")
            .replace('→', " به ");
    }
    // Drop bare http(s) URLs and markdown punctuation + emojis/symbols
    let mut out = String::with_capacity(s.len());
    for word in s.split_whitespace() {
        if word.starts_with("http://") || word.starts_with("https://") {
            continue;
        }
        if !out.is_empty() {
            out.push(' ');
        }
        for c in word.chars() {
            let u = c as u32;
            let is_emoji = (0x1F000..=0x1FAFF).contains(&u)
                || (0x2300..=0x23FF).contains(&u)
                || (0x2600..=0x27BF).contains(&u)
                || (0x2B00..=0x2BFF).contains(&u)
                || (0xFE00..=0xFE0F).contains(&u)
                || u == 0x200D;
            if is_emoji || matches!(c, '*' | '`' | '#' | '_' | '~' | '>' | '|' | '•' | '▪' | '◦')
            {
                continue;
            }
            out.push(c);
        }
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn format_signed_pct(n: i64) -> String {
    if n >= 0 {
        format!("+{n}%")
    } else {
        format!("{n}%")
    }
}

pub fn synthesize_tts(
    text: &str,
    voice_override: Option<&str>,
    rate_override: Option<i64>,
    volume_override: Option<i64>,
) -> Value {
    let clean = clean_for_tts(text);
    if clean.is_empty() {
        return json!({ "ok": false, "error": "empty text after sanitizing" });
    }
    let persian = is_persian(&clean);
    let voice = match voice_override.filter(|v| !v.is_empty() && *v != "auto") {
        Some(v) => v.to_string(),
        None => {
            if persian {
                settings::get("aiTtsVoiceFa")
                    .as_str()
                    .unwrap_or("fa-IR-DilaraNeural")
                    .to_string()
            } else {
                settings::get("aiTtsVoiceEn")
                    .as_str()
                    .unwrap_or("en-US-AvaMultilingualNeural")
                    .to_string()
            }
        }
    };
    let rate_num = rate_override
        .unwrap_or_else(|| settings::get("aiTtsRate").as_i64().unwrap_or(8))
        .clamp(-50, 100);
    let vol_pct = volume_override
        .unwrap_or_else(|| settings::get("aiTtsVolume").as_i64().unwrap_or(90))
        .clamp(0, 100);
    let ssml_vol = format_signed_pct((vol_pct - 100).clamp(-50, 0));
    let ssml_rate = format_signed_pct(rate_num);

    match net_http::edge_tts_mp3(&clean, &voice, &ssml_rate, &ssml_vol) {
        Ok(mp3) => json!({
            "ok": true,
            "audioB64": sys::b64(&mp3),
            "mime": "audio/mpeg",
            "voice": voice,
            "bytes": mp3.len(),
        }),
        Err(e) => json!({ "ok": false, "error": e }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtin_gapgpt_key_decodes_cleanly() {
        let k = default_api_key();
        assert!(k.starts_with("sk-"));
        assert!(k.len() >= 48);
    }

    #[test]
    fn wake_word_matches_english_persian_and_fuzzy_whisper_variants() {
        assert_eq!(match_wake_word("Hello PILLVYN"), Some(String::new()));
        assert_eq!(
            match_wake_word("Hello PILLVYN, open https://github.com"),
            Some("open https://github.com".into())
        );
        assert_eq!(
            match_wake_word("Hello pill win, set a 5 minute timer"),
            Some("set a 5 minute timer".into())
        );
        assert_eq!(
            match_wake_word("HELLO P I L L V W ION OPEN CHROME"),
            Some("OPEN CHROME".into())
        );
        assert_eq!(
            match_wake_word("Hey Pilven what time is it?"),
            Some("what time is it?".into())
        );
        assert_eq!(match_wake_word("سلام پیلوین"), Some(String::new()));
        assert_eq!(
            match_wake_word("سلام پیل وین! تایمر رو روی ۵ دقیقه بذار"),
            Some("تایمر رو روی ۵ دقیقه بذار".into())
        );
        assert_eq!(
            match_wake_word("پیلوین کروم رو باز کن"),
            Some("کروم رو باز کن".into())
        );
        assert_eq!(match_wake_word("just a normal English sentence"), None);
        assert_eq!(match_wake_word("سلام چطوری رفیق"), None);
    }

    #[test]
    fn sse_parser_accumulates_text_and_parallel_tool_calls() {
        let mut acc = StreamAccum::default();
        assert_eq!(
            apply_sse_data(
                &mut acc,
                r#"{"model":"google/gemma-4-26B-A4B-it","choices":[{"index":0,"delta":{"content":"سلام "}}]}"#,
            ),
            Some("سلام ".into())
        );
        assert_eq!(
            apply_sse_data(
                &mut acc,
                r#"{"choices":[{"index":0,"delta":{"content":"پیلوین!"}}]}"#,
            ),
            Some("پیلوین!".into())
        );
        assert_eq!(acc.content, "سلام پیلوین!");
        assert_eq!(acc.model, "google/gemma-4-26B-A4B-it");

        let mut acc2 = StreamAccum::default();
        apply_sse_data(
            &mut acc2,
            r#"{"choices":[{"index":0,"delta":{"tool_calls":[{"id":"c0","index":0,"function":{"name":"set_timer"}}]}}]}"#,
        );
        apply_sse_data(
            &mut acc2,
            r#"{"choices":[{"index":0,"delta":{"tool_calls":[{"index":0,"function":{"arguments":"{\"minutes\":5}"}}]}}]}"#,
        );
        apply_sse_data(
            &mut acc2,
            r#"{"choices":[{"index":0,"delta":{"tool_calls":[{"id":"c1","index":1,"function":{"name":"launch_app","arguments":"{\"target\":\"chrome\"}"}}]}}]}"#,
        );
        assert_eq!(acc2.tool_calls.len(), 2);
        assert_eq!(acc2.tool_calls[0].name, "set_timer");
        assert_eq!(acc2.tool_calls[0].arguments, r#"{"minutes":5}"#);
        assert_eq!(acc2.tool_calls[1].name, "launch_app");
    }

    #[test]
    fn detects_persian_vs_english_script() {
        assert!(is_persian("سلام چطوری؟"));
        assert!(is_persian("تایمر ۵ دقیقه تنظیم شد"));
        assert!(!is_persian("Timer set for 5 minutes."));
        assert!(!is_persian("Hello PILLVYN"));
    }

    #[test]
    fn sanitizes_markdown_and_emojis_for_tts() {
        let clean = clean_for_tts(
            "**سلام!** رم ۵۰% پر است (8 GB) ⏱️😊 [لینک](https://example.com) ```js\ncode\n```",
        );
        assert_eq!(clean, "سلام! رم ۵۰ درصد پر است (8 گیگابایت ) لینک");
    }

    #[test]
    fn builds_valid_whisper_multipart_payload() {
        let body = build_whisper_multipart("BOUND", b"RIFF1234", "fa", "");
        let text = String::from_utf8_lossy(&body);
        assert!(text.contains("--BOUND\r\n"));
        assert!(text.contains("name=\"language\"\r\n\r\nfa"));
        assert!(text.contains("filename=\"speech.wav\""));
        assert!(text.contains("RIFF1234"));
        assert!(text.ends_with("--BOUND--\r\n"));
    }
}
