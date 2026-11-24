// Native Windows WinHTTP client: HTTPS/HTTP requests, streaming downloads with progress,
// and Microsoft Edge Read-Aloud Neural TTS over native WinHTTP WebSockets (zero extra crates).
use std::ffi::c_void;
use std::io::Write;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use windows::core::{HSTRING, PCWSTR};
use windows::Win32::Networking::WinHttp::*;

use crate::update::sha256_hex;

struct WinHandle(*mut c_void);
impl WinHandle {
    fn new(p: *mut c_void) -> Option<Self> {
        (!p.is_null()).then_some(Self(p))
    }
}
impl Drop for WinHandle {
    fn drop(&mut self) {
        if !self.0.is_null() {
            unsafe {
                let _ = WinHttpCloseHandle(self.0);
            }
        }
    }
}

pub struct ParsedUrl {
    pub https: bool,
    pub host: String,
    pub port: u16,
    pub path: String,
}

pub fn parse_url(url: &str) -> Result<ParsedUrl, String> {
    let (https, rest) = if let Some(r) = url.strip_prefix("https://") {
        (true, r)
    } else if let Some(r) = url.strip_prefix("http://") {
        (false, r)
    } else if let Some(r) = url.strip_prefix("wss://") {
        (true, r)
    } else if let Some(r) = url.strip_prefix("ws://") {
        (false, r)
    } else {
        return Err(format!("unsupported URL scheme: {url}"));
    };
    let (authority, path) = match rest.find('/') {
        Some(i) => (&rest[..i], &rest[i..]),
        None => (rest, "/"),
    };
    let (host, port) = match authority.rfind(':') {
        Some(i) if !authority[..i].contains(']') => {
            let p = authority[i + 1..]
                .parse::<u16>()
                .map_err(|_| format!("invalid port in URL: {url}"))?;
            (&authority[..i], p)
        }
        _ => (authority, if https { 443 } else { 80 }),
    };
    if host.is_empty() {
        return Err(format!("missing host in URL: {url}"));
    }
    Ok(ParsedUrl {
        https,
        host: host.to_string(),
        port,
        path: path.to_string(),
    })
}

fn wide_no_nul(s: &str) -> Vec<u16> {
    s.encode_utf16().collect()
}

pub fn request(
    method: &str,
    url: &str,
    headers: &[(&str, &str)],
    body: Option<&[u8]>,
    timeout_ms: u32,
) -> Result<(u16, Vec<u8>), String> {
    let parsed = parse_url(url)?;
    unsafe {
        let session = WinHandle::new(WinHttpOpen(
            &HSTRING::from("PILLVYN/3.3.0"),
            WINHTTP_ACCESS_TYPE_AUTOMATIC_PROXY,
            PCWSTR::null(),
            PCWSTR::null(),
            0,
        ))
        .ok_or_else(|| "WinHttpOpen failed".to_string())?;

        let t = timeout_ms.min(i32::MAX as u32) as i32;
        let _ = WinHttpSetTimeouts(session.0, t.min(15_000), t.min(15_000), t, t);

        let conn = WinHandle::new(WinHttpConnect(
            session.0,
            &HSTRING::from(parsed.host.as_str()),
            parsed.port,
            0,
        ))
        .ok_or_else(|| format!("WinHttpConnect failed for {}", parsed.host))?;

        let flags = if parsed.https {
            WINHTTP_FLAG_SECURE
        } else {
            WINHTTP_OPEN_REQUEST_FLAGS(0)
        };
        let req = WinHandle::new(WinHttpOpenRequest(
            conn.0,
            &HSTRING::from(method),
            &HSTRING::from(parsed.path.as_str()),
            PCWSTR::null(),
            PCWSTR::null(),
            std::ptr::null(),
            flags,
        ))
        .ok_or_else(|| "WinHttpOpenRequest failed".to_string())?;

        for (k, v) in headers {
            let line = format!("{k}: {v}\r\n");
            let w = wide_no_nul(&line);
            let _ = WinHttpAddRequestHeaders(req.0, &w, WINHTTP_ADDREQ_FLAG_ADD);
        }

        let (ptr, len) = match body {
            Some(b) if !b.is_empty() => (Some(b.as_ptr() as *const c_void), b.len() as u32),
            _ => (None, 0u32),
        };
        WinHttpSendRequest(req.0, None, ptr, len, len, 0)
            .map_err(|e| format!("WinHttpSendRequest failed: {e}"))?;
        WinHttpReceiveResponse(req.0, std::ptr::null_mut())
            .map_err(|e| format!("WinHttpReceiveResponse failed: {e}"))?;

        let mut status: u32 = 0;
        let mut status_len = std::mem::size_of::<u32>() as u32;
        let _ = WinHttpQueryHeaders(
            req.0,
            WINHTTP_QUERY_STATUS_CODE | WINHTTP_QUERY_FLAG_NUMBER,
            PCWSTR::null(),
            Some((&mut status as *mut u32) as *mut c_void),
            &mut status_len,
            std::ptr::null_mut(),
        );

        let mut out = Vec::new();
        let mut buf = [0u8; 16384];
        loop {
            let mut avail = 0u32;
            WinHttpQueryDataAvailable(req.0, &mut avail)
                .map_err(|e| format!("WinHttpQueryDataAvailable failed: {e}"))?;
            if avail == 0 {
                break;
            }
            let to_read = (avail as usize).min(buf.len()) as u32;
            let mut read = 0u32;
            WinHttpReadData(req.0, buf.as_mut_ptr() as *mut c_void, to_read, &mut read)
                .map_err(|e| format!("WinHttpReadData failed: {e}"))?;
            if read == 0 {
                break;
            }
            out.extend_from_slice(&buf[..read as usize]);
        }
        Ok((status as u16, out))
    }
}

