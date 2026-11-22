// Settings (one small JSON file) and the "Start with Windows" entry.
use std::ffi::c_void;
use std::sync::atomic::Ordering::SeqCst;
use std::sync::{Mutex, OnceLock};

use serde_json::{json, Map, Value};
use windows::core::HSTRING;
use windows::Win32::Foundation::ERROR_SUCCESS;
use windows::Win32::System::Registry::*;

use crate::sys;

type TraySyncFn = Box<dyn Fn(bool) + Send + Sync>;

static SETTINGS: Mutex<Option<Map<String, Value>>> = Mutex::new(None);
static TRAY_AUTO_SYNC: OnceLock<TraySyncFn> = OnceLock::new();

pub fn register_tray_autostart_sync(f: impl Fn(bool) + Send + Sync + 'static) {
    let _ = TRAY_AUTO_SYNC.set(Box::new(f));
}

const THEMES: [&str; 14] = [
    "glass", "frost", "black", "aurora", "nebula", "crystal", "solar", "venom", "ember", "reactor",
    "cloud", "water", "lava", "ice",
];
const FACES: [&str; 3] = ["eyes", "clock", "logo"];
const BOOLS: [&str; 17] = [
    "autoUpdate",
    "musicReact",
    "shimmer",
    "sounds",
    "netAlerts",
    "sensorDot",
    "smartCopy",
    "autoHide",
    "hideVideo",
    "copyPreview",
    "clipHistory",
    "deviceAlerts",
    "downloadAlerts",
    "shelf",
    "visuals",
    "noteHotkey",
    "vaultSymbols",
];
const CALENDARS: [&str; 4] = ["gregorian", "shamsi", "shamsi-fa", "both"];

fn defaults() -> Map<String, Value> {
    json!({
        "autoHide": true, "dndHint": false, "toured": false, "calendar": "gregorian", "hideVideo": true,
        "copyPreview": true, "clipHistory": true, "deviceAlerts": true, "downloadAlerts": true, "shelf": true, "visuals": true,
        "notifSeconds": 4,          // how long a notification stays on the pill
        "topOffset": 0,             // extra vertical offset from the top screen edge (0..=24 px)
        "face": "eyes",             // what the resting pill shows: "eyes", "clock", or "logo"
        "noteHotkey": true,         // Ctrl+Alt+N opens the quick note from anywhere
        "vaultLength": 18, "vaultSymbols": true, "lastUser": "",
        "skin": "glass",            // the resting pill: liquid glass, frosted pearl, obsidian, or an animated theme
        "musicTheme": "sky",        // what plays behind the track: "sky" or an animated skin
        "shimmer": true,            // glass rim glint & state-change shimmer sweep
        "sounds": true,             // subtle UI micro-acoustic feedback chimes
        "netAlerts": true, "sensorDot": true, "smartCopy": true, "musicReact": true, "autoUpdate": true,
    })
    .as_object()
    .cloned()
    .unwrap_or_default()
}

fn valid_skin(v: &str) -> bool {
    THEMES.contains(&v)
}
fn valid_face(v: &str) -> bool {
    FACES.contains(&v)
}
fn valid_calendar(v: &str) -> bool {
    CALENDARS.contains(&v)
}
fn valid_notif_seconds(v: f64) -> Option<i64> {
    let n = v.round() as i64;
    (2..=15).contains(&n).then_some(n)
}
fn valid_top_offset(v: f64) -> Option<i64> {
    let n = v.round() as i64;
    (0..=24).contains(&n).then_some(n)
}
fn valid_vault_length(v: f64) -> Option<i64> {
    let n = v.round() as i64;
    (8..=40).contains(&n).then_some(n)
}

pub fn top_offset() -> isize {
    get("topOffset")
        .as_f64()
        .and_then(valid_top_offset)
        .unwrap_or(0) as isize
}
fn file() -> std::path::PathBuf {
    sys::data_dir().join("settings.json")
}
fn with<T>(f: impl FnOnce(&mut Map<String, Value>) -> T) -> T {
    let mut g = SETTINGS.lock().unwrap();
    f(g.get_or_insert_with(defaults))
}

pub fn load() {
    let mut map = defaults();
    if let Ok(Value::Object(saved)) = std::fs::read_to_string(file())
        .map_err(|_| ())
        .and_then(|t| serde_json::from_str::<Value>(&t).map_err(|_| ()))
    {
        for (k, v) in saved {
            map.insert(k, v);
        }
    }
    *SETTINGS.lock().unwrap() = Some(map);
}
fn save() {
    let text = with(|m| serde_json::to_string(m).unwrap_or_default());
    let _ = std::fs::write(file(), text);
}
pub fn get(key: &str) -> Value {
    with(|m| m.get(key).cloned().unwrap_or(Value::Null))
}
pub fn flag(key: &str) -> bool {
    get(key).as_bool().unwrap_or(false)
}
pub fn put(key: &str, value: Value) {
    with(|m| m.insert(key.to_string(), value));
    save();
}
pub fn payload() -> Value {
    let mut map = with(|m| m.clone());
    map.insert("autoStartOn".into(), autostart_on().into());
    map.insert("version".into(), env!("CARGO_PKG_VERSION").into());
    map.insert("appName".into(), sys::APP_NAME.into());
    Value::Object(map)
}
pub fn broadcast() {
    sys::emit("settings", payload());
}

