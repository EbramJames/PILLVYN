use crate::island::activities::now_ms;
use arboard::Clipboard;
use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClipboardEntry {
    pub id: String,
    pub kind: String, // "text" | "image"
    pub preview: String,
    pub full_text: Option<String>,
    pub image_data_url: Option<String>,
    pub width: Option<usize>,
    pub height: Option<usize>,
    pub char_count: Option<usize>,
    pub timestamp_ms: u64,
    pub pinned: bool,
    pub content_hash: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClipboardSnapshot {
    pub enabled: bool,
    pub items: Vec<ClipboardEntry>,
}

pub struct ClipboardStore {
    items: VecDeque<ClipboardEntry>,
    pub enabled: bool,
    pub skip_sensitive: bool,
    pub max_items: usize,
    last_hash: String,
}

impl ClipboardStore {
    pub fn new(enabled: bool, skip_sensitive: bool, max_items: usize) -> Self {
        Self {
            items: VecDeque::with_capacity(max_items),
            enabled,
            skip_sensitive,
            max_items,
            last_hash: String::new(),
        }
    }

    pub fn update_policy(&mut self, enabled: bool, skip_sensitive: bool, max_items: usize) {
        self.enabled = enabled;
        self.skip_sensitive = skip_sensitive;
        self.max_items = max_items.clamp(5, 100);
        self.trim_to_limit();
    }

    pub fn capture_current(&mut self) -> bool {
        if !self.enabled {
            return false;
        }
        let Ok(mut cb) = Clipboard::new() else {
            return false;
        };

        if let Ok(text) = cb.get_text() {
            let trimmed = text.trim();
            if trimmed.is_empty() {
                return false;
            }
            if self.skip_sensitive && looks_sensitive(trimmed) {
                return false;
            }
            let hash = sha256_hex(text.as_bytes());
            if hash == self.last_hash {
                return false;
            }
            self.last_hash = hash.clone();
            self.items.retain(|e| e.content_hash != hash);

            let preview = if trimmed.chars().count() > 140 {
                let s: String = trimmed.chars().take(140).collect();
                format!("{s}…")
            } else {
                trimmed.to_string()
            };

            let entry = ClipboardEntry {
                id: format!("cb-{}", now_ms()),
                kind: "text".to_string(),
                preview,
                full_text: Some(text.chars().take(16_000).collect()),
                image_data_url: None,
                width: None,
                height: None,
                char_count: Some(text.chars().count()),
                timestamp_ms: now_ms(),
                pinned: false,
                content_hash: hash,
            };
            self.insert_entry(entry);
            return true;
        }

        if let Ok(img) = cb.get_image() {
            let mut hasher = Sha256::new();
            hasher.update(img.width.to_le_bytes());
            hasher.update(img.height.to_le_bytes());
            hasher.update(img.bytes.as_ref());
            let hash = format!("{:x}", hasher.finalize());
            if hash == self.last_hash {
                return false;
            }
            self.last_hash = hash.clone();
            self.items.retain(|e| e.content_hash != hash);

            let bmp = encode_rgba_preview_bmp(img.width, img.height, img.bytes.as_ref(), 240);
            let data_url = format!("data:image/bmp;base64,{}", BASE64.encode(bmp));

            let entry = ClipboardEntry {
                id: format!("cb-{}", now_ms()),
                kind: "image".to_string(),
                preview: format!("Image ({}×{})", img.width, img.height),
                full_text: None,
                image_data_url: Some(data_url),
                width: Some(img.width),
                height: Some(img.height),
                char_count: None,
                timestamp_ms: now_ms(),
                pinned: false,
                content_hash: hash,
            };
            self.insert_entry(entry);
            return true;
        }

        false
    }

    fn insert_entry(&mut self, entry: ClipboardEntry) {
        self.items.push_front(entry);
        self.trim_to_limit();
    }

    fn trim_to_limit(&mut self) {
        while self.items.len() > self.max_items {
            if let Some(pos) = self.items.iter().rposition(|e| !e.pinned) {
                self.items.remove(pos);
            } else {
                self.items.pop_back();
            }
        }
    }

    pub fn restore_item(&mut self, id: &str) -> Result<(), String> {
        let Some(item) = self.items.iter().find(|e| e.id == id).cloned() else {
            return Err("Clipboard entry not found".into());
        };
        if let Some(text) = item.full_text {
            let mut cb = Clipboard::new().map_err(|e| format!("Clipboard unavailable: {e}"))?;
            self.last_hash = item.content_hash;
            cb.set_text(text)
                .map_err(|e| format!("Failed to restore clipboard text: {e}"))?;
            return Ok(());
        }
        Err("Only text entries can currently be restored back to clipboard".into())
    }

    pub fn toggle_pin(&mut self, id: &str) -> bool {
        if let Some(item) = self.items.iter_mut().find(|e| e.id == id) {
            item.pinned = !item.pinned;
            return true;
        }
        false
    }

    pub fn delete_item(&mut self, id: &str) -> bool {
        let before = self.items.len();
        self.items.retain(|e| e.id != id);
        self.items.len() != before
    }

    pub fn clear(&mut self) {
        self.items.clear();
        self.last_hash.clear();
    }

    pub fn snapshot(&self) -> ClipboardSnapshot {
        ClipboardSnapshot {
            enabled: self.enabled,
            items: self.items.iter().cloned().collect(),
        }
    }
}

fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}