pub fn download_file(
    url: &str,
    dest: &Path,
    mut on_progress: impl FnMut(u64, u64),
) -> Result<u64, String> {
    if let Some(parent) = dest.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let parsed = parse_url(url)?;
    unsafe {
        let session = WinHandle::new(WinHttpOpen(
            &HSTRING::from("PILLVYN/3.3.0"),
            WINHTTP_ACCESS_TYPE_AUTOMATIC_PROXY,
            PCWSTR::null(),
            PCWSTR::null(),
            0,
        ))
        .ok_or_else(|| "WinHttpOpen failed".to_string())?;
        let _ = WinHttpSetTimeouts(session.0, 15_000, 20_000, 30_000, 60_000);

        let conn = WinHandle::new(WinHttpConnect(
            session.0,
            &HSTRING::from(parsed.host.as_str()),
            parsed.port,
            0,
        ))
        .ok_or_else(|| format!("WinHttpConnect failed for {}", parsed.host))?;

        let flags = if parsed.https {
            WINHTTP_FLAG_SECURE
        } else {
            WINHTTP_OPEN_REQUEST_FLAGS(0)
        };
        let req = WinHandle::new(WinHttpOpenRequest(
            conn.0,
            &HSTRING::from("GET"),
            &HSTRING::from(parsed.path.as_str()),
            PCWSTR::null(),
            PCWSTR::null(),
            std::ptr::null(),
            flags,
        ))
        .ok_or_else(|| "WinHttpOpenRequest failed".to_string())?;

        WinHttpSendRequest(req.0, None, None, 0, 0, 0)
            .map_err(|e| format!("download send failed: {e}"))?;
        WinHttpReceiveResponse(req.0, std::ptr::null_mut())
            .map_err(|e| format!("download response failed: {e}"))?;

        let mut status: u32 = 0;
        let mut status_len = std::mem::size_of::<u32>() as u32;
        let _ = WinHttpQueryHeaders(
            req.0,
            WINHTTP_QUERY_STATUS_CODE | WINHTTP_QUERY_FLAG_NUMBER,
            PCWSTR::null(),
            Some((&mut status as *mut u32) as *mut c_void),
            &mut status_len,
            std::ptr::null_mut(),
        );
        if status >= 400 {
            return Err(format!("HTTP {status} downloading {url}"));
        }

        let mut total: u64 = 0;
        let mut total_len = std::mem::size_of::<u64>() as u32;
        let _ = WinHttpQueryHeaders(
            req.0,
            WINHTTP_QUERY_CONTENT_LENGTH | WINHTTP_QUERY_FLAG_NUMBER64,
            PCWSTR::null(),
            Some((&mut total as *mut u64) as *mut c_void),
            &mut total_len,
            std::ptr::null_mut(),
        );

        let part = dest.with_extension("part");
        let mut file = std::fs::File::create(&part)
            .map_err(|e| format!("failed to create {}: {e}", part.display()))?;
        let mut got = 0u64;
        let mut last_cb = 0u64;
        let mut buf = [0u8; 65536];
        loop {
            let mut avail = 0u32;
            WinHttpQueryDataAvailable(req.0, &mut avail)
                .map_err(|e| format!("download query failed: {e}"))?;
            if avail == 0 {
                break;
            }
            let to_read = (avail as usize).min(buf.len()) as u32;
            let mut read = 0u32;
            WinHttpReadData(req.0, buf.as_mut_ptr() as *mut c_void, to_read, &mut read)
                .map_err(|e| format!("download read failed: {e}"))?;
            if read == 0 {
                break;
            }
            file.write_all(&buf[..read as usize])
                .map_err(|e| format!("disk write failed: {e}"))?;
            got += read as u64;
            if got.saturating_sub(last_cb) >= 131_072 || (total > 0 && got >= total) {
                last_cb = got;
                on_progress(got, total);
            }
        }
        drop(file);
        on_progress(got, total.max(got));
        let _ = std::fs::remove_file(dest);
        std::fs::rename(&part, dest)
            .map_err(|e| format!("rename to {} failed: {e}", dest.display()))?;
        Ok(got)
    }
}

