use std::f64::consts::TAU;
#[cfg(target_os = "windows")]
use std::sync::OnceLock;

#[cfg(target_os = "windows")]
static SMS_DING_WAV: OnceLock<&'static [u8]> = OnceLock::new();

/// Builds a subtle, studio-tuned two-note crystalline PCM WAV bell chime
/// (44.1 kHz, 16-bit mono, ~380 ms) suitable for asynchronous Win32 `PlaySoundW` playback.
pub fn build_sms_ding_wav() -> Vec<u8> {
    const SAMPLE_RATE: u32 = 44_100;
    const DURATION_MS: usize = 390;
    let total_samples = (SAMPLE_RATE as usize * DURATION_MS) / 1000;
    let attack_samples = (SAMPLE_RATE as f64 * 0.005) as usize; // 5 ms soft cosine onset

    let data_bytes = (total_samples * 2) as u32;
    let riff_size = 36 + data_bytes;
    let mut wav = Vec::with_capacity(44 + data_bytes as usize);

    // RIFF header
    wav.extend_from_slice(b"RIFF");
    wav.extend_from_slice(&riff_size.to_le_bytes());
    wav.extend_from_slice(b"WAVE");

    // fmt chunk (PCM mono 16-bit @ 44100 Hz)
    wav.extend_from_slice(b"fmt ");
    wav.extend_from_slice(&16u32.to_le_bytes());
    wav.extend_from_slice(&1u16.to_le_bytes()); // PCM
    wav.extend_from_slice(&1u16.to_le_bytes()); // Mono
    wav.extend_from_slice(&SAMPLE_RATE.to_le_bytes());
    wav.extend_from_slice(&(SAMPLE_RATE * 2).to_le_bytes()); // Byte rate
    wav.extend_from_slice(&2u16.to_le_bytes()); // Block align
    wav.extend_from_slice(&16u16.to_le_bytes()); // Bits per sample

    // data chunk header
    wav.extend_from_slice(b"data");
    wav.extend_from_slice(&data_bytes.to_le_bytes());

    let second_tone_start = (SAMPLE_RATE as f64 * 0.095) as usize; // Subtle second shimmer at 95 ms
    for idx in 0..total_samples {
        let t = idx as f64 / SAMPLE_RATE as f64;

        // Primary crystalline bell: E6 (1318.51 Hz) + gentle 2nd harmonic + light air shimmer
        let attack1 = if idx < attack_samples {
            0.5 - 0.5 * ((idx as f64 / attack_samples as f64) * std::f64::consts::PI).cos()
        } else {
            1.0
        };
        let env1 = attack1 * (-t * 14.5).exp();
        let primary = (TAU * 1318.51 * t).sin() * 0.68
            + (TAU * 2637.02 * t).sin() * 0.22 * (-t * 18.0).exp()
            + (TAU * 3955.53 * t).sin() * 0.08 * (-t * 28.0).exp();

        // Secondary delicate harmonic echo: B6 (1975.53 Hz) + B5 warmth
        let secondary = if idx >= second_tone_start {
            let local_idx = idx - second_tone_start;
            let t2 = local_idx as f64 / SAMPLE_RATE as f64;
            let attack2 = if local_idx < attack_samples {
                0.5 - 0.5
                    * ((local_idx as f64 / attack_samples as f64) * std::f64::consts::PI).cos()
            } else {
                1.0
            };
            let env2 = attack2 * (-t2 * 13.5).exp();
            env2 * ((TAU * 1975.53 * t2).sin() * 0.50 + (TAU * 987.77 * t2).sin() * 0.16)
        } else {
            0.0
        };

        // Keep peak subtle (-10 dBFS max scale) and fade final 20 ms cleanly to zero
        let tail_fade = if total_samples - idx < 882 {
            (total_samples - idx) as f64 / 882.0
        } else {
            1.0
        };
        let sample_f = (env1 * primary * 0.25 + secondary * 0.20) * tail_fade;
        let clamped = (sample_f * i16::MAX as f64).clamp(i16::MIN as f64, i16::MAX as f64) as i16;
        wav.extend_from_slice(&clamped.to_le_bytes());
    }

    wav
}

#[cfg(target_os = "windows")]
fn cached_ding_wav() -> &'static [u8] {
    SMS_DING_WAV.get_or_init(|| Box::leak(build_sms_ding_wav().into_boxed_slice()))
}

/// Plays the subtle native Windows SMS arrival chime asynchronously from pinned static memory.
/// No-op inside unit/integration tests so test suites remain silent.
pub fn play_sms_ding() {
    if cfg!(test) {
        return;
    }

    #[cfg(target_os = "windows")]
    {
        use windows::core::PCWSTR;
        use windows::Win32::Media::Audio::{PlaySoundW, SND_ASYNC, SND_MEMORY, SND_NODEFAULT};

        let wav = cached_ding_wav();
        unsafe {
            let _ = PlaySoundW(
                PCWSTR(wav.as_ptr() as *const u16),
                None,
                SND_MEMORY | SND_ASYNC | SND_NODEFAULT,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_valid_gentle_pcm_ding_wav() {
        let wav = build_sms_ding_wav();
        assert_eq!(&wav[0..4], b"RIFF");
        assert_eq!(&wav[8..12], b"WAVE");
        assert_eq!(&wav[12..16], b"fmt ");
        assert_eq!(&wav[36..40], b"data");

        let data_len = u32::from_le_bytes(wav[40..44].try_into().unwrap()) as usize;
        assert_eq!(wav.len(), 44 + data_len);
        assert!(data_len > 30_000);

        let max_abs = wav[44..]
            .chunks_exact(2)
            .map(|c| i16::from_le_bytes([c[0], c[1]]).abs())
            .max()
            .unwrap_or(0);
        // Subtle chime amplitude: audible (> 2500) yet restrained (< 14000 out of 32767)
        assert!(max_abs > 2500 && max_abs < 14000);
        // Final sample cleanly faded out
        let last = i16::from_le_bytes([wav[wav.len() - 2], wav[wav.len() - 1]]).abs();
        assert!(last < 32);
    }
}
