// Updates: now and then the app asks GitHub whether a newer release exists, downloads its installer,
// verifies its SHA-256 checksum, and tells the page. With "Update automatically" on (the default)
// it then installs itself at a quiet moment: right after the app has started, or once nobody has touched the
// computer for a few minutes. Otherwise one click installs it. The next start says what happened.
use std::os::windows::process::CommandExt;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::Duration;

use serde_json::{json, Value};
use windows::core::HSTRING;
use windows::Win32::System::Com::Urlmon::URLDownloadToFileW;
use windows::Win32::System::SystemInformation::GetTickCount;
use windows::Win32::UI::Input::KeyboardAndMouse::{GetLastInputInfo, LASTINPUTINFO};

use crate::{settings, sys};

const BASE: &str = "https://github.com/EbramJames/PILLVYN/releases/latest/download";
const SETUP_NAME: &str = "PILLVYN-Setup.exe";
const SHA_NAME: &str = "PILLVYN-Setup.exe.sha256";

#[derive(Clone)]
struct ReadyUpdate {
    version: String,
    setup: PathBuf,
    sha256: String,
}

static READY: Mutex<Option<ReadyUpdate>> = Mutex::new(None);

fn dir() -> PathBuf {
    let d = std::env::temp_dir().join("PILLVYNUpdate");
    let _ = std::fs::create_dir_all(&d);
    d
}
fn fetch(name: &str) -> Option<PathBuf> {
    let to = dir().join(name);
    let _ = std::fs::remove_file(&to);
    let url = format!("{BASE}/{name}?t={}", sys::now_ms()); // the query keeps Windows from serving an old copy
    unsafe {
        URLDownloadToFileW(
            None,
            &HSTRING::from(url),
            &HSTRING::from(to.to_string_lossy().as_ref()),
            0,
            None,
        )
        .ok()?
    };
    to.exists().then_some(to)
}
fn newer(a: &str, than: &str) -> bool {
    let n = |s: &str| {
        s.trim_start_matches('v')
            .split('.')
            .map(|p| p.parse::<u32>().unwrap_or(0))
            .collect::<Vec<_>>()
    };
    n(a) > n(than)
}

fn sha256_bytes(data: &[u8]) -> [u8; 32] {
    const K: [u32; 64] = [
        0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4,
        0xab1c5ed5, 0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe,
        0x9bdc06a7, 0xc19bf174, 0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f,
        0x4a7484aa, 0x5cb0a9dc, 0x76f988da, 0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7,
        0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967, 0x27b70a85, 0x2e1b2138, 0x4d2c6dfc,
        0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85, 0xa2bfe8a1, 0xa81a664b,
        0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070, 0x19a4c116,
        0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
        0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7,
        0xc67178f2,
    ];
    let mut h: [u32; 8] = [
        0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab,
        0x5be0cd19,
    ];
    let bit_len = (data.len() as u64).wrapping_mul(8);
    let mut msg = Vec::with_capacity(data.len() + 72);
    msg.extend_from_slice(data);
    msg.push(0x80);
    while msg.len() % 64 != 56 {
        msg.push(0);
    }
    msg.extend_from_slice(&bit_len.to_be_bytes());
    for block in msg.as_chunks::<64>().0 {
        let mut w = [0u32; 64];
        for (i, word) in w.iter_mut().take(16).enumerate() {
            let b = i * 4;
            *word = u32::from_be_bytes([block[b], block[b + 1], block[b + 2], block[b + 3]]);
        }
        for i in 16..64 {
            let s0 = w[i - 15].rotate_right(7) ^ w[i - 15].rotate_right(18) ^ (w[i - 15] >> 3);
            let s1 = w[i - 2].rotate_right(17) ^ w[i - 2].rotate_right(19) ^ (w[i - 2] >> 10);
            w[i] = w[i - 16]
                .wrapping_add(s0)
                .wrapping_add(w[i - 7])
                .wrapping_add(s1);
        }
        let [mut a, mut b, mut c, mut d, mut e, mut f, mut g, mut hh] = h;
        for i in 0..64 {
            let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let ch = (e & f) ^ ((!e) & g);
            let temp1 = hh
                .wrapping_add(s1)
                .wrapping_add(ch)
                .wrapping_add(K[i])
                .wrapping_add(w[i]);
            let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let maj = (a & b) ^ (a & c) ^ (b & c);
            let temp2 = s0.wrapping_add(maj);
            hh = g;
            g = f;
            f = e;
            e = d.wrapping_add(temp1);
            d = c;
            c = b;
            b = a;
            a = temp1.wrapping_add(temp2);
        }
        for (slot, delta) in h.iter_mut().zip([a, b, c, d, e, f, g, hh]) {
            *slot = slot.wrapping_add(delta);
        }
    }
    let mut out = [0u8; 32];
    for (i, word) in h.iter().enumerate() {
        out[i * 4..i * 4 + 4].copy_from_slice(&word.to_be_bytes());
    }
    out
}