// ---------------------------------------------------------------- Microsoft Edge Neural TTS (WSS, Sec-MS-GEC v2)
const EDGE_TOKEN: &str = "6A5AA1D4EAFF4E9FB37E23D68491D6F4";
const EDGE_VERSION: &str = "2-143.0.3650.75";
const EDGE_UA: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/143.0.0.0 Safari/537.36 Edg/143.0.0.0";

const EDGE_SHA_INITIAL: [u32; 8] = [
    0x6A09_E667,
    0xBB67_AE85,
    0x3C6E_F372,
    0xA54F_F53A,
    0x510E_527F,
    0x9B05_688C,
    0x1F83_D9AB,
    0x5BE0_CD19,
];
const EDGE_SHA_STATE: [u32; 8] = [
    0xFD9F_BEA2,
    0x9124_3CE7,
    0xBE49_9094,
    0xCD2E_1E0A,
    0x6C00_6F49,
    0x04BD_35AB,
    0xFEB0_A7E4,
    0x03EF_FBD2,
];
const EDGE_SHA_SCHED: [u32; 14] = [
    0xEB24_BF9E,
    0x1558_691F,
    0xA0C0_9EE2,
    0x0CFA_32BA,
    0xA622_EE8C,
    0xD1DD_9491,
    0xC0DD_25A0,
    0xF9E7_0AB6,
    0xF0C3_1573,
    0x7058_CE4E,
    0x7188_E606,
    0x00E5_AB33,
    0xD258_B060,
    0x7276_6564,
];
const EDGE_SHA_K: [u32; 64] = [
    0x428A_2F98,
    0x7137_4491,
    0xB5C0_FBCF,
    0xE9B5_DBA5,
    0x3956_C25B,
    0x59F1_11F1,
    0x923F_82A4,
    0xAB1C_5ED5,
    0xD807_AA98,
    0x1283_5B01,
    0x2431_85BE,
    0x550C_7DC3,
    0x72BE_5D74,
    0x80DE_B1FE,
    0x9BDC_06A7,
    0xC19B_F174,
    0xE49B_69C1,
    0xEFBE_4786,
    0x0FC1_9DC6,
    0x240C_A1CC,
    0x2DE9_2C6F,
    0x4A74_84AA,
    0x5CB0_A9DC,
    0x76F9_88DA,
    0x983E_5152,
    0xA831_C66D,
    0xB003_27C8,
    0xBF59_7FC7,
    0xC6E0_0BF3,
    0xD5A7_9147,
    0x06CA_6351,
    0x1429_2967,
    0x27B7_0A85,
    0x2E1B_2138,
    0x4D2C_6DFC,
    0x5338_0D13,
    0x650A_7354,
    0x766A_0ABB,
    0x81C2_C92E,
    0x9272_2C85,
    0xA2BF_E8A1,
    0xA81A_664B,
    0xC24B_8B70,
    0xC76C_51A3,
    0xD192_E819,
    0xD699_0624,
    0xF40E_3585,
    0x106A_A070,
    0x19A4_C116,
    0x1E37_6C08,
    0x2748_774C,
    0x34B0_BCB5,
    0x391C_0CB3,
    0x4ED8_AA4A,
    0x5B9C_CA4F,
    0x682E_6FF3,
    0x748F_82EE,
    0x78A5_636F,
    0x84C8_7814,
    0x8CC7_0208,
    0x90BE_FFFA,
    0xA450_6CEB,
    0xBEF9_A3F7,
    0xC671_78F2,
];
const EDGE_SHA_ADJ: u32 = 0xC9DC_06A7;
const EDGE_CS64_COEFF: [(u32, u32); 14] = [
    (0xE82E_91B1, 0xB327_00D0),
    (0x44DE_24C6, 0x88DF_F51D),
    (0x3BEB_65BC, 0x2951_4038),
    (0xC89C_5933, 0xB328_CBDF),
    (0x8D1E_C211, 0xD405_B4D5),
    (0x314E_F5D5, 0x26E5_5F3C),
    (0xE924_91B1, 0x6078_01C6),
    (0xF0A5_B9D5, 0x82B2_C1FB),
    (0x0606_4CF3, 0xFA51_A3DA),
    (0xDA79_FA60, 0x207C_C464),
    (0xBED2_0560, 0x3C3C_74F3),
    (0x89F9_C9F8, 0xD2D1_4B61),
    (0x35FA_38C6, 0xBB04_A2C1),
    (0xCB49_E3C6, 0xC562_6167),
];