pub fn looks_sensitive(text: &str) -> bool {
    let prefixes = [
        "-----BEGIN ",
        "ghp_",
        "gho_",
        "github_pat_",
        "sk-proj-",
        "sk-ant-",
        "AIza",
        "xoxb-",
        "xoxp-",
        "AKIA",
    ];
    prefixes.iter().any(|p| text.contains(p))
}

fn encode_rgba_preview_bmp(src_w: usize, src_h: usize, rgba: &[u8], max_dim: usize) -> Vec<u8> {
    if src_w == 0 || src_h == 0 || rgba.len() < src_w * src_h * 4 {
        return Vec::new();
    }
    let scale = if src_w.max(src_h) > max_dim {
        max_dim as f64 / src_w.max(src_h) as f64
    } else {
        1.0
    };
    let dst_w = ((src_w as f64 * scale).round() as usize).max(1);
    let dst_h = ((src_h as f64 * scale).round() as usize).max(1);
    let row_size = (dst_w * 3).div_ceil(4) * 4;
    let img_size = row_size * dst_h;
    let file_size = 54 + img_size;
    let mut buf = vec![0u8; file_size];

    buf[0] = b'B';
    buf[1] = b'M';
    buf[2..6].copy_from_slice(&(file_size as u32).to_le_bytes());
    buf[10..14].copy_from_slice(&54u32.to_le_bytes());
    buf[14..18].copy_from_slice(&40u32.to_le_bytes());
    buf[18..22].copy_from_slice(&(dst_w as i32).to_le_bytes());
    buf[22..26].copy_from_slice(&(dst_h as i32).to_le_bytes());
    buf[26..28].copy_from_slice(&1u16.to_le_bytes());
    buf[28..30].copy_from_slice(&24u16.to_le_bytes());
    buf[34..38].copy_from_slice(&(img_size as u32).to_le_bytes());

    for dy in 0..dst_h {
        let sy = (dy * src_h / dst_h).min(src_h - 1);
        let bmp_y = dst_h - 1 - dy;
        for dx in 0..dst_w {
            let sx = (dx * src_w / dst_w).min(src_w - 1);
            let s_idx = (sy * src_w + sx) * 4;
            let d_idx = 54 + bmp_y * row_size + dx * 3;
            let a = rgba[s_idx + 3] as f32 / 255.0;
            // Composite against dark glass (#10141E)
            let bg = (16.0, 20.0, 30.0);
            let r = (rgba[s_idx] as f32 * a + bg.0 * (1.0 - a)) as u8;
            let g = (rgba[s_idx + 1] as f32 * a + bg.1 * (1.0 - a)) as u8;
            let b = (rgba[s_idx + 2] as f32 * a + bg.2 * (1.0 - a)) as u8;
            buf[d_idx] = b;
            buf[d_idx + 1] = g;
            buf[d_idx + 2] = r;
        }
    }
    buf
}