/// A change asked for by the page: only known keys with sane values are accepted.
pub fn apply(key: &str, value: &Value) {
    match key {
        "autoStart" => {
            let on = value.as_bool().unwrap_or(false);
            put("autoStart", on.into());
            set_autostart(on);
            if let Some(sync) = TRAY_AUTO_SYNC.get() {
                sync(autostart_on());
            }
        }
        "calendar" => {
            if let Some(v) = value.as_str().filter(|v| valid_calendar(v)) {
                put(key, v.into());
            }
        }
        "face" => {
            if let Some(v) = value.as_str().filter(|v| valid_face(v)) {
                put(key, v.into());
            }
        }
        "skin" => {
            if let Some(v) = value.as_str().filter(|v| valid_skin(v)) {
                put(key, v.into());
            }
        }
        "musicTheme" => {
            if let Some(v) = value
                .as_str()
                .filter(|v| *v == "sky" || (valid_skin(v) && *v != "black"))
            {
                put(key, v.into());
            }
        }
        "notifSeconds" => {
            if let Some(n) = value.as_f64().and_then(valid_notif_seconds) {
                put(key, n.into());
            }
        }
        "topOffset" => {
            if let Some(n) = value.as_f64().and_then(valid_top_offset) {
                put(key, n.into());
                crate::hit::OFFSET_Y.store(n as isize, SeqCst);
                crate::hit::reposition();
            }
        }
        "vaultLength" => {
            if let Some(n) = value.as_f64().and_then(valid_vault_length) {
                put(key, n.into());
            }
        }
        "dndHint" | "toured" => put(key, true.into()),
        _ if BOOLS.contains(&key) => put(key, value.as_bool().unwrap_or(false).into()),
        _ => return,
    }
    crate::hit::HOTKEY_WANTED.store(flag("noteHotkey"), std::sync::atomic::Ordering::SeqCst);
    broadcast();
}

// ---------------------------------------------------------------- start with Windows
// Our own value under the user's Run key, written and read back directly.
const RUN_KEY: &str = "Software\\Microsoft\\Windows\\CurrentVersion\\Run";

fn exe() -> String {
    std::env::current_exe()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_default()
}
pub fn packaged() -> bool {
    sys::debug_dir().is_none() && !exe().to_ascii_lowercase().contains("\\target\\")
}
pub fn autostart_on() -> bool {
    unsafe {
        let mut buf = [0u16; 600];
        let mut n = (buf.len() * 2) as u32;
        let r = RegGetValueW(
            HKEY_CURRENT_USER,
            &HSTRING::from(RUN_KEY),
            &HSTRING::from(sys::APP_NAME),
            RRF_RT_REG_SZ,
            None,
            Some(buf.as_mut_ptr() as *mut c_void),
            Some(&mut n),
        );
        r == ERROR_SUCCESS
            && sys::from_wide(&buf)
                .to_lowercase()
                .contains(&exe().to_lowercase())
    }
}
pub fn set_autostart(on: bool) {
    unsafe {
        if on {
            let cmd: Vec<u16> = format!("\"{}\"", exe())
                .encode_utf16()
                .chain(std::iter::once(0))
                .collect();
            let _ = RegSetKeyValueW(
                HKEY_CURRENT_USER,
                &HSTRING::from(RUN_KEY),
                &HSTRING::from(sys::APP_NAME),
                REG_SZ.0,
                Some(cmd.as_ptr() as *const c_void),
                (cmd.len() * 2) as u32,
            );
        } else {
            let _ = RegDeleteKeyValueW(
                HKEY_CURRENT_USER,
                &HSTRING::from(RUN_KEY),
                &HSTRING::from(sys::APP_NAME),
            );
        }
    }
}
/// The installed copy starts with Windows unless the user switched that off. Checked on every launch, so it
/// heals itself if an installer or a cleaner removed the entry (or the app moved to a new folder).
pub fn ensure_autostart() {
    if packaged() {
        unsafe {
            let _ = RegDeleteKeyValueW(
                HKEY_CURRENT_USER,
                &HSTRING::from(RUN_KEY),
                &HSTRING::from(sys::OLD_NAME),
            );
        }
    }
    if packaged() && get("autoStart") != Value::Bool(false) && !autostart_on() {
        set_autostart(true);
        sys::log(&format!(
            "start-with-Windows entry written -> {}",
            autostart_on()
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_include_glass_theme_and_shimmer() {
        let d = defaults();
        assert_eq!(d.get("skin").and_then(Value::as_str), Some("glass"));
        assert_eq!(d.get("face").and_then(Value::as_str), Some("eyes"));
        assert_eq!(d.get("shimmer").and_then(Value::as_bool), Some(true));
        assert_eq!(d.get("sounds").and_then(Value::as_bool), Some(true));
        assert_eq!(d.get("topOffset").and_then(Value::as_i64), Some(0));
        assert_eq!(d.get("notifSeconds").and_then(Value::as_i64), Some(4));
    }

    #[test]
    fn validates_themes_faces_and_ranges() {
        for skin in [
            "glass", "frost", "black", "aurora", "nebula", "crystal", "solar", "venom", "ember",
            "reactor",
        ] {
            assert!(valid_skin(skin), "expected valid skin {skin}");
        }
        assert!(!valid_skin("unknown"));

        for face in ["eyes", "clock", "logo"] {
            assert!(valid_face(face));
        }
        assert!(!valid_face("invalid"));

        assert_eq!(valid_notif_seconds(2.0), Some(2));
        assert_eq!(valid_notif_seconds(15.0), Some(15));
        assert_eq!(valid_notif_seconds(1.0), None);
        assert_eq!(valid_notif_seconds(16.0), None);

        assert_eq!(valid_top_offset(0.0), Some(0));
        assert_eq!(valid_top_offset(24.0), Some(24));
        assert_eq!(valid_top_offset(-1.0), None);
        assert_eq!(valid_top_offset(25.0), None);

        assert_eq!(valid_vault_length(8.0), Some(8));
        assert_eq!(valid_vault_length(40.0), Some(40));
        assert_eq!(valid_vault_length(7.0), None);
        assert_eq!(valid_vault_length(41.0), None);
    }
}