fn edge_sigma0(v: u32) -> u32 {
    v.rotate_right(7) ^ v.rotate_right(18) ^ (v >> 3)
}
fn edge_sigma1(v: u32) -> u32 {
    v.rotate_right(17) ^ v.rotate_right(19) ^ (v >> 10)
}
fn edge_sha_rounds(state: [u32; 8], w: &[u32; 64], first_round: usize) -> [u32; 8] {
    let [mut a, mut b, mut c, mut d, mut e, mut f, mut g, mut h] = state;
    for (i, &word) in w.iter().enumerate().skip(first_round) {
        let k = if first_round == 14 && i == 14 {
            EDGE_SHA_ADJ
        } else {
            EDGE_SHA_K[i]
        };
        let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
        let ch = g ^ (e & (f ^ g));
        let t1 = h
            .wrapping_add(s1)
            .wrapping_add(ch)
            .wrapping_add(k)
            .wrapping_add(word);
        let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
        let maj = (a & b) | (c & (a | b));
        let t2 = s0.wrapping_add(maj);
        h = g;
        g = f;
        f = e;
        e = d.wrapping_add(t1);
        d = c;
        c = b;
        b = a;
        a = t1.wrapping_add(t2);
    }
    [a, b, c, d, e, f, g, h]
}
fn edge_expand_sched(w: &mut [u32; 64], first_word: usize) {
    for i in first_word..64 {
        w[i] = edge_sigma1(w[i - 2])
            .wrapping_add(w[i - 7])
            .wrapping_add(edge_sigma0(w[i - 15]))
            .wrapping_add(w[i - 16]);
    }
}
fn edge_sha256_v2(data: &[u8]) -> [u8; 32] {
    let mut w0 = [0u32; 64];
    w0[14] = u32::from_be_bytes([0, data[0], data[1], data[2]]);
    w0[15] = u32::from_be_bytes([data[3], data[4], data[5], data[6]]);
    for i in 16..30 {
        let mut val = EDGE_SHA_SCHED[i - 16];
        if i - 2 >= 14 {
            let s1 = edge_sigma1(w0[i - 2]);
            val = val.wrapping_add(if i == 16 { s1 ^ 0x000B_92C0 } else { s1 });
        }
        if i - 7 >= 14 {
            val = val.wrapping_add(w0[i - 7]);
        }
        if i - 15 >= 14 {
            let s0 = edge_sigma0(w0[i - 15]);
            val = val.wrapping_add(if i == 29 { s0 ^ 0x059C_0B80 } else { s0 });
        }
        w0[i] = val;
    }
    w0[30] = edge_sigma1(w0[28])
        .wrapping_add(w0[23])
        .wrapping_add(edge_sigma0(w0[15]))
        .wrapping_add(w0[14])
        .wrapping_add(EDGE_SHA_ADJ)
        .wrapping_sub(EDGE_SHA_K[14]);
    edge_expand_sched(&mut w0, 31);
    let working0 = edge_sha_rounds(EDGE_SHA_STATE, &w0, 14);
    let mut state = [0u32; 8];
    for i in 0..8 {
        state[i] = EDGE_SHA_INITIAL[i].wrapping_add(working0[i]);
    }

    let length = 57u64 + data.len() as u64;
    let mut padded = Vec::with_capacity(data.len() + 72);
    padded.extend_from_slice(&data[7..]);
    padded.push(0x80);
    while !(padded.len() + 8).is_multiple_of(64) {
        padded.push(0);
    }
    padded.extend_from_slice(&(length * 8).to_be_bytes());
    for block in padded.as_chunks::<64>().0 {
        let mut w = [0u32; 64];
        for (i, slot) in w.iter_mut().take(16).enumerate() {
            let b = i * 4;
            *slot = u32::from_be_bytes([block[b], block[b + 1], block[b + 2], block[b + 3]]);
        }
        edge_expand_sched(&mut w, 16);
        let working = edge_sha_rounds(state, &w, 0);
        for i in 0..8 {
            state[i] = state[i].wrapping_add(working[i]);
        }
    }
    let mut out = [0u8; 32];
    for (i, word) in state.iter().enumerate() {
        out[i * 4..i * 4 + 4].copy_from_slice(&word.to_be_bytes());
    }
    out
}
fn edge_cs64_v2(data: &[u8], digest: &[u8; 32]) -> (u32, u32) {
    let key_a = u32::from_le_bytes([digest[0], digest[1], digest[2], digest[3]]) | 1;
    let key_b = u32::from_le_bytes([digest[4], digest[5], digest[6], digest[7]]) | 1;
    let factors = [
        key_a.wrapping_mul(0x8289_3933),
        key_b.wrapping_mul(0x8289_3933),
    ];
    let mut terms: Vec<u32> = EDGE_CS64_COEFF
        .iter()
        .enumerate()
        .map(|(i, &(coeff, _))| coeff.wrapping_mul(factors[i % 2]))
        .collect();
    let mut offsets: Vec<u32> = EDGE_CS64_COEFF.iter().map(|&(_, off)| off).collect();

    let mut raw = Vec::with_capacity(data.len() + 8);
    raw.push(0u8);
    raw.extend_from_slice(data);
    while !raw.len().is_multiple_of(4) {
        raw.push(0);
    }
    let mut words: Vec<u32> = raw
        .as_chunks::<4>()
        .0
        .iter()
        .map(|b| u32::from_le_bytes(*b))
        .collect();
    if !words.len().is_multiple_of(2) {
        words.push(0);
    }
    for (i, &w) in words.iter().enumerate() {
        let mut term = w.wrapping_mul(if i % 2 == 0 { key_a } else { key_b });
        let mut off =
            w.rotate_left(16)
                .wrapping_mul(if i % 2 == 0 { 0xEF05_69FB } else { 0xC317_13DB });
        if i == 0 {
            term = term.wrapping_add(0xF2F9_0B1Au32.wrapping_mul(factors[0]));
            off = off.wrapping_add(0x0B1A_0000);
        }
        terms.push(term);
        offsets.push(off);
    }

    let (mut swap_acc, mut swap_last, mut st, mut acc) = (0u32, 0u32, 0u32, 0u32);
    let mut i = 0usize;
    while i + 1 < terms.len() {
        let (x, y) = (terms[i], terms[i + 1]);
        let t = offsets[i].wrapping_add(x).rotate_left(16);
        let q = t.wrapping_mul(0x76F5_0F34);
        swap_acc =
            swap_acc.wrapping_add(q.rotate_left(16).wrapping_mul(0xC3EF_EA97).wrapping_add(q));
        let t2 = offsets[i + 1].wrapping_add(y).rotate_left(16);
        let q2 = t2.wrapping_mul(0x1881_CE3C);
        swap_last = q2
            .rotate_left(16)
            .wrapping_mul(0x35BD_1EC9)
            .wrapping_add(q2);
        swap_acc = swap_acc.wrapping_add(swap_last);

        st = st.wrapping_mul(key_a).wrapping_add(x);
        st = st.rotate_left(16).wrapping_mul(0xCF98_B111);
        st = st.rotate_left(16).wrapping_mul(0x8708_5B9F);
        st = st.rotate_left(16).wrapping_mul(0x12CE_B96D);
        st = st.rotate_left(16).wrapping_mul(0x257E_1D83);
        acc = acc.wrapping_add(st);
        let mut u = st.wrapping_mul(key_b).wrapping_add(y);
        u = u.rotate_left(16).wrapping_mul(0xA274_16F5);
        u = u.rotate_left(16).wrapping_mul(0xD383_96FF);
        st = u
            .rotate_left(16)
            .wrapping_mul(0x7C93_2B89)
            .rotate_left(16)
            .wrapping_mul(0xBFA4_9F69);
        acc = acc.wrapping_add(st);
        i += 2;
    }
    (swap_last ^ st, swap_acc ^ acc)
}
pub(crate) fn edge_sec_ms_gec_v2(unix_secs: u64) -> String {
    use std::fmt::Write;
    const WIN_EPOCH: u64 = 11_644_473_600;
    let ticks = ((unix_secs + WIN_EPOCH) / 300) * 300 * 10_000_000;
    let msg = format!("{ticks}{EDGE_TOKEN}");
    let digest = edge_sha256_v2(msg.as_bytes());
    let (c0, c1) = edge_cs64_v2(msg.as_bytes(), &digest);
    let mut out = String::with_capacity(81);
    for b in c0.to_le_bytes().into_iter().chain(c1.to_le_bytes()) {
        let _ = write!(&mut out, "{b:02X}");
    }
    out.push('-');
    for b in digest {
        let _ = write!(&mut out, "{b:02X}");
    }
    out
}
fn edge_sec_ms_gec() -> String {
    let unix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    edge_sec_ms_gec_v2(unix)
}