/// Spawns an event-driven Windows message-only clipboard listener (`AddClipboardFormatListener`).
#[cfg(target_os = "windows")]
pub fn spawn_clipboard_listener(app: AppHandle, store: Arc<Mutex<ClipboardStore>>) {
    use windows::core::w;
    use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
    use windows::Win32::System::DataExchange::AddClipboardFormatListener;
    use windows::Win32::System::LibraryLoader::GetModuleHandleW;
    use windows::Win32::UI::WindowsAndMessaging::{
        CreateWindowExW, DefWindowProcW, DispatchMessageW, GetMessageW, GetWindowLongPtrW,
        RegisterClassW, SetWindowLongPtrW, TranslateMessage, GWLP_USERDATA, HWND_MESSAGE, MSG,
        WM_CLIPBOARDUPDATE, WNDCLASSW,
    };

    struct ListenerCtx {
        app: AppHandle,
        store: Arc<Mutex<ClipboardStore>>,
    }

    unsafe extern "system" fn wnd_proc(
        hwnd: HWND,
        msg: u32,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> LRESULT {
        if msg == WM_CLIPBOARDUPDATE {
            let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *const ListenerCtx;
            if !ptr.is_null() {
                let ctx = &*ptr;
                if let Ok(mut guard) = ctx.store.lock() {
                    if guard.capture_current() {
                        let snap = guard.snapshot();
                        let _ = ctx.app.emit("pillvyn://clipboard", snap);
                    }
                }
            }
            return LRESULT(0);
        }
        DefWindowProcW(hwnd, msg, wparam, lparam)
    }

    std::thread::spawn(move || unsafe {
        let hinstance = GetModuleHandleW(None).unwrap_or_default();
        let class_name = w!("PillvynClipboardWatcher");
        let wc = WNDCLASSW {
            lpfnWndProc: Some(wnd_proc),
            hInstance: hinstance.into(),
            lpszClassName: class_name,
            ..Default::default()
        };
        RegisterClassW(&wc);

        let hwnd = CreateWindowExW(
            Default::default(),
            class_name,
            w!("PILLVYN Clipboard"),
            Default::default(),
            0,
            0,
            0,
            0,
            HWND_MESSAGE,
            None,
            hinstance,
            None,
        );

        let Ok(hwnd) = hwnd else {
            return;
        };

        let ctx = Box::new(ListenerCtx { app, store });
        let ctx_ptr = Box::into_raw(ctx);
        SetWindowLongPtrW(hwnd, GWLP_USERDATA, ctx_ptr as isize);

        if AddClipboardFormatListener(hwnd).is_err() {
            drop(Box::from_raw(ctx_ptr));
            return;
        }

        let mut msg = MSG::default();
        while GetMessageW(&mut msg, HWND::default(), 0, 0).as_bool() {
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }

        drop(Box::from_raw(ctx_ptr));
    });
}

#[cfg(not(target_os = "windows"))]
pub fn spawn_clipboard_listener(_app: AppHandle, _store: Arc<Mutex<ClipboardStore>>) {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sensitive_filter_catches_tokens() {
        assert!(looks_sensitive("ghp_1234567890abcdef"));
        assert!(looks_sensitive("-----BEGIN OPENSSH PRIVATE KEY-----"));
        assert!(!looks_sensitive("https://github.com/pillvyn/pillvyn"));
    }
}