fn sha256_hex(data: &[u8]) -> String {
    use std::fmt::Write;
    let mut out = String::with_capacity(64);
    for b in sha256_bytes(data) {
        let _ = write!(&mut out, "{b:02x}");
    }
    out
}

fn parse_sha256_hex(text: &str) -> Option<String> {
    let token = text.split_whitespace().next()?;
    let hex = token.to_ascii_lowercase();
    (hex.len() == 64 && hex.bytes().all(|b| b.is_ascii_hexdigit())).then_some(hex)
}

fn verify_sha256(file: &Path, sha_file: &Path, manifest_sha: Option<&str>) -> Option<String> {
    let data = std::fs::read(file).ok()?;
    let sha_text = std::fs::read_to_string(sha_file).ok()?;
    let expected = parse_sha256_hex(&sha_text)?;
    if let Some(m) = manifest_sha.and_then(parse_sha256_hex) {
        if m != expected {
            return None;
        }
    }
    (sha256_hex(&data) == expected).then_some(expected)
}

/// "none" | "ready 3.1.0" | "failed: why"
pub fn check() -> String {
    if let Some(r) = READY.lock().unwrap().as_ref() {
        return format!("ready {}", r.version);
    }
    let Some(info) = fetch("latest.json")
        .and_then(|p| std::fs::read_to_string(p).ok())
        .and_then(|t| serde_json::from_str::<Value>(&t).ok())
    else {
        return "failed: could not reach GitHub".into();
    };
    let version = info["version"].as_str().unwrap_or("").to_string();
    if !newer(&version, env!("CARGO_PKG_VERSION")) {
        return "none".into();
    }
    let (Some(setup), Some(sha_file)) = (fetch(SETUP_NAME), fetch(SHA_NAME)) else {
        return "failed: download did not finish".into();
    };
    let Some(sha256) = verify_sha256(&setup, &sha_file, info["sha256"].as_str()) else {
        let _ = std::fs::remove_file(&setup);
        sys::log(&format!(
            "update {version}: SHA-256 checksum mismatch, download discarded"
        ));
        return "failed: installer checksum verification failed".into();
    };
    *READY.lock().unwrap() = Some(ReadyUpdate {
        version: version.clone(),
        setup,
        sha256,
    });
    sys::helper(json!({ "t": "update", "version": version }));
    format!("ready {version}")
}
/// Runs the downloaded installer quietly; it closes this copy, replaces it and starts the new one.
/// It is started through cmd's "start", so it is not this app's child and outlives the app it closes.
pub fn install() -> bool {
    let Some(ready) = READY.lock().unwrap().clone() else {
        return false;
    };
    let sha_file = dir().join(SHA_NAME);
    if verify_sha256(&ready.setup, &sha_file, Some(&ready.sha256)).is_none() {
        return false; // the file changed since it was checked
    }
    sys::log(&format!("installing update {}", ready.version));
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    let started = std::process::Command::new("cmd")
        .raw_arg(format!("/c start \"\" \"{}\" /S", ready.setup.display()))
        .creation_flags(CREATE_NO_WINDOW)
        .spawn()
        .is_ok();
    if !started {
        sys::shell_open(&ready.setup.to_string_lossy(), Some("/S"));
    }
    true
}

