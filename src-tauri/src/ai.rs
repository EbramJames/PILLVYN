// PILLVYN AI Assistant backend:
// - DPAPI-encrypted OpenRouter API key store (never stored in plaintext or committed to git)
// - Localhost DSP asset server for RNNoise WASM + ONNX Runtime Web + Silero VAD ONNX
// - One-click prerequisites manager for RNNoise, Silero VAD, whisper.cpp, and multilingual GGML models
// - Managed whisper-server.exe (auto Persian/English STT + wake-word verification)
// - OpenRouter Smart Brain (inclusionai/ling-3.2-flash:free) with multi-turn memory & multi-step Tool Calling
// - Native Microsoft Edge Neural TTS (Persian & English MP3 with voice, speed, and volume)
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::os::windows::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering::SeqCst};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use serde_json::{json, Value};

use crate::{audio, clip, hit, net_http, ocr, poll, settings, store, sys};

const CREATE_NO_WINDOW: u32 = 0x0800_0000;
const OPENROUTER_URL: &str = "https://openrouter.ai/api/v1/chat/completions";
pub const DEFAULT_MODEL: &str = "inclusionai/ling-3.2-flash:free";
const MAX_HISTORY: usize = 20;

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

pub fn load_api_key() -> Option<String> {
    if let Ok(env_k) =
        std::env::var("PILLVYN_OPENROUTER_KEY").or_else(|_| std::env::var("OPENROUTER_API_KEY"))
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
    (!t.is_empty()).then(|| t.to_string())
}

pub fn api_key_masked() -> String {
    match load_api_key() {
        Some(k) if k.len() > 12 => format!("{}••••{}", &k[..8], &k[k.len() - 4..]),
        Some(_) => "••••••••".into(),
        None => String::new(),
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
        "small-q5" => "ggml-small-q5_1.bin",
        _ => "ggml-base.bin",
    }
}

fn model_url(id: &str) -> &'static str {
    match id {
        "small-q5" => MODEL_SMALL_URL,
        _ => MODEL_BASE_URL,
    }
}

