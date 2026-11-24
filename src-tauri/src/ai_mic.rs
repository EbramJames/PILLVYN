// Native Windows WASAPI microphone capture at 48 kHz mono (exact frame rate expected by RNNoise).
// Streams 40 ms (1920 samples = 4 x 480 RNNoise frames) Int16 PCM packets only while AI voice is listening.
use std::sync::atomic::{AtomicBool, Ordering::SeqCst};
use std::time::{Duration, Instant};

use serde_json::json;
use windows::Win32::Media::Audio::*;
use windows::Win32::System::Com::{
    CoCreateInstance, CoInitializeEx, CoTaskMemFree, CLSCTX_ALL, COINIT_MULTITHREADED,
};

use crate::sys;

pub static MIC_ON: AtomicBool = AtomicBool::new(false);
const TARGET_RATE: usize = 48_000;
const BATCH_SAMPLES: usize = 1_920; // 40 ms @ 48 kHz = 4 RNNoise 10ms frames

pub fn set_enabled(on: bool) {
    MIC_ON.store(on, SeqCst);
}

unsafe fn default_mic(enumerator: &IMMDeviceEnumerator) -> windows::core::Result<IMMDevice> {
    enumerator
        .GetDefaultAudioEndpoint(eCapture, eCommunications)
        .or_else(|_| enumerator.GetDefaultAudioEndpoint(eCapture, eConsole))
}

unsafe fn capture_loop() -> windows::core::Result<()> {
    let enumerator: IMMDeviceEnumerator = CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)?;
    let device = default_mic(&enumerator)?;
    let dev_id = sys::pwstr_take(device.GetId()?);
    let client: IAudioClient = device.Activate(CLSCTX_ALL, None)?;
    let pwfx = client.GetMixFormat()?;
    let fmt = std::ptr::read_unaligned(pwfx);
    let (channels, in_rate, bits) = (
        fmt.nChannels as usize,
        fmt.nSamplesPerSec as usize,
        fmt.wBitsPerSample,
    );
    let mut is_float = fmt.wFormatTag == 3;
    if fmt.wFormatTag == 0xFFFE {
        let ext = std::ptr::read_unaligned(pwfx as *const WAVEFORMATEXTENSIBLE);
        is_float = ext.SubFormat.data1 == 3;
    }
    let init = client.Initialize(
        AUDCLNT_SHAREMODE_SHARED,
        0,
        2_000_000, // 200 ms buffer
        0,
        pwfx,
        None,
    );
    CoTaskMemFree(Some(pwfx as *const _));
    init?;
    let cap: IAudioCaptureClient = client.GetService()?;
    client.Start()?;

    sys::emit("ai-mic", json!({ "active": true, "rate": in_rate }));

    // Linear resampler state from in_rate -> 48_000 Hz mono
    let step = in_rate as f64 / TARGET_RATE as f64;
    let mut frac = 0.0f64;
    let mut prev = 0.0f32;
    let mut have_prev = false;
    let mut out_pcm: Vec<u8> = Vec::with_capacity(BATCH_SAMPLES * 2);
    let mut last_check = Instant::now();

    while MIC_ON.load(SeqCst) {
        std::thread::sleep(Duration::from_millis(10));
        loop {
            let frames = cap.GetNextPacketSize()?;
            if frames == 0 {
                break;
            }
            let (mut data, mut got, mut flags) = (std::ptr::null_mut::<u8>(), 0u32, 0u32);
            cap.GetBuffer(&mut data, &mut got, &mut flags, None, None)?;
            let silent = flags & 2 != 0 || data.is_null();
            let mut mono = Vec::with_capacity(got as usize);
            for f in 0..got as usize {
                let mut s = 0f32;
                if !silent {
                    for c in 0..channels {
                        let i = f * channels + c;
                        s += if is_float && bits == 32 {
                            std::ptr::read_unaligned(data.add(i * 4) as *const f32)
                        } else if bits == 16 {
                            std::ptr::read_unaligned(data.add(i * 2) as *const i16) as f32 / 32768.0
                        } else if bits == 32 {
                            std::ptr::read_unaligned(data.add(i * 4) as *const i32) as f32
                                / 2_147_483_648.0
                        } else {
                            0.0
                        };
                    }
                    s /= channels.max(1) as f32;
                }
                mono.push(s.clamp(-1.0, 1.0));
            }
            cap.ReleaseBuffer(got)?;

            if in_rate == TARGET_RATE {
                for &s in &mono {
                    let q = (s * 32767.0).round().clamp(-32768.0, 32767.0) as i16;
                    out_pcm.extend_from_slice(&q.to_le_bytes());
                    if out_pcm.len() >= BATCH_SAMPLES * 2 {
                        sys::emit("ai-pcm", sys::b64(&out_pcm));
                        out_pcm.clear();
                    }
                }
            } else if !mono.is_empty() {
                // Smooth linear interpolation resampling to exact 48 kHz
                let mut src = Vec::with_capacity(mono.len() + 1);
                src.push(if have_prev { prev } else { mono[0] });
                src.extend_from_slice(&mono);
                prev = *mono.last().unwrap_or(&0.0);
                have_prev = true;

                let max_idx = (src.len() - 1) as f64;
                while frac < max_idx {
                    let idx = frac.floor() as usize;
                    let t = (frac - idx as f64) as f32;
                    let s0 = src[idx];
                    let s1 = src[(idx + 1).min(src.len() - 1)];
                    let s = s0 + (s1 - s0) * t;
                    let q = (s * 32767.0).round().clamp(-32768.0, 32767.0) as i16;
                    out_pcm.extend_from_slice(&q.to_le_bytes());
                    if out_pcm.len() >= BATCH_SAMPLES * 2 {
                        sys::emit("ai-pcm", sys::b64(&out_pcm));
                        out_pcm.clear();
                    }
                    frac += step;
                }
                frac -= mono.len() as f64;
                if frac < 0.0 {
                    frac = 0.0;
                }
            }
        }

        if last_check.elapsed() > Duration::from_secs(3) {
            last_check = Instant::now();
            let now_id = default_mic(&enumerator)
                .and_then(|d| d.GetId())
                .map(|p| sys::pwstr_take(p))
                .unwrap_or_default();
            if now_id != dev_id {
                break;
            }
        }
    }
    let _ = client.Stop();
    sys::emit("ai-mic", json!({ "active": false }));
    Ok(())
}

pub fn start() {
    std::thread::spawn(|| unsafe {
        let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
        loop {
            if MIC_ON.load(SeqCst) {
                if let Err(e) = capture_loop() {
                    sys::log(&format!("ai mic capture failed: {e}"));
                    sys::emit(
                        "ai-mic",
                        json!({ "active": false, "error": format!("{e}") }),
                    );
                    std::thread::sleep(Duration::from_secs(2));
                }
            }
            std::thread::sleep(Duration::from_millis(120));
        }
    });
}