/// How long nobody has pressed a key or moved the mouse, anywhere (ms).
fn idle_ms() -> u64 {
    unsafe {
        let mut li = LASTINPUTINFO {
            cbSize: std::mem::size_of::<LASTINPUTINFO>() as u32,
            dwTime: 0,
        };
        if !GetLastInputInfo(&mut li).as_bool() {
            return 0;
        }
        GetTickCount().wrapping_sub(li.dwTime) as u64
    }
}
/// The first start after an update says so, once.
fn announce() {
    let now = env!("CARGO_PKG_VERSION");
    let before = settings::get("lastVersion")
        .as_str()
        .unwrap_or("")
        .to_string();
    if before != now {
        settings::put("lastVersion", now.into());
        if !before.is_empty() && newer(now, &before) {
            sys::helper(json!({ "t": "updated", "version": now }));
        }
    }
}

const FRESH: u64 = 3 * 60_000; // the app started this recently: nobody is using it yet
const AWAY: u64 = 3 * 60_000; // nobody has touched the computer for this long
const EVERY: u64 = 6 * 3_600_000; // how often GitHub is asked
const RETRY: u64 = 30 * 60_000; // ... and how soon again after it could not be reached

pub fn start() {
    std::thread::spawn(|| {
        let began = sys::now_ms();
        std::thread::sleep(Duration::from_secs(6)); // the page is listening by now
        announce();
        std::thread::sleep(Duration::from_secs(39));
        let mut next_check = 0u64;
        loop {
            let auto = settings::packaged() && settings::get("autoUpdate") != Value::Bool(false);
            let now = sys::now_ms();
            if auto && now >= next_check {
                let r = check();
                next_check = now
                    + if r.starts_with("failed") {
                        RETRY
                    } else {
                        EVERY
                    };
                if r.starts_with("failed") {
                    sys::log(&format!("update check {r}"));
                }
            }
            if auto && READY.lock().unwrap().is_some() {
                let fresh = sys::now_ms().saturating_sub(began) < FRESH;
                if (fresh || idle_ms() >= AWAY) && install() {
                    return; // the installer closes this copy and starts the new one
                }
            }
            std::thread::sleep(Duration::from_secs(20));
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sha256_matches_nist_vectors() {
        assert_eq!(
            sha256_hex(b""),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
        assert_eq!(
            sha256_hex(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    /// Any packaged installer in dist/ and its .sha256 checksum must verify, while tampered bytes must be rejected.
    #[test]
    fn verifies_sha256_checksum_file() {
        let tmp = std::env::temp_dir().join("pillvyn-sha-test");
        let _ = std::fs::create_dir_all(&tmp);
        let good = tmp.join("good.bin");
        let sha = tmp.join("good.bin.sha256");
        let payload = b"PILLVYN installer payload";
        std::fs::write(&good, payload).unwrap();
        let digest = sha256_hex(payload);
        std::fs::write(&sha, format!("{digest}  PILLVYN-Setup.exe\n")).unwrap();
        assert_eq!(
            verify_sha256(&good, &sha, Some(&digest)),
            Some(digest.clone())
        );
        assert!(verify_sha256(&good, &sha, Some(&"0".repeat(64))).is_none());

        let mut tampered = payload.to_vec();
        tampered[0] ^= 1;
        std::fs::write(&good, tampered).unwrap();
        assert!(verify_sha256(&good, &sha, Some(&digest)).is_none());
        let _ = std::fs::remove_dir_all(&tmp);

        let dist = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("dist");
        let (setup, sha_dist) = (dist.join(SETUP_NAME), dist.join(SHA_NAME));
        if setup.exists() && sha_dist.exists() {
            assert!(verify_sha256(&setup, &sha_dist, None).is_some());
        }
    }

    #[test]
    fn versions_compare_by_number() {
        assert!(newer("3.10.0", "3.9.9"));
        assert!(newer("v3.1.1", "3.1.0"));
        assert!(newer("3.1.0", "3.0.0"));
        assert!(!newer("3.1.0", "3.1.0"));
        assert!(!newer("", "3.1.0"));
    }
}