fn rand_hex_16() -> String {
    let t = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let pid = std::process::id() as u128;
    let h = sha256_hex(format!("{t}:{pid}:pillvyn-tts").as_bytes());
    h[..32].to_string()
}

fn escape_xml(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            c if c.is_control() && c != '\n' && c != '\t' => out.push(' '),
            c => out.push(c),
        }
    }
    out
}

pub(crate) fn build_edge_ssml(text: &str, voice: &str, rate: &str, volume: &str) -> String {
    let safe_text = escape_xml(text.trim());
    let safe_voice = escape_xml(voice);
    let safe_rate = escape_xml(rate);
    let safe_vol = escape_xml(volume);
    format!(
        "<speak version='1.0' xmlns='http://www.w3.org/2001/10/synthesis' xml:lang='en-US'>\
         <voice name='{safe_voice}'>\
         <prosody pitch='+0Hz' rate='{safe_rate}' volume='{safe_vol}'>{safe_text}</prosody>\
         </voice></speak>"
    )
}

/// Extracts raw MP3 payload from an Edge TTS binary WebSocket message (`[u16be hdr_len][headers][mp3]`).
pub(crate) fn extract_edge_audio_chunk(frame: &[u8]) -> Option<&[u8]> {
    if frame.len() < 2 {
        return None;
    }
    let hdr_len = u16::from_be_bytes([frame[0], frame[1]]) as usize;
    let body_start = 2 + hdr_len;
    if body_start > frame.len() {
        return None;
    }
    let hdr = std::str::from_utf8(&frame[2..body_start]).ok()?;
    if hdr.contains("Path:audio") {
        Some(&frame[body_start..])
    } else {
        None
    }
}