fn active_model_id() -> String {
    settings::get("aiWhisperModel")
        .as_str()
        .unwrap_or("base")
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
    let voice_ready = rnnoise && silero && whisper_bin && whisper_model;
    json!({
        "rnnoise": rnnoise,
        "silero": silero,
        "whisperBin": whisper_bin,
        "whisperModel": whisper_model,
        "modelName": model_path.file_name().and_then(|n| n.to_str()).unwrap_or("ggml-base.bin"),
        "modelId": active_model_id(),
        "voiceReady": voice_ready,
        "hasKey": has_key,
        "keyMasked": key_masked,
        "installing": INSTALLING.load(SeqCst),
        "assetUrl": asset_base_url(),
        "model": settings::get("aiModel").as_str().unwrap_or(DEFAULT_MODEL),
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
                    let pct = p0 + ((p1 - p0) as f64 * frac).round() as u32;
                    emit_prog(&format!("Downloading {label}…"), pct, false, None);
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
            emit_prog("Downloading Whisper multilingual model…", 55, false, None);
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

// ---------------------------------------------------------------- whisper.cpp server & STT
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
        .map(|n| n.get().clamp(2, 6))
        .unwrap_or(4);

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
            "-nt",
            "--suppress-nst",
            "-t",
            &threads.to_string(),
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .creation_flags(CREATE_NO_WINDOW)
        .spawn()
        .map_err(|e| format!("could not launch whisper-server.exe: {e}"))?;

    // Wait up to 15s for whisper-server.exe HTTP socket to accept connections
    let start = Instant::now();
    let addr = format!("127.0.0.1:{port}");
    while start.elapsed() < Duration::from_secs(15) {
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
        std::thread::sleep(Duration::from_millis(120));
    }
    let _ = child.kill();
    Err("whisper-server timed out while loading model".into())
}

pub(crate) fn build_whisper_multipart(boundary: &str, wav_bytes: &[u8], prompt: &str) -> Vec<u8> {
    let mut body = Vec::with_capacity(wav_bytes.len() + 1024);
    let mut add_field = |name: &str, val: &str| {
        let _ = write!(
            body,
            "--{boundary}\r\nContent-Disposition: form-data; name=\"{name}\"\r\n\r\n{val}\r\n"
        );
    };
    add_field("response_format", "verbose_json");
    add_field("language", "auto");
    add_field("temperature", "0.0");
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

pub fn transcribe_wav(wav_b64: &str, prompt: Option<&str>) -> Value {
    let t0 = Instant::now();
    let Some(wav_bytes) = sys::unb64(wav_b64) else {
        return json!({ "ok": false, "error": "invalid WAV base64" });
    };
    if wav_bytes.len() < 100 {
        return json!({ "ok": false, "error": "audio clip too short" });
    }
    let port = match ensure_whisper_server() {
        Ok(p) => p,
        Err(e) => return json!({ "ok": false, "error": e }),
    };

    let boundary = "----PILLVYNWhisperBoundary7MA4YWxkTrZu0gW";
    let p_hint = prompt.unwrap_or("");
    let body = build_whisper_multipart(boundary, &wav_bytes, p_hint);
    let ct = format!("multipart/form-data; boundary={boundary}");
    let url = format!("http://127.0.0.1:{port}/inference");

    match net_http::request("POST", &url, &[("Content-Type", &ct)], Some(&body), 35_000) {
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
            json!({ "ok": false, "error": e })
        }
    }
}

fn clean_whisper_text(raw: &str) -> String {
    let mut s = raw.trim().to_string();
    // Strip common whisper non-speech hallucination markers if any slip past --suppress-nst
    for noise in [
        "[BLANK_AUDIO]",
        "(blank audio)",
        "[Silence]",
        "(silence)",
        "[Music]",
        "(music)",
        "♪",
    ] {
        s = s.replace(noise, "");
    }
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

// ---------------------------------------------------------------- Wake word ("Hello PILLVYN" / "سلام پیلوین")
fn normalize_latin(s: &str) -> String {
    s.to_lowercase()
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c.is_whitespace() {
                c
            } else {
                ' '
            }
        })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn normalize_persian(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            'ي' | 'ى' => 'ی',
            'ك' => 'ک',
            'أ' | 'إ' | 'آ' => 'ا',
            '\u{200c}' => ' ', // ZWNJ
            c if c.is_alphanumeric() || c.is_whitespace() => c,
            _ => ' ',
        })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// Returns `Some(remainder_command)` when `transcript` contains the wake phrase
/// ("Hello PILLVYN", "Hey PILLVYN", "سلام پیلوین", or close phonetic Whisper transcriptions).
pub fn match_wake_word(transcript: &str) -> Option<String> {
    let norm_en = normalize_latin(transcript);
    const EN_WAKE: &[&str] = &[
        "hello pillvyn",
        "hello pilvyn",
        "hello pilvin",
        "hello pillvin",
        "hello pillwin",
        "hello pelvin",
        "hello pilwin",
        "hello pill ven",
        "hello pil ven",
        "hello pill van",
        "hello pillbin",
        "hello pilven",
        "hello philvin",
        "hello pill fin",
        "hello pilgrim",
        "hey pillvyn",
        "hey pilvyn",
        "hey pilvin",
        "hey pillvin",
        "hey pillwin",
        "hi pillvyn",
        "hi pilvin",
        "ok pillvyn",
        "okay pillvyn",
    ];
    for phrase in EN_WAKE {
        if let Some(pos) = norm_en.find(phrase) {
            let after = norm_en[pos + phrase.len()..].trim();
            return Some(after.to_string());
        }
    }

    let norm_fa = normalize_persian(transcript);
    const FA_WAKE: &[&str] = &[
        "سلام پیلوین",
        "سلام پیل وین",
        "سلام بیلوین",
        "سلام فیلوین",
        "سلام پیلو این",
        "های پیلوین",
        "الو پیلوین",
        "هی پیلوین",
        "هلو پیلوین",
        "هلو پیل وین",
    ];
    for phrase in FA_WAKE {
        if let Some(pos) = norm_fa.find(phrase) {
            let after = norm_fa[pos + phrase.len()..].trim();
            return Some(after.to_string());
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

// ---------------------------------------------------------------- OpenRouter Smart Brain + Tool Calling
fn tool_definitions() -> Value {
    json!([
        {
            "type": "function",
            "function": {
                "name": "set_timer",
                "description": "Start a countdown timer on the PILLVYN Dynamic Island.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "minutes": { "type": "number", "description": "Duration in minutes (1 to 180)" }
                    },
                    "required": ["minutes"]
                }
            }
        },
        {
            "type": "function",
            "function": {
                "name": "control_timer",
                "description": "Pause, resume, or reset the active PILLVYN timer.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "action": { "type": "string", "enum": ["pause", "resume", "reset"] }
                    },
                    "required": ["action"]
                }
            }
        },
        {
            "type": "function",
            "function": {
                "name": "media_control",
                "description": "Control the currently playing music or media on Windows.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "action": { "type": "string", "enum": ["play_pause", "next", "previous"] }
                    },
                    "required": ["action"]
                }
            }
        },
        {
            "type": "function",
            "function": {
                "name": "set_volume",
                "description": "Set the Windows master speaker volume percentage (0 to 100).",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "percent": { "type": "number", "description": "Volume percentage 0..100" }
                    },
                    "required": ["percent"]
                }
            }
        },
        {
            "type": "function",
            "function": {
                "name": "toggle_mute",
                "description": "Toggle mute on the Windows system audio output.",
                "parameters": { "type": "object", "properties": {} }
            }
        },
        {
            "type": "function",
            "function": {
                "name": "create_note",
                "description": "Save a quick note in PILLVYN, optionally with a reminder in N minutes.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "text": { "type": "string", "description": "The note or reminder text" },
                        "remind_in_minutes": { "type": "number", "description": "Optional reminder delay in minutes (0 for plain note)" }
                    },
                    "required": ["text"]
                }
            }
        },
        {
            "type": "function",
            "function": {
                "name": "list_notes",
                "description": "Read the user's saved quick notes and reminders from PILLVYN.",
                "parameters": { "type": "object", "properties": {} }
            }
        },
        {
            "type": "function",
            "function": {
                "name": "get_clipboard_history",
                "description": "Read the most recent copied clipboard snippets from PILLVYN.",
                "parameters": { "type": "object", "properties": {} }
            }
        },
        {
            "type": "function",
            "function": {
                "name": "copy_to_clipboard",
                "description": "Copy generated text, code, translation, or an answer directly to the Windows clipboard.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "text": { "type": "string", "description": "Text to copy" }
                    },
                    "required": ["text"]
                }
            }
        },
        {
            "type": "function",
            "function": {
                "name": "switch_theme",
                "description": "Change PILLVYN's visual theme immediately.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "theme": {
                            "type": "string",
                            "enum": ["glass", "frost", "black", "aurora", "nebula", "crystal", "solar", "venom"]
                        }
                    },
                    "required": ["theme"]
                }
            }
        },
        {
            "type": "function",
            "function": {
                "name": "open_url",
                "description": "Open a web URL (https:// or http://) in the user's default browser.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "url": { "type": "string", "description": "Valid http or https URL" }
                    },
                    "required": ["url"]
                }
            }
        },
        {
            "type": "function",
            "function": {
                "name": "desktop_action",
                "description": "Trigger a built-in PILLVYN desktop action: screenshot, ocr (text grab), or dnd (Windows Do Not Disturb settings).",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "action": { "type": "string", "enum": ["screenshot", "ocr", "dnd"] }
                    },
                    "required": ["action"]
                }
            }
        }
    ])
}

fn execute_tool(name: &str, args: &Value) -> (String, String) {
    match name {
        "set_timer" => {
            let mins = args["minutes"].as_f64().unwrap_or(5.0).clamp(0.5, 180.0);
            sys::emit("ai-action", json!({ "type": "set_timer", "minutes": mins }));
            (
                json!({ "ok": true, "minutes": mins }).to_string(),
                format!("Timer {mins}m"),
            )
        }
        "control_timer" => {
            let act = args["action"].as_str().unwrap_or("pause");
            sys::emit(
                "ai-action",
                json!({ "type": "control_timer", "action": act }),
            );
            (
                json!({ "ok": true, "action": act }).to_string(),
                format!("Timer {act}"),
            )
        }
        "media_control" => {
            let act = args["action"].as_str().unwrap_or("play_pause");
            match act {
                "next" => poll::send(poll::Cmd::Next),
                "previous" => poll::send(poll::Cmd::Prev),
                _ => poll::send(poll::Cmd::Toggle),
            }
            (
                json!({ "ok": true, "action": act }).to_string(),
                format!("Media {act}"),
            )
        }
        "set_volume" => {
            let pct = args["percent"].as_f64().unwrap_or(50.0).clamp(0.0, 100.0);
            poll::send(poll::Cmd::Vol((pct / 100.0) as f32));
            (
                json!({ "ok": true, "percent": pct.round() }).to_string(),
                format!("Volume {}%", pct.round() as i32),
            )
        }
        "toggle_mute" => {
            poll::send(poll::Cmd::Mute);
            (json!({ "ok": true }).to_string(), "Mute toggled".into())
        }
        "create_note" => {
            let text = args["text"].as_str().unwrap_or("").trim();
            let remind_min = args["remind_in_minutes"].as_f64().unwrap_or(0.0).max(0.0);
            let remind_ms = if remind_min > 0.0 {
                sys::now_ms() + (remind_min * 60_000.0).round() as u64
            } else {
                0
            };
            if text.is_empty() {
                (
                    json!({ "ok": false, "error": "empty text" }).to_string(),
                    "Note skipped".into(),
                )
            } else {
                let _ = store::note_save(None, text, remind_ms);
                let badge = if remind_min > 0.0 {
                    format!("Reminder ({remind_min}m)")
                } else {
                    "Note saved".into()
                };
                (
                    json!({ "ok": true, "saved": text, "remindInMinutes": remind_min }).to_string(),
                    badge,
                )
            }
        }
        "list_notes" => {
            let notes = store::notes();
            let compact: Vec<Value> = notes
                .as_array()
                .map(|arr| {
                    arr.iter()
                        .take(10)
                        .map(|n| json!({ "text": n["text"], "pinned": n["pinned"], "remind": n["remind"] }))
                        .collect()
                })
                .unwrap_or_default();
            (json!({ "notes": compact }).to_string(), "Read notes".into())
        }
        "get_clipboard_history" => {
            let clips = clip::list();
            let compact: Vec<Value> = clips
                .as_array()
                .map(|arr| {
                    arr.iter()
                        .take(8)
                        .map(|c| json!({ "kind": c["kind"], "preview": c["preview"], "pinned": c["pinned"] }))
                        .collect()
                })
                .unwrap_or_default();
            (
                json!({ "clips": compact }).to_string(),
                "Read clipboard".into(),
            )
        }
        "copy_to_clipboard" => {
            let text = args["text"].as_str().unwrap_or("");
            let ok = !text.is_empty() && clip::write_text(text);
            (json!({ "ok": ok }).to_string(), "Copied".into())
        }
        "switch_theme" => {
            let theme = args["theme"].as_str().unwrap_or("glass");
            settings::apply("skin", &Value::String(theme.to_string()));
            (
                json!({ "ok": true, "theme": theme }).to_string(),
                format!("Theme → {theme}"),
            )
        }
        "open_url" => {
            let u = args["url"].as_str().unwrap_or("").trim();
            let valid = (u.starts_with("https://") || u.starts_with("http://"))
                && u.len() < 2000
                && !u
                    .chars()
                    .any(|c| c.is_whitespace() || c.is_control() || c == '"');
            if valid {
                sys::shell_open(u, None);
                (
                    json!({ "ok": true, "url": u }).to_string(),
                    "Opened link".into(),
                )
            } else {
                (
                    json!({ "ok": false, "error": "invalid URL" }).to_string(),
                    "Blocked URL".into(),
                )
            }
        }
        "desktop_action" => {
            let act = args["action"].as_str().unwrap_or("");
            match act {
                "screenshot" => sys::shell_open("ms-screenclip:", None),
                "ocr" => ocr::start(),
                "dnd" => sys::shell_open("ms-settings:notifications", None),
                _ => {
                    return (
                        json!({ "ok": false, "error": "unknown action" }).to_string(),
                        "Unknown action".into(),
                    )
                }
            }
            (
                json!({ "ok": true, "action": act }).to_string(),
                format!("Action: {act}"),
            )
        }
        _ => (
            json!({ "ok": false, "error": format!("unknown tool: {name}") }).to_string(),
            format!("Unknown: {name}"),
        ),
    }
}