/// Synthesizes Persian or English speech using Microsoft Edge Read-Aloud Neural TTS into MP3 bytes.
pub fn edge_tts_mp3(text: &str, voice: &str, rate: &str, volume: &str) -> Result<Vec<u8>, String> {
    let clean = text.trim();
    if clean.is_empty() {
        return Err("empty TTS text".into());
    }
    let gec = edge_sec_ms_gec();
    let conn_id = rand_hex_16();
    let muid = rand_hex_16().to_ascii_uppercase();
    let path = format!(
        "/consumer/speech/synthesize/readaloud/edge/v1?TrustedClientToken={EDGE_TOKEN}&Sec-MS-GEC={gec}&Sec-MS-GEC-Version={EDGE_VERSION}&ConnectionId={conn_id}"
    );

    unsafe {
        let session = WinHandle::new(WinHttpOpen(
            &HSTRING::from(EDGE_UA),
            WINHTTP_ACCESS_TYPE_AUTOMATIC_PROXY,
            PCWSTR::null(),
            PCWSTR::null(),
            0,
        ))
        .ok_or_else(|| "Edge TTS WinHttpOpen failed".to_string())?;
        let _ = WinHttpSetTimeouts(session.0, 10_000, 10_000, 15_000, 20_000);

        let conn = WinHandle::new(WinHttpConnect(
            session.0,
            &HSTRING::from("speech.platform.bing.com"),
            443,
            0,
        ))
        .ok_or_else(|| "Edge TTS WinHttpConnect failed".to_string())?;

        let req = WinHandle::new(WinHttpOpenRequest(
            conn.0,
            &HSTRING::from("GET"),
            &HSTRING::from(path.as_str()),
            PCWSTR::null(),
            PCWSTR::null(),
            std::ptr::null(),
            WINHTTP_FLAG_SECURE,
        ))
        .ok_or_else(|| "Edge TTS WinHttpOpenRequest failed".to_string())?;

        let extra_headers = format!(
            "Origin: chrome-extension://jdiccldimpdaibmpdkjnbmckianbfold\r\n\
             Pragma: no-cache\r\n\
             Cache-Control: no-cache\r\n\
             Accept-Encoding: gzip, deflate, br\r\n\
             Accept-Language: en-US,en;q=0.9\r\n\
             Cookie: muid={muid};\r\n"
        );
        let hw = wide_no_nul(&extra_headers);
        let _ = WinHttpAddRequestHeaders(req.0, &hw, WINHTTP_ADDREQ_FLAG_ADD);

        WinHttpSetOption(Some(req.0), WINHTTP_OPTION_UPGRADE_TO_WEB_SOCKET, None)
            .map_err(|e| format!("Edge TTS WS upgrade option failed: {e}"))?;
        WinHttpSendRequest(req.0, None, None, 0, 0, 0)
            .map_err(|e| format!("Edge TTS handshake send failed: {e}"))?;
        WinHttpReceiveResponse(req.0, std::ptr::null_mut())
            .map_err(|e| format!("Edge TTS handshake response failed: {e}"))?;

        let mut ws_status: u32 = 0;
        let mut ws_status_len = std::mem::size_of::<u32>() as u32;
        let _ = WinHttpQueryHeaders(
            req.0,
            WINHTTP_QUERY_STATUS_CODE | WINHTTP_QUERY_FLAG_NUMBER,
            PCWSTR::null(),
            Some((&mut ws_status as *mut u32) as *mut c_void),
            &mut ws_status_len,
            std::ptr::null_mut(),
        );
        if ws_status != 101 {
            return Err(format!("Edge TTS WebSocket rejected (HTTP {ws_status})"));
        }

        let ws = WinHandle::new(WinHttpWebSocketCompleteUpgrade(req.0, None))
            .ok_or_else(|| "Edge TTS WebSocket upgrade failed".to_string())?;
        drop(req);

        let ts = "Thu, 01 Jan 2026 00:00:00 GMT";
        let cfg_msg = format!(
            "X-Timestamp:{ts}\r\nContent-Type:application/json; charset=utf-8\r\nPath:speech.config\r\n\r\n\
             {{\"context\":{{\"synthesis\":{{\"audio\":{{\"metadataoptions\":{{\"sentenceBoundaryEnabled\":\"false\",\"wordBoundaryEnabled\":\"false\"}},\"outputFormat\":\"audio-24khz-48kbitrate-mono-mp3\"}}}}}}}}"
        );
        let rc = WinHttpWebSocketSend(
            ws.0,
            WINHTTP_WEB_SOCKET_UTF8_MESSAGE_BUFFER_TYPE,
            Some(cfg_msg.as_bytes()),
        );
        if rc != 0 {
            return Err(format!("Edge TTS config send failed ({rc})"));
        }

        let ssml = build_edge_ssml(clean, voice, rate, volume);
        let ssml_msg = format!(
            "X-RequestId:{conn_id}\r\nContent-Type:application/ssml+xml\r\nX-Timestamp:{ts}Z\r\nPath:ssml\r\n\r\n{ssml}"
        );
        let rc2 = WinHttpWebSocketSend(
            ws.0,
            WINHTTP_WEB_SOCKET_UTF8_MESSAGE_BUFFER_TYPE,
            Some(ssml_msg.as_bytes()),
        );
        if rc2 != 0 {
            return Err(format!("Edge TTS SSML send failed ({rc2})"));
        }

        let mut mp3 = Vec::with_capacity(32_768);
        let mut msg_buf = Vec::with_capacity(16_384);
        let mut chunk = [0u8; 16_384];
        loop {
            let mut read = 0u32;
            let mut btype = WINHTTP_WEB_SOCKET_BUFFER_TYPE(0);
            let err = WinHttpWebSocketReceive(
                ws.0,
                chunk.as_mut_ptr() as *mut c_void,
                chunk.len() as u32,
                &mut read,
                &mut btype,
            );
            if err != 0 {
                if !mp3.is_empty() {
                    break;
                }
                return Err(format!("Edge TTS receive error ({err})"));
            }
            if btype == WINHTTP_WEB_SOCKET_CLOSE_BUFFER_TYPE {
                break;
            }
            msg_buf.extend_from_slice(&chunk[..read as usize]);
            if btype == WINHTTP_WEB_SOCKET_BINARY_MESSAGE_BUFFER_TYPE {
                if let Some(audio) = extract_edge_audio_chunk(&msg_buf) {
                    mp3.extend_from_slice(audio);
                }
                msg_buf.clear();
            } else if btype == WINHTTP_WEB_SOCKET_UTF8_MESSAGE_BUFFER_TYPE {
                let done = std::str::from_utf8(&msg_buf)
                    .map(|t| t.contains("Path:turn.end"))
                    .unwrap_or(false);
                msg_buf.clear();
                if done {
                    break;
                }
            }
        }
        let _ = WinHttpWebSocketClose(
            ws.0,
            WINHTTP_WEB_SOCKET_SUCCESS_CLOSE_STATUS.0 as u16,
            None,
            0,
        );
        if mp3.is_empty() {
            return Err("Edge TTS returned empty audio".into());
        }
        Ok(mp3)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_http_and_https_urls() {
        let p = parse_url("https://openrouter.ai/api/v1/chat/completions").unwrap();
        assert!(p.https);
        assert_eq!(p.host, "openrouter.ai");
        assert_eq!(p.port, 443);
        assert_eq!(p.path, "/api/v1/chat/completions");

        let l = parse_url("http://127.0.0.1:18492/inference").unwrap();
        assert!(!l.https);
        assert_eq!(l.host, "127.0.0.1");
        assert_eq!(l.port, 18492);
        assert_eq!(l.path, "/inference");
    }

    #[test]
    fn escapes_ssml_and_extracts_binary_audio_frame() {
        let ssml = build_edge_ssml("5 < 10 & \"ok\"", "fa-IR-DilaraNeural", "+10%", "+0%");
        assert!(ssml.contains("5 &lt; 10 &amp; &quot;ok&quot;"));
        assert!(ssml.contains("fa-IR-DilaraNeural"));

        let hdr = b"X-RequestId:abc\r\nContent-Type:audio/mpeg\r\nPath:audio\r\n";
        let mut frame = Vec::new();
        frame.extend_from_slice(&(hdr.len() as u16).to_be_bytes());
        frame.extend_from_slice(hdr);
        frame.extend_from_slice(&[0xFF, 0xFB, 0x90, 0x00]);
        assert_eq!(
            extract_edge_audio_chunk(&frame),
            Some(&[0xFF, 0xFB, 0x90, 0x00][..])
        );

        let tok = edge_sec_ms_gec_v2(1_700_000_000);
        let (left, right) = tok.split_once('-').expect("v2 token contains hyphen");
        assert_eq!(left.len(), 16);
        assert_eq!(right.len(), 64);
        assert!(left.bytes().all(|b| b.is_ascii_hexdigit()));
        assert!(right.bytes().all(|b| b.is_ascii_hexdigit()));
    }
}