fn system_prompt(client_ctx: &Value) -> String {
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
    let local_time = client_ctx["time"].as_str().unwrap_or("");
    let local_date = client_ctx["date"].as_str().unwrap_or("");
    let media = client_ctx["media"].as_str().unwrap_or("");
    format!(
        "You are PILLVYN, a sleek, fast, friendly AI voice assistant living inside a Windows Dynamic Island overlay.\n\
         Rules:\n\
         - Reply naturally and concisely in the SAME language the user uses (Persian / فارسی or English). Keep spoken replies clear, warm, and short (1–3 sentences unless the user asks for detail).\n\
         - Never use markdown tables or heavy formatting when speaking casually, so text-to-speech sounds crisp and human.\n\
         - Whenever the user asks to control PILLVYN or Windows (timer, music, volume, notes/reminders, clipboard, theme, links, screenshot, text grab), call the appropriate tool(s) first, then confirm concisely.\n\
         - If the user asks for structured JSON explicitly, return valid JSON.\n\
         Live Windows context: time={local_time}, date={local_date}, volume={vol}, theme={skin}, playing=\"{media}\", active_window=\"{active_app}\"."
    )
}

fn call_openrouter(api_key: &str, model: &str, messages: &[Value]) -> Result<Value, String> {
    let payload = json!({
        "model": model,
        "messages": messages,
        "tools": tool_definitions(),
        "tool_choice": "auto",
        "temperature": 0.4,
    });
    let body = serde_json::to_vec(&payload).map_err(|e| e.to_string())?;
    let auth = format!("Bearer {api_key}");
    let headers = [
        ("Authorization", auth.as_str()),
        ("Content-Type", "application/json"),
        ("HTTP-Referer", "https://github.com/EbramJames/PILLVYN"),
        ("X-Title", "PILLVYN"),
    ];

    let mut last_err = String::new();
    for attempt in 0..3u32 {
        if attempt > 0 {
            std::thread::sleep(Duration::from_millis(1200 * attempt as u64));
        }
        match net_http::request("POST", OPENROUTER_URL, &headers, Some(&body), 35_000) {
            Ok((status, resp)) if (200..300).contains(&status) => {
                let v: Value = serde_json::from_slice(&resp)
                    .map_err(|e| format!("invalid JSON from OpenRouter: {e}"))?;
                if let Some(err) = v.get("error") {
                    let msg = err["message"].as_str().unwrap_or("OpenRouter error");
                    let code = err["code"].as_i64().unwrap_or(0);
                    last_err = format!("OpenRouter ({code}): {msg}");
                    if code == 429 || code == 502 || code == 503 {
                        continue;
                    }
                    return Err(last_err);
                }
                return Ok(v);
            }
            Ok((status, resp)) => {
                let txt = String::from_utf8_lossy(&resp);
                let parsed_msg = serde_json::from_slice::<Value>(&resp)
                    .ok()
                    .and_then(|v| v["error"]["message"].as_str().map(str::to_string))
                    .unwrap_or_else(|| txt.chars().take(180).collect());
                last_err = format!("OpenRouter HTTP {status}: {parsed_msg}");
                if matches!(status, 429 | 502 | 503 | 504) {
                    continue;
                }
                return Err(last_err);
            }
            Err(e) => {
                last_err = e;
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
            "error": "No OpenRouter API key set. Add your key in Settings → AI."
        });
    };
    let model = settings::get("aiModel")
        .as_str()
        .filter(|s| !s.trim().is_empty())
        .unwrap_or(DEFAULT_MODEL)
        .to_string();

    let mut conv = {
        let mut h = HISTORY.lock().unwrap();
        h.push(json!({ "role": "user", "content": text }));
        while h.len() > MAX_HISTORY {
            h.remove(0);
        }
        let mut msgs = Vec::with_capacity(h.len() + 1);
        msgs.push(json!({ "role": "system", "content": system_prompt(&client_ctx) }));
        msgs.extend(h.iter().cloned());
        msgs
    };

    let mut tool_badges: Vec<String> = Vec::new();
    let mut final_reply = String::new();
    let t0 = Instant::now();

    for _round in 0..4 {
        let resp = match call_openrouter(&api_key, &model, &conv) {
            Ok(v) => v,
            Err(e) => {
                // Roll back the failed user turn so conversation stays clean
                let mut h = HISTORY.lock().unwrap();
                if h.last().and_then(|m| m["role"].as_str()) == Some("user") {
                    h.pop();
                }
                return json!({ "ok": false, "error": e });
            }
        };
        let msg = &resp["choices"][0]["message"];
        let calls = msg["tool_calls"].as_array();

        if let Some(tcalls) = calls.filter(|a| !a.is_empty()) {
            let assistant_msg = json!({
                "role": "assistant",
                "content": msg["content"].clone(),
                "tool_calls": tcalls,
            });
            conv.push(assistant_msg.clone());
            {
                let mut h = HISTORY.lock().unwrap();
                h.push(assistant_msg);
            }
            for tc in tcalls {
                let id = tc["id"].as_str().unwrap_or("call_0");
                let fname = tc["function"]["name"].as_str().unwrap_or("");
                let raw_args = tc["function"]["arguments"].as_str().unwrap_or("{}");
                let args: Value = serde_json::from_str(raw_args).unwrap_or_else(|_| json!({}));
                let (result_json, badge) = execute_tool(fname, &args);
                tool_badges.push(badge.clone());
                sys::emit("ai-tool", json!({ "name": fname, "badge": badge }));
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

        final_reply = msg["content"].as_str().unwrap_or("").trim().to_string();
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
        "model": model,
        "ms": t0.elapsed().as_millis() as u64,
    })
}

// ---------------------------------------------------------------- Edge TTS
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
    let clean = text.trim();
    if clean.is_empty() {
        return json!({ "ok": false, "error": "empty text" });
    }
    let persian = is_persian(clean);
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
                    .unwrap_or("en-US-AvaNeural")
                    .to_string()
            }
        }
    };
    let rate_num = rate_override
        .unwrap_or_else(|| settings::get("aiTtsRate").as_i64().unwrap_or(0))
        .clamp(-50, 100);
    let vol_pct = volume_override
        .unwrap_or_else(|| settings::get("aiTtsVolume").as_i64().unwrap_or(90))
        .clamp(0, 100);
    // Edge SSML volume accepts relative -50%..+50%; final linear amplitude is also applied on <audio>.volume
    let ssml_vol = format_signed_pct((vol_pct - 100).clamp(-50, 0));
    let ssml_rate = format_signed_pct(rate_num);

    match net_http::edge_tts_mp3(clean, &voice, &ssml_rate, &ssml_vol) {
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
    fn wake_word_matches_english_and_persian_with_inline_commands() {
        assert_eq!(match_wake_word("Hello PILLVYN"), Some(String::new()));
        assert_eq!(
            match_wake_word("Hello PILLVYN, set a 5 minute timer!"),
            Some("set a 5 minute timer".into())
        );
        assert_eq!(
            match_wake_word("Hey pilvin what time is it"),
            Some("what time is it".into())
        );
        assert_eq!(match_wake_word("سلام پیلوین"), Some(String::new()));
        assert_eq!(
            match_wake_word("سلام پیل وین! تایمر رو روی ۵ دقیقه بذار"),
            Some("تایمر رو روی ۵ دقیقه بذار".into())
        );
        assert_eq!(match_wake_word("just a normal sentence"), None);
    }

    #[test]
    fn detects_persian_vs_english_script() {
        assert!(is_persian("سلام چطوری؟"));
        assert!(is_persian("تایمر ۵ دقیقه تنظیم شد"));
        assert!(!is_persian("Timer set for 5 minutes."));
        assert!(!is_persian("Hello PILLVYN"));
    }

    #[test]
    fn builds_valid_whisper_multipart_payload() {
        let body = build_whisper_multipart("BOUND", b"RIFF1234", "Hello PILLVYN");
        let text = String::from_utf8_lossy(&body);
        assert!(text.contains("--BOUND\r\n"));
        assert!(text.contains("name=\"language\"\r\n\r\nauto"));
        assert!(text.contains("filename=\"speech.wav\""));
        assert!(text.contains("RIFF1234"));
        assert!(text.ends_with("--BOUND--\r\n"));
    }

    #[test]
    fn exposes_complete_tool_schema() {
        let tools = tool_definitions();
        let arr = tools.as_array().unwrap();
        assert!(arr.len() >= 11);
        assert!(arr.iter().any(|t| t["function"]["name"] == "set_timer"));
        assert!(arr.iter().any(|t| t["function"]["name"] == "switch_theme"));
    }
}
