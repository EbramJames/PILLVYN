// Full-access Windows & PILLVYN tool executor for the AI Assistant.
// Provides 22 structured JSON tools: PowerShell, app launcher, Win32 window manager,
// process inspection/termination, filesystem read/write/list/search, live system telemetry,
// display brightness, power control, Unicode keyboard typing/shortcuts, live web search,
// and direct PILLVYN Dynamic Island controls (timer, media, volume, notes, clipboard, themes, OCR).
use std::os::windows::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use serde_json::{json, Value};
use windows::core::{BOOL, PCWSTR};
use windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
use windows::Win32::System::Power::{GetSystemPowerStatus, SetSuspendState, SYSTEM_POWER_STATUS};
use windows::Win32::System::Shutdown::LockWorkStation;
use windows::Win32::System::SystemInformation::{GlobalMemoryStatusEx, MEMORYSTATUSEX};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYBD_EVENT_FLAGS, KEYEVENTF_KEYUP,
    VIRTUAL_KEY, VK_C, VK_CONTROL, VK_D, VK_LWIN, VK_MENU, VK_RETURN, VK_S, VK_TAB, VK_V, VK_W,
    VK_Y, VK_Z,
};
use windows::Win32::UI::Shell::{
    SHEmptyRecycleBinW, SHERB_NOCONFIRMATION, SHERB_NOPROGRESSUI, SHERB_NOSOUND,
};
use windows::Win32::UI::WindowsAndMessaging::{
    EnumWindows, IsIconic, IsWindowVisible, IsZoomed, PostMessageW, SendMessageW, ShowWindow,
    HWND_BROADCAST, SC_MONITORPOWER, SW_MAXIMIZE, SW_MINIMIZE, SW_RESTORE, WM_CLOSE, WM_SYSCOMMAND,
};

use crate::{audio, clip, hit, net_http, ocr, poll, settings, shelf, store, sys};

const CREATE_NO_WINDOW: u32 = 0x0800_0000;

pub fn tool_definitions(full_access: bool) -> Value {
    let mut tools = vec![
        json!({
            "type": "function",
            "function": {
                "name": "set_timer",
                "description": "Start a countdown timer on the PILLVYN Dynamic Island.",
                "parameters": {
                    "type": "object",
                    "properties": { "minutes": { "type": "number", "description": "Duration in minutes (0.5 to 180)" } },
                    "required": ["minutes"]
                }
            }
        }),
        json!({
            "type": "function",
            "function": {
                "name": "control_timer",
                "description": "Pause, resume, or reset the active PILLVYN timer.",
                "parameters": {
                    "type": "object",
                    "properties": { "action": { "type": "string", "enum": ["pause", "resume", "reset"] } },
                    "required": ["action"]
                }
            }
        }),
        json!({
            "type": "function",
            "function": {
                "name": "media_control",
                "description": "Control currently playing music or media on Windows.",
                "parameters": {
                    "type": "object",
                    "properties": { "action": { "type": "string", "enum": ["play_pause", "next", "previous"] } },
                    "required": ["action"]
                }
            }
        }),
        json!({
            "type": "function",
            "function": {
                "name": "set_volume",
                "description": "Set Windows master speaker volume percentage (0 to 100).",
                "parameters": {
                    "type": "object",
                    "properties": { "percent": { "type": "number", "description": "Volume 0..100" } },
                    "required": ["percent"]
                }
            }
        }),
        json!({
            "type": "function",
            "function": {
                "name": "toggle_mute",
                "description": "Toggle mute on Windows master audio output.",
                "parameters": { "type": "object", "properties": {} }
            }
        }),
        json!({
            "type": "function",
            "function": {
                "name": "create_note",
                "description": "Save a quick note in PILLVYN, optionally with a timed reminder.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "text": { "type": "string", "description": "Note or reminder text" },
                        "remind_in_minutes": { "type": "number", "description": "Optional reminder delay in minutes (0 = plain note)" }
                    },
                    "required": ["text"]
                }
            }
        }),
        json!({
            "type": "function",
            "function": {
                "name": "list_notes",
                "description": "Read the user's saved quick notes and reminders from PILLVYN.",
                "parameters": { "type": "object", "properties": {} }
            }
        }),
        json!({
            "type": "function",
            "function": {
                "name": "get_clipboard_history",
                "description": "Read recent copied clipboard items from PILLVYN.",
                "parameters": { "type": "object", "properties": {} }
            }
        }),
        json!({
            "type": "function",
            "function": {
                "name": "copy_to_clipboard",
                "description": "Copy text, code, or a translation directly to the Windows clipboard.",
                "parameters": {
                    "type": "object",
                    "properties": { "text": { "type": "string" } },
                    "required": ["text"]
                }
            }
        }),
        json!({
            "type": "function",
            "function": {
                "name": "switch_theme",
                "description": "Switch PILLVYN's visual theme immediately.",
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
        }),
        json!({
            "type": "function",
            "function": {
                "name": "open_url",
                "description": "Open a web URL (https:// or http://) in the default browser.",
                "parameters": {
                    "type": "object",
                    "properties": { "url": { "type": "string" } },
                    "required": ["url"]
                }
            }
        }),
        json!({
            "type": "function",
            "function": {
                "name": "desktop_action",
                "description": "Trigger a built-in Windows/PILLVYN action: screenshot, ocr (text grab), or dnd (Do Not Disturb settings).",
                "parameters": {
                    "type": "object",
                    "properties": { "action": { "type": "string", "enum": ["screenshot", "ocr", "dnd"] } },
                    "required": ["action"]
                }
            }
        }),
        json!({
            "type": "function",
            "function": {
                "name": "get_system_info",
                "description": "Get live Windows system telemetry: RAM, battery, CPU/disks, active window, volume, and theme.",
                "parameters": { "type": "object", "properties": {} }
            }
        }),
        json!({
            "type": "function",
            "function": {
                "name": "web_search",
                "description": "Search the live web for up-to-date facts, weather, news, prices, or documentation.",
                "parameters": {
                    "type": "object",
                    "properties": { "query": { "type": "string", "description": "Search query in English or Persian" } },
                    "required": ["query"]
                }
            }
        }),
        json!({
            "type": "function",
            "function": {
                "name": "control_pillvyn_ui",
                "description": "Open or close any PILLVYN Dynamic Island panel (settings, notes, clips, shelf, inbox, vault, tour, timer, ai, close), jump to a Settings tab (appearance, behavior, features, ai, about), or show a quick HUD banner on the island.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "panel": { "type": "string", "enum": ["ai", "settings", "notes", "clips", "shelf", "inbox", "vault", "tour", "timer", "close"] },
                        "settings_tab": { "type": "string", "enum": ["appearance", "behavior", "features", "ai", "about"] },
                        "hud_title": { "type": "string", "description": "Optional short HUD banner title to flash on the pill" },
                        "hud_text": { "type": "string", "description": "Optional short HUD banner subtitle" }
                    }
                }
            }
        }),
        json!({
            "type": "function",
            "function": {
                "name": "manage_settings",
                "description": "Read or change ANY PILLVYN setting directly: skin, face (eyes/clock/logo), calendar (gregorian/shamsi/shamsi-fa/both), shimmer, sounds, musicReact, autoHide, hideVideo, topOffset (0-24), notifSeconds (2-15), aiWakeWord, aiFullAccess, aiReasoning (fast/deep), aiSttLang (auto/fa/en), aiSttEngine (auto/local/cloud), aiTtsAuto, aiTtsRate (-50..100), aiTtsVolume (0..100), aiTtsVoiceFa, aiTtsVoiceEn.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "action": { "type": "string", "enum": ["get", "set"] },
                        "key": { "type": "string", "description": "Setting key name" },
                        "value": { "description": "New value (boolean, number, or string) when action is 'set'" }
                    },
                    "required": ["action"]
                }
            }
        }),
        json!({
            "type": "function",
            "function": {
                "name": "manage_shelf",
                "description": "Control PILLVYN's Dynamic Island File Shelf: list parked files, add a file path onto the shelf, reveal a parked file in Explorer, or clear the shelf.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "action": { "type": "string", "enum": ["list", "add", "reveal", "clear"] },
                        "path": { "type": "string", "description": "File path (supports Desktop/Downloads/Documents prefixes) for add or reveal" }
                    },
                    "required": ["action"]
                }
            }
        }),
        json!({
            "type": "function",
            "function": {
                "name": "manage_notes",
                "description": "Pin/unpin or delete an existing PILLVYN quick note by numeric id or text search match.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "action": { "type": "string", "enum": ["pin", "delete"] },
                        "id": { "type": "number", "description": "Note id from list_notes" },
                        "query": { "type": "string", "description": "Fallback substring match if id is omitted" }
                    },
                    "required": ["action"]
                }
            }
        }),
        json!({
            "type": "function",
            "function": {
                "name": "manage_notifications",
                "description": "Clear notifications from PILLVYN's Dynamic Island inbox.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "action": { "type": "string", "enum": ["clear"] }
                    },
                    "required": ["action"]
                }
            }
        }),
        json!({
            "type": "function",
            "function": {
                "name": "generate_password",
                "description": "Generate a cryptographically strong password using PILLVYN's Vault generator and copy it privately to the clipboard (auto-clears after 30 seconds).",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "length": { "type": "number", "description": "Length between 8 and 40 (default 18)" },
                        "symbols": { "type": "boolean", "description": "Include symbols (default true)" }
                    }
                }
            }
        }),
    ];

    if full_access {
        tools.extend([
            json!({
                "type": "function",
                "function": {
                    "name": "launch_app",
                    "description": "Launch any Windows application, program, folder, file, or ms-settings URI (e.g. chrome, edge, vscode, notepad, calc, spotify, discord, telegram, explorer, settings, terminal, or a full path).",
                    "parameters": {
                        "type": "object",
                        "properties": {
                            "target": { "type": "string", "description": "App name, executable, folder, or URI" },
                            "args": { "type": "string", "description": "Optional command-line arguments" }
                        },
                        "required": ["target"]
                    }
                }
            }),
            json!({
                "type": "function",
                "function": {
                    "name": "manage_window",
                    "description": "List, focus, minimize, maximize, restore, or close any open Windows application window.",
                    "parameters": {
                        "type": "object",
                        "properties": {
                            "action": { "type": "string", "enum": ["list", "focus", "minimize", "maximize", "restore", "close"] },
                            "title_filter": { "type": "string", "description": "Partial window title or exe name (for focus/minimize/maximize/restore/close)" }
                        },
                        "required": ["action"]
                    }
                }
            }),
            json!({
                "type": "function",
                "function": {
                    "name": "list_processes",
                    "description": "List top running Windows processes with PID and memory usage.",
                    "parameters": { "type": "object", "properties": {} }
                }
            }),
            json!({
                "type": "function",
                "function": {
                    "name": "kill_process",
                    "description": "Terminate a Windows process by executable name (e.g. notepad.exe) or numeric PID.",
                    "parameters": {
                        "type": "object",
                        "properties": { "target": { "type": "string", "description": "Process name (e.g. chrome.exe) or PID" } },
                        "required": ["target"]
                    }
                }
            }),
            json!({
                "type": "function",
                "function": {
                    "name": "file_list",
                    "description": "List files and folders inside a directory (supports aliases: desktop, downloads, documents, pictures, music, home, or any full Windows path).",
                    "parameters": {
                        "type": "object",
                        "properties": { "path": { "type": "string" } },
                        "required": ["path"]
                    }
                }
            }),
            json!({
                "type": "function",
                "function": {
                    "name": "file_read",
                    "description": "Read text contents of a file on disk (up to 16 KB).",
                    "parameters": {
                        "type": "object",
                        "properties": { "path": { "type": "string" } },
                        "required": ["path"]
                    }
                }
            }),
            json!({
                "type": "function",
                "function": {
                    "name": "file_write",
                    "description": "Create, overwrite, or append text to a file on disk (supports desktop/documents/downloads aliases or full path).",
                    "parameters": {
                        "type": "object",
                        "properties": {
                            "path": { "type": "string" },
                            "content": { "type": "string" },
                            "append": { "type": "boolean", "description": "Append instead of overwriting" }
                        },
                        "required": ["path", "content"]
                    }
                }
            }),
            json!({
                "type": "function",
                "function": {
                    "name": "file_search",
                    "description": "Search for files or folders by name inside a directory (e.g. desktop, downloads, documents, or full path).",
                    "parameters": {
                        "type": "object",
                        "properties": {
                            "directory": { "type": "string" },
                            "query": { "type": "string" }
                        },
                        "required": ["directory", "query"]
                    }
                }
            }),
            json!({
                "type": "function",
                "function": {
                    "name": "run_powershell",
                    "description": "Execute any Windows PowerShell command for full system automation, diagnostics, file/app management, registry/settings, git, networking, or calculations. Returns stdout, stderr, and exitCode.",
                    "parameters": {
                        "type": "object",
                        "properties": { "command": { "type": "string", "description": "PowerShell command or script" } },
                        "required": ["command"]
                    }
                }
            }),
            json!({
                "type": "function",
                "function": {
                    "name": "power_control",
                    "description": "Control Windows power and desktop state: lock, sleep, display_off, empty_recycle_bin, restart, shutdown.",
                    "parameters": {
                        "type": "object",
                        "properties": {
                            "action": {
                                "type": "string",
                                "enum": ["lock", "sleep", "display_off", "empty_recycle_bin", "restart", "shutdown"]
                            }
                        },
                        "required": ["action"]
                    }
                }
            }),
            json!({
                "type": "function",
                "function": {
                    "name": "set_brightness",
                    "description": "Set laptop/monitor screen brightness (0 to 100%).",
                    "parameters": {
                        "type": "object",
                        "properties": { "percent": { "type": "number", "description": "Brightness 0..100" } },
                        "required": ["percent"]
                    }
                }
            }),
            json!({
                "type": "function",
                "function": {
                    "name": "keyboard_action",
                    "description": "Type Persian/English text into the currently active window or press a common keyboard shortcut.",
                    "parameters": {
                        "type": "object",
                        "properties": {
                            "action": {
                                "type": "string",
                                "enum": ["type", "copy", "paste", "undo", "redo", "save", "close_tab", "switch_window", "show_desktop", "enter"]
                            },
                            "text": { "type": "string", "description": "Text to type when action is 'type'" }
                        },
                        "required": ["action"]
                    }
                }
            }),
        ]);
    }
    Value::Array(tools)
}

pub fn execute_tool(name: &str, args: &Value, full_access: bool) -> (String, String) {
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
                        .take(12)
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
                        .take(10)
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
        "get_system_info" => (system_info_json().to_string(), "System info".into()),
        "web_search" => {
            let q = args["query"].as_str().unwrap_or("").trim();
            (web_search_exec(q).to_string(), "Web search".into())
        }
        "control_pillvyn_ui" => {
            let panel = args["panel"].as_str().unwrap_or("");
            let tab = args["settings_tab"].as_str().unwrap_or("");
            let hud_title = args["hud_title"].as_str().unwrap_or("");
            let hud_text = args["hud_text"].as_str().unwrap_or("");
            sys::emit(
                "ai-action",
                json!({
                    "type": "ui",
                    "panel": panel,
                    "settingsTab": tab,
                    "hudTitle": hud_title,
                    "hudText": hud_text,
                }),
            );
            let badge = if !panel.is_empty() {
                format!("UI → {panel}")
            } else if !tab.is_empty() {
                format!("Settings → {tab}")
            } else {
                "Island HUD".into()
            };
            (
                json!({ "ok": true, "panel": panel, "settings_tab": tab }).to_string(),
                badge,
            )
        }
        "manage_settings" => {
            let act = args["action"].as_str().unwrap_or("get");
            let key = args["key"].as_str().unwrap_or("").trim();
            if act == "set" && !key.is_empty() {
                let val = &args["value"];
                settings::apply(key, val);
                let updated = settings::get(key);
                (
                    json!({ "ok": true, "key": key, "value": updated }).to_string(),
                    format!("Set {key}"),
                )
            } else if !key.is_empty() {
                (
                    json!({ "ok": true, "key": key, "value": settings::get(key) }).to_string(),
                    format!("Read {key}"),
                )
            } else {
                let mut snap = settings::payload();
                if let Some(obj) = snap.as_object_mut() {
                    obj.remove("aiKeyMasked");
                }
                (snap.to_string(), "Read settings".into())
            }
        }
        "manage_shelf" => {
            let act = args["action"].as_str().unwrap_or("list");
            match act {
                "clear" => {
                    shelf::clear();
                    (
                        json!({ "ok": true, "cleared": true }).to_string(),
                        "Shelf cleared".into(),
                    )
                }
                "add" => {
                    let p = resolve_user_path(args["path"].as_str().unwrap_or(""));
                    let p_str = p.to_string_lossy().to_string();
                    let exists = p.exists();
                    if exists {
                        let list = shelf::add(vec![p_str.clone()]);
                        sys::emit("shelf", list);
                        let nm = p.file_name().and_then(|n| n.to_str()).unwrap_or("file");
                        (
                            json!({ "ok": true, "added": p_str }).to_string(),
                            format!("Shelved {nm}"),
                        )
                    } else {
                        (
                            json!({ "ok": false, "error": format!("file not found: {p_str}") })
                                .to_string(),
                            "Shelf file missing".into(),
                        )
                    }
                }
                "reveal" => {
                    let p = resolve_user_path(args["path"].as_str().unwrap_or(""));
                    let p_str = p.to_string_lossy().to_string();
                    shelf::reveal(&p_str);
                    (
                        json!({ "ok": true, "path": p_str }).to_string(),
                        "Revealed file".into(),
                    )
                }
                _ => {
                    let raw = shelf::list();
                    let compact: Vec<Value> = raw
                        .as_array()
                        .into_iter()
                        .flatten()
                        .map(|f| json!({ "name": f["name"], "path": f["path"] }))
                        .collect();
                    (
                        json!({ "ok": true, "files": compact }).to_string(),
                        "Listed shelf".into(),
                    )
                }
            }
        }
        "manage_notes" => {
            let act = args["action"].as_str().unwrap_or("pin");
            let mut target_id = args["id"].as_u64();
            if target_id.is_none() {
                let q = args["query"].as_str().unwrap_or("").trim().to_lowercase();
                if !q.is_empty() {
                    if let Some(arr) = store::notes().as_array() {
                        target_id = arr.iter().find_map(|n| {
                            n["text"]
                                .as_str()
                                .filter(|t| t.to_lowercase().contains(&q))
                                .and_then(|_| n["id"].as_u64())
                        });
                    }
                }
            }
            match (act, target_id) {
                ("delete", Some(id)) => {
                    store::note_remove(id);
                    (
                        json!({ "ok": true, "deleted_id": id }).to_string(),
                        "Deleted note".into(),
                    )
                }
                ("pin", Some(id)) => {
                    store::note_pin(id);
                    (
                        json!({ "ok": true, "toggled_pin_id": id }).to_string(),
                        "Pinned note".into(),
                    )
                }
                _ => (
                    json!({ "ok": false, "error": "note not found — call list_notes first" })
                        .to_string(),
                    "Note not found".into(),
                ),
            }
        }
        "manage_notifications" => {
            poll::send(poll::Cmd::ClearAll);
            sys::emit("ai-action", json!({ "type": "clear_notifs" }));
            (
                json!({ "ok": true, "cleared": true }).to_string(),
                "Cleared alerts".into(),
            )
        }
        "generate_password" => {
            let len = args["length"].as_u64().unwrap_or(18).clamp(8, 40) as usize;
            let symbols = args["symbols"].as_bool().unwrap_or(true);
            let pw = store::generate(len, symbols);
            let copied = store::copy_secret(&pw);
            (
                json!({ "ok": true, "length": pw.len(), "copied_private_30s": copied }).to_string(),
                format!("Password ({len}ch)"),
            )
        }

        // ---- Full-access system tools ----
        "launch_app" | "manage_window" | "list_processes" | "kill_process" | "file_list"
        | "file_read" | "file_write" | "file_search" | "run_powershell" | "power_control"
        | "set_brightness" | "keyboard_action"
            if !full_access =>
        {
            (
                json!({ "ok": false, "error": "Full system access is disabled in Settings → AI." })
                    .to_string(),
                "System access off".into(),
            )
        }
        "launch_app" => {
            launch_app_exec(args["target"].as_str().unwrap_or(""), args["args"].as_str())
        }
        "manage_window" => manage_window_exec(
            args["action"].as_str().unwrap_or("list"),
            args["title_filter"].as_str().unwrap_or(""),
        ),
        "list_processes" => list_processes_exec(),
        "kill_process" => kill_process_exec(args["target"].as_str().unwrap_or("")),
        "file_list" => file_list_exec(args["path"].as_str().unwrap_or("desktop")),
        "file_read" => file_read_exec(args["path"].as_str().unwrap_or("")),
        "file_write" => file_write_exec(
            args["path"].as_str().unwrap_or(""),
            args["content"].as_str().unwrap_or(""),
            args["append"].as_bool().unwrap_or(false),
        ),
        "file_search" => file_search_exec(
            args["directory"].as_str().unwrap_or("desktop"),
            args["query"].as_str().unwrap_or(""),
        ),
        "run_powershell" => run_powershell_exec(args["command"].as_str().unwrap_or("")),
        "power_control" => power_control_exec(args["action"].as_str().unwrap_or("")),
        "set_brightness" => set_brightness_exec(args["percent"].as_f64().unwrap_or(70.0)),
        "keyboard_action" => keyboard_action_exec(
            args["action"].as_str().unwrap_or(""),
            args["text"].as_str().unwrap_or(""),
        ),
        _ => (
            json!({ "ok": false, "error": format!("unknown tool: {name}") }).to_string(),
            format!("Unknown: {name}"),
        ),
    }
}

// ---------------------------------------------------------------- Path resolution
pub(crate) fn resolve_user_path(raw: &str) -> PathBuf {
    let mut expanded = raw.trim().trim_matches('"').to_string();
    for (token, env_var) in [
        ("%userprofile%", "USERPROFILE"),
        ("$env:userprofile", "USERPROFILE"),
        ("%appdata%", "APPDATA"),
        ("%localappdata%", "LOCALAPPDATA"),
        ("%temp%", "TEMP"),
        ("%tmp%", "TEMP"),
    ] {
        let lower_now = expanded.to_ascii_lowercase();
        if let Some(pos) = lower_now.find(token) {
            if let Ok(val) = std::env::var(env_var) {
                expanded.replace_range(pos..pos + token.len(), &val);
            }
        }
    }
    let trimmed = expanded.as_str();
    let home = std::env::var_os("USERPROFILE")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("C:\\"));
    let lower = trimmed.to_ascii_lowercase();
    match lower.as_str() {
        "" | "home" | "~" => return home,
        "desktop" => return home.join("Desktop"),
        "downloads" => return home.join("Downloads"),
        "documents" | "docs" => return home.join("Documents"),
        "pictures" | "photos" => return home.join("Pictures"),
        "music" => return home.join("Music"),
        "videos" => return home.join("Videos"),
        _ => {}
    }
    for (prefix, sub) in [
        ("desktop\\", "Desktop"),
        ("desktop/", "Desktop"),
        ("downloads\\", "Downloads"),
        ("downloads/", "Downloads"),
        ("documents\\", "Documents"),
        ("documents/", "Documents"),
        ("~\\", ""),
        ("~/", ""),
    ] {
        if lower.starts_with(prefix) {
            let rest = &trimmed[prefix.len()..];
            return if sub.is_empty() {
                home.join(rest)
            } else {
                home.join(sub).join(rest)
            };
        }
    }
    PathBuf::from(trimmed)
}

// ---------------------------------------------------------------- PowerShell runner (25s timeout, UTF-8)
fn run_ps_raw(script: &str, timeout_secs: u64) -> Result<(i32, String, String), String> {
    let wrapped = format!(
        "[Console]::OutputEncoding=[System.Text.Encoding]::UTF8; $OutputEncoding=[System.Text.Encoding]::UTF8; {script}"
    );
    let mut child = Command::new("powershell")
        .args([
            "-NoProfile",
            "-NonInteractive",
            "-ExecutionPolicy",
            "Bypass",
            "-Command",
            &wrapped,
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .creation_flags(CREATE_NO_WINDOW)
        .spawn()
        .map_err(|e| format!("could not launch PowerShell: {e}"))?;

    let start = Instant::now();
    let deadline = Duration::from_secs(timeout_secs);
    loop {
        if child.try_wait().map_err(|e| e.to_string())?.is_some() {
            let out = child.wait_with_output().map_err(|e| e.to_string())?;
            let code = out.status.code().unwrap_or(-1);
            let stdout: String = String::from_utf8_lossy(&out.stdout)
                .trim()
                .chars()
                .take(6000)
                .collect();
            let stderr: String = String::from_utf8_lossy(&out.stderr)
                .trim()
                .chars()
                .take(2000)
                .collect();
            return Ok((code, stdout, stderr));
        }
        if start.elapsed() > deadline {
            let _ = child.kill();
            let _ = child.wait();
            return Err(format!("PowerShell timed out after {timeout_secs}s"));
        }
        std::thread::sleep(Duration::from_millis(60));
    }
}

fn run_powershell_exec(cmd: &str) -> (String, String) {
    let trimmed = cmd.trim();
    if trimmed.is_empty() {
        return (
            json!({ "ok": false, "error": "empty command" }).to_string(),
            "PowerShell empty".into(),
        );
    }
    match run_ps_raw(trimmed, 25) {
        Ok((code, stdout, stderr)) => (
            json!({
                "ok": code == 0,
                "exitCode": code,
                "stdout": stdout,
                "stderr": stderr,
            })
            .to_string(),
            if code == 0 {
                "Ran PowerShell".into()
            } else {
                format!("PowerShell ({code})")
            },
        ),
        Err(e) => (
            json!({ "ok": false, "error": e }).to_string(),
            "PowerShell failed".into(),
        ),
    }
}

// ---------------------------------------------------------------- Smart App Launcher
fn find_start_menu_shortcut(query: &str) -> Option<PathBuf> {
    let q = query.to_ascii_lowercase();
    let mut dirs = Vec::new();
    if let Some(pd) = std::env::var_os("ProgramData") {
        dirs.push(PathBuf::from(pd).join("Microsoft\\Windows\\Start Menu\\Programs"));
    }
    if let Some(ad) = std::env::var_os("APPDATA") {
        dirs.push(PathBuf::from(ad).join("Microsoft\\Windows\\Start Menu\\Programs"));
    }
    fn walk(dir: &Path, q: &str, depth: usize) -> Option<PathBuf> {
        if depth > 4 {
            return None;
        }
        for entry in std::fs::read_dir(dir).ok()?.flatten() {
            let p = entry.path();
            if p.is_dir() {
                if let Some(hit) = walk(&p, q, depth + 1) {
                    return Some(hit);
                }
            } else if p
                .extension()
                .and_then(|e| e.to_str())
                .is_some_and(|e| e.eq_ignore_ascii_case("lnk"))
            {
                let stem = p
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("")
                    .to_ascii_lowercase();
                if stem == q || stem.contains(q) {
                    return Some(p);
                }
            }
        }
        None
    }
    for d in dirs {
        if let Some(found) = walk(&d, &q, 0) {
            return Some(found);
        }
    }
    None
}

fn launch_app_exec(raw_target: &str, args: Option<&str>) -> (String, String) {
    let t = raw_target.trim();
    if t.is_empty() {
        return (
            json!({ "ok": false, "error": "empty target" }).to_string(),
            "Launch failed".into(),
        );
    }
    let lower = t.to_ascii_lowercase();
    let resolved: String = match lower.as_str() {
        "chrome" | "google chrome" => "chrome".into(),
        "edge" | "microsoft edge" => "msedge".into(),
        "firefox" => "firefox".into(),
        "notepad" | "نوت‌پد" | "نوت پد" => "notepad".into(),
        "calc" | "calculator" | "ماشین حساب" => "calc".into(),
        "paint" | "mspaint" => "mspaint".into(),
        "cmd" | "command prompt" => "cmd".into(),
        "terminal" | "windows terminal" | "wt" => "wt".into(),
        "powershell" => "powershell".into(),
        "explorer" | "file explorer" | "فایل اکسپلورر" => "explorer".into(),
        "task manager" | "taskmgr" | "تسک منیجر" => "taskmgr".into(),
        "settings" | "windows settings" | "تنظیمات" => "ms-settings:".into(),
        "control" | "control panel" => "control".into(),
        "code" | "vscode" | "vs code" | "visual studio code" => "code".into(),
        "spotify" => "spotify:".into(),
        "discord" => find_start_menu_shortcut("discord")
            .map(|p| p.to_string_lossy().into_owned())
            .unwrap_or_else(|| "discord:".into()),
        "telegram" | "تلگرام" => find_start_menu_shortcut("telegram")
            .map(|p| p.to_string_lossy().into_owned())
            .unwrap_or_else(|| "tg:".into()),
        "desktop" | "downloads" | "documents" | "pictures" | "music" | "videos" => {
            resolve_user_path(&lower).to_string_lossy().into_owned()
        }
        _ => {
            let maybe_path = resolve_user_path(t);
            if maybe_path.exists() {
                maybe_path.to_string_lossy().into_owned()
            } else if let Some(lnk) = find_start_menu_shortcut(t) {
                lnk.to_string_lossy().into_owned()
            } else {
                t.to_string()
            }
        }
    };
    sys::allow_foreground();
    sys::shell_open(&resolved, args.filter(|a| !a.trim().is_empty()));
    let short: String = t.chars().take(24).collect();
    (
        json!({ "ok": true, "launched": resolved }).to_string(),
        format!("Opened {short}"),
    )
}

// ---------------------------------------------------------------- Win32 Window Manager
struct WinEntry {
    hwnd: HWND,
    title: String,
    exe: String,
    pid: u32,
    minimized: bool,
    maximized: bool,
}

fn enumerate_windows() -> Vec<WinEntry> {
    let mut list: Vec<WinEntry> = Vec::new();
    unsafe extern "system" fn cb(h: HWND, lp: LPARAM) -> BOOL {
        if !IsWindowVisible(h).as_bool() || h == sys::hwnd() {
            return BOOL(1);
        }
        let title = sys::title_of(h);
        if title.trim().is_empty() {
            return BOOL(1);
        }
        let pid = sys::pid_of(h);
        let exe = sys::exe_of_pid(pid)
            .and_then(|p| p.rsplit('\\').next().map(str::to_string))
            .unwrap_or_default();
        let vec = &mut *(lp.0 as *mut Vec<WinEntry>);
        vec.push(WinEntry {
            hwnd: h,
            title,
            exe,
            pid,
            minimized: IsIconic(h).as_bool(),
            maximized: IsZoomed(h).as_bool(),
        });
        BOOL(1)
    }
    unsafe {
        let _ = EnumWindows(Some(cb), LPARAM(&mut list as *mut Vec<WinEntry> as isize));
    }
    list
}

fn manage_window_exec(action: &str, filter: &str) -> (String, String) {
    let wins = enumerate_windows();
    if action == "list" {
        let items: Vec<Value> = wins
            .iter()
            .take(25)
            .map(|w| {
                json!({
                    "title": w.title,
                    "exe": w.exe,
                    "pid": w.pid,
                    "minimized": w.minimized,
                    "maximized": w.maximized,
                })
            })
            .collect();
        return (
            json!({ "ok": true, "windows": items }).to_string(),
            "Listed windows".into(),
        );
    }
    let q = filter.trim().to_ascii_lowercase();
    let Some(hit_win) = wins.iter().find(|w| {
        !q.is_empty()
            && (w.title.to_ascii_lowercase().contains(&q)
                || w.exe.to_ascii_lowercase().contains(&q))
    }) else {
        return (
            json!({ "ok": false, "error": format!("No open window matching '{filter}'") })
                .to_string(),
            "Window not found".into(),
        );
    };
    unsafe {
        match action {
            "focus" => {
                if hit_win.minimized {
                    let _ = ShowWindow(hit_win.hwnd, SW_RESTORE);
                }
                sys::force_foreground(hit_win.hwnd);
            }
            "minimize" => {
                let _ = ShowWindow(hit_win.hwnd, SW_MINIMIZE);
            }
            "maximize" => {
                let _ = ShowWindow(hit_win.hwnd, SW_MAXIMIZE);
            }
            "restore" => {
                let _ = ShowWindow(hit_win.hwnd, SW_RESTORE);
            }
            "close" => {
                let _ = PostMessageW(Some(hit_win.hwnd), WM_CLOSE, WPARAM(0), LPARAM(0));
            }
            _ => {
                return (
                    json!({ "ok": false, "error": format!("unknown window action: {action}") })
                        .to_string(),
                    "Window error".into(),
                )
            }
        }
    }
    let short: String = hit_win
        .exe
        .trim_end_matches(".exe")
        .chars()
        .take(16)
        .collect();
    (
        json!({ "ok": true, "action": action, "title": hit_win.title, "exe": hit_win.exe })
            .to_string(),
        format!("Window {action}: {short}"),
    )
}

// ---------------------------------------------------------------- Processes
fn list_processes_exec() -> (String, String) {
    let ps = "Get-Process | Sort-Object WorkingSet64 -Descending | Select-Object -First 15 Id, ProcessName, @{n='MemMB';e={[math]::Round($_.WorkingSet64/1MB,1)}} | ConvertTo-Json -Compress";
    match run_ps_raw(ps, 10) {
        Ok((0, stdout, _)) => (stdout, "Listed processes".into()),
        Ok((_, _, err)) => (
            json!({ "ok": false, "error": err }).to_string(),
            "Process error".into(),
        ),
        Err(e) => (
            json!({ "ok": false, "error": e }).to_string(),
            "Process error".into(),
        ),
    }
}

fn kill_process_exec(target: &str) -> (String, String) {
    let t = target.trim();
    if t.is_empty() {
        return (
            json!({ "ok": false, "error": "empty process target" }).to_string(),
            "Kill failed".into(),
        );
    }
    let out = if t.chars().all(|c| c.is_ascii_digit()) {
        Command::new("taskkill")
            .args(["/F", "/PID", t])
            .creation_flags(CREATE_NO_WINDOW)
            .output()
    } else {
        let img = if t.to_ascii_lowercase().ends_with(".exe") {
            t.to_string()
        } else {
            format!("{t}.exe")
        };
        Command::new("taskkill")
            .args(["/F", "/IM", &img])
            .creation_flags(CREATE_NO_WINDOW)
            .output()
    };
    match out {
        Ok(o) if o.status.success() => (
            json!({ "ok": true, "killed": t }).to_string(),
            format!("Closed {t}"),
        ),
        Ok(o) => (
            json!({ "ok": false, "error": String::from_utf8_lossy(&o.stderr).trim() }).to_string(),
            "Close failed".into(),
        ),
        Err(e) => (
            json!({ "ok": false, "error": e.to_string() }).to_string(),
            "Close failed".into(),
        ),
    }
}

// ---------------------------------------------------------------- Filesystem tools
fn file_list_exec(raw: &str) -> (String, String) {
    let dir = resolve_user_path(raw);
    match std::fs::read_dir(&dir) {
        Ok(rd) => {
            let mut items = Vec::new();
            for entry in rd.flatten().take(50) {
                let meta = entry.metadata().ok();
                items.push(json!({
                    "name": entry.file_name().to_string_lossy(),
                    "dir": meta.as_ref().map(|m| m.is_dir()).unwrap_or(false),
                    "bytes": meta.as_ref().map(|m| m.len()).unwrap_or(0),
                }));
            }
            (
                json!({ "ok": true, "path": dir.to_string_lossy(), "items": items }).to_string(),
                "Listed folder".into(),
            )
        }
        Err(e) => (
            json!({ "ok": false, "error": format!("{}: {e}", dir.display()) }).to_string(),
            "Folder error".into(),
        ),
    }
}

fn file_read_exec(raw: &str) -> (String, String) {
    let p = resolve_user_path(raw);
    match std::fs::read(&p) {
        Ok(bytes) => {
            let truncated = bytes.len() > 16_384;
            let slice = &bytes[..bytes.len().min(16_384)];
            let content = String::from_utf8_lossy(slice).into_owned();
            (
                json!({ "ok": true, "path": p.to_string_lossy(), "truncated": truncated, "content": content })
                    .to_string(),
                "Read file".into(),
            )
        }
        Err(e) => (
            json!({ "ok": false, "error": format!("{}: {e}", p.display()) }).to_string(),
            "Read error".into(),
        ),
    }
}

fn file_write_exec(raw: &str, content: &str, append: bool) -> (String, String) {
    let p = resolve_user_path(raw);
    if let Some(parent) = p.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let res = if append {
        use std::io::Write;
        std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&p)
            .and_then(|mut f| f.write_all(content.as_bytes()))
    } else {
        std::fs::write(&p, content.as_bytes())
    };
    match res {
        Ok(()) => {
            let nm = p.file_name().and_then(|n| n.to_str()).unwrap_or("file");
            (
                json!({ "ok": true, "path": p.to_string_lossy(), "bytes": content.len() })
                    .to_string(),
                format!("Saved {nm}"),
            )
        }
        Err(e) => (
            json!({ "ok": false, "error": format!("{}: {e}", p.display()) }).to_string(),
            "Write error".into(),
        ),
    }
}

fn file_search_exec(raw_dir: &str, query: &str) -> (String, String) {
    let root = resolve_user_path(raw_dir);
    let q = query.trim().to_ascii_lowercase();
    if q.is_empty() {
        return (
            json!({ "ok": false, "error": "empty query" }).to_string(),
            "Search empty".into(),
        );
    }
    let mut hits = Vec::new();
    fn walk(dir: &Path, q: &str, depth: usize, hits: &mut Vec<String>) {
        if depth > 4 || hits.len() >= 30 {
            return;
        }
        let Ok(rd) = std::fs::read_dir(dir) else {
            return;
        };
        for e in rd.flatten() {
            if hits.len() >= 30 {
                break;
            }
            let p = e.path();
            let name = e.file_name().to_string_lossy().to_ascii_lowercase();
            if name.contains(q) {
                hits.push(p.to_string_lossy().into_owned());
            }
            if p.is_dir() && !name.starts_with('.') && name != "node_modules" && name != "target" {
                walk(&p, q, depth + 1, hits);
            }
        }
    }
    walk(&root, &q, 0, &mut hits);
    (
        json!({ "ok": true, "directory": root.to_string_lossy(), "matches": hits }).to_string(),
        format!("Found {} files", hits.len()),
    )
}

// ---------------------------------------------------------------- System Info, Power, Brightness, Keyboard, Web Search
fn system_info_json() -> Value {
    let mut mem = MEMORYSTATUSEX {
        dwLength: std::mem::size_of::<MEMORYSTATUSEX>() as u32,
        ..Default::default()
    };
    let (ram_used_gb, ram_total_gb, ram_pct) = unsafe {
        if GlobalMemoryStatusEx(&mut mem).is_ok() {
            let tot = mem.ullTotalPhys as f64 / 1_073_741_824.0;
            let avail = mem.ullAvailPhys as f64 / 1_073_741_824.0;
            (
                ((tot - avail) * 10.0).round() / 10.0,
                (tot * 10.0).round() / 10.0,
                mem.dwMemoryLoad,
            )
        } else {
            (0.0, 0.0, 0)
        }
    };
    let mut pwr = SYSTEM_POWER_STATUS::default();
    let battery = unsafe {
        if GetSystemPowerStatus(&mut pwr).is_ok() && pwr.BatteryLifePercent <= 100 {
            json!({ "percent": pwr.BatteryLifePercent, "charging": pwr.ACLineStatus == 1 })
        } else {
            Value::Null
        }
    };
    let vol = audio::volume()
        .map(|(v, m)| json!({ "percent": (v * 100.0).round() as i32, "muted": m }))
        .unwrap_or(Value::Null);
    json!({
        "ok": true,
        "ramUsedGB": ram_used_gb,
        "ramTotalGB": ram_total_gb,
        "ramLoadPct": ram_pct,
        "battery": battery,
        "volume": vol,
        "theme": settings::get("skin"),
        "activeWindow": hit::fg_context(),
        "cpuCores": std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4),
    })
}

fn power_control_exec(action: &str) -> (String, String) {
    match action {
        "lock" => {
            unsafe {
                let _ = LockWorkStation();
            }
            (
                json!({ "ok": true, "action": "lock" }).to_string(),
                "Locked PC".into(),
            )
        }
        "sleep" => {
            std::thread::spawn(|| {
                std::thread::sleep(Duration::from_millis(600));
                unsafe {
                    let _ = SetSuspendState(false, false, false);
                }
            });
            (
                json!({ "ok": true, "action": "sleep" }).to_string(),
                "Sleeping PC".into(),
            )
        }
        "display_off" => {
            unsafe {
                let _ = SendMessageW(
                    HWND_BROADCAST,
                    WM_SYSCOMMAND,
                    Some(WPARAM(SC_MONITORPOWER as usize)),
                    Some(LPARAM(2)),
                );
            }
            (
                json!({ "ok": true, "action": "display_off" }).to_string(),
                "Display off".into(),
            )
        }
        "empty_recycle_bin" => {
            let ok = unsafe {
                SHEmptyRecycleBinW(
                    None,
                    PCWSTR::null(),
                    SHERB_NOCONFIRMATION | SHERB_NOPROGRESSUI | SHERB_NOSOUND,
                )
                .is_ok()
            };
            (json!({ "ok": ok }).to_string(), "Emptied trash".into())
        }
        "restart" => {
            let _ = Command::new("shutdown")
                .args(["/r", "/t", "5"])
                .creation_flags(CREATE_NO_WINDOW)
                .spawn();
            (
                json!({ "ok": true, "action": "restart", "delaySeconds": 5 }).to_string(),
                "Restarting (5s)".into(),
            )
        }
        "shutdown" => {
            let _ = Command::new("shutdown")
                .args(["/s", "/t", "5"])
                .creation_flags(CREATE_NO_WINDOW)
                .spawn();
            (
                json!({ "ok": true, "action": "shutdown", "delaySeconds": 5 }).to_string(),
                "Shutting down (5s)".into(),
            )
        }
        _ => (
            json!({ "ok": false, "error": format!("unknown power action: {action}") }).to_string(),
            "Power error".into(),
        ),
    }
}

fn set_brightness_exec(pct: f64) -> (String, String) {
    let val = pct.round().clamp(0.0, 100.0) as i32;
    let ps = format!("(Get-WmiObject -Namespace root/WMI -Class WmiMonitorBrightnessMethods).WmiSetBrightness(1, {val})");
    match run_ps_raw(&ps, 8) {
        Ok((0, _, _)) => (
            json!({ "ok": true, "percent": val }).to_string(),
            format!("Brightness {val}%"),
        ),
        Ok((_, _, err)) => (
            json!({ "ok": false, "error": format!("External desktop monitors may not support WMI brightness: {err}") }).to_string(),
            "Brightness N/A".into(),
        ),
        Err(e) => (json!({ "ok": false, "error": e }).to_string(), "Brightness err".into()),
    }
}

fn send_key_combo(keys: &[VIRTUAL_KEY]) {
    let mut inputs = Vec::with_capacity(keys.len() * 2);
    for &vk in keys {
        inputs.push(INPUT {
            r#type: INPUT_KEYBOARD,
            Anonymous: INPUT_0 {
                ki: KEYBDINPUT {
                    wVk: vk,
                    wScan: 0,
                    dwFlags: KEYBD_EVENT_FLAGS(0),
                    time: 0,
                    dwExtraInfo: 0,
                },
            },
        });
    }
    for &vk in keys.iter().rev() {
        inputs.push(INPUT {
            r#type: INPUT_KEYBOARD,
            Anonymous: INPUT_0 {
                ki: KEYBDINPUT {
                    wVk: vk,
                    wScan: 0,
                    dwFlags: KEYEVENTF_KEYUP,
                    time: 0,
                    dwExtraInfo: 0,
                },
            },
        });
    }
    unsafe {
        let _ = SendInput(&inputs, std::mem::size_of::<INPUT>() as i32);
    }
}

fn keyboard_action_exec(action: &str, text: &str) -> (String, String) {
    match action {
        "type" => {
            if text.is_empty() {
                return (
                    json!({ "ok": false, "error": "empty text" }).to_string(),
                    "Nothing typed".into(),
                );
            }
            let _ = clip::write_text(text);
            std::thread::sleep(Duration::from_millis(60));
            send_key_combo(&[VK_CONTROL, VK_V]);
            (
                json!({ "ok": true, "typed": text }).to_string(),
                "Typed text".into(),
            )
        }
        "copy" => {
            send_key_combo(&[VK_CONTROL, VK_C]);
            (json!({ "ok": true }).to_string(), "Ctrl+C".into())
        }
        "paste" => {
            send_key_combo(&[VK_CONTROL, VK_V]);
            (json!({ "ok": true }).to_string(), "Ctrl+V".into())
        }
        "undo" => {
            send_key_combo(&[VK_CONTROL, VK_Z]);
            (json!({ "ok": true }).to_string(), "Undo".into())
        }
        "redo" => {
            send_key_combo(&[VK_CONTROL, VK_Y]);
            (json!({ "ok": true }).to_string(), "Redo".into())
        }
        "save" => {
            send_key_combo(&[VK_CONTROL, VK_S]);
            (json!({ "ok": true }).to_string(), "Ctrl+S".into())
        }
        "close_tab" => {
            send_key_combo(&[VK_CONTROL, VK_W]);
            (json!({ "ok": true }).to_string(), "Closed tab".into())
        }
        "switch_window" => {
            send_key_combo(&[VK_MENU, VK_TAB]);
            (json!({ "ok": true }).to_string(), "Switched window".into())
        }
        "show_desktop" => {
            send_key_combo(&[VK_LWIN, VK_D]);
            (json!({ "ok": true }).to_string(), "Show desktop".into())
        }
        "enter" => {
            send_key_combo(&[VK_RETURN]);
            (json!({ "ok": true }).to_string(), "Enter".into())
        }
        _ => (
            json!({ "ok": false, "error": format!("unknown keyboard action: {action}") })
                .to_string(),
            "Key error".into(),
        ),
    }
}

fn pct_encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len() * 2);
    for b in s.as_bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b'~') {
            out.push(*b as char);
        } else if *b == b' ' {
            out.push('+');
        } else {
            use std::fmt::Write;
            let _ = write!(&mut out, "%{b:02X}");
        }
    }
    out
}

fn strip_html_tags(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut in_tag = false;
    for c in s.chars() {
        if c == '<' {
            in_tag = true;
        } else if c == '>' {
            in_tag = false;
        } else if !in_tag {
            out.push(c);
        }
    }
    out.replace("&amp;", "&")
        .replace("&quot;", "\"")
        .replace("&#x27;", "'")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&nbsp;", " ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn web_search_exec(query: &str) -> Value {
    if query.is_empty() {
        return json!({ "ok": false, "error": "empty query" });
    }
    let url = format!("https://html.duckduckgo.com/html/?q={}", pct_encode(query));
    match net_http::request(
        "GET",
        &url,
        &[("User-Agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 Chrome/143.0.0.0 Safari/537.36")],
        None,
        12_000,
    ) {
        Ok((200..=299, bytes)) => {
            let html = String::from_utf8_lossy(&bytes);
            let mut results = Vec::new();
            let mut rest = html.as_ref();
            while let Some(pos) = rest.find("result__snippet") {
                rest = &rest[pos + 15..];
                if let Some(gt) = rest.find('>') {
                    let after = &rest[gt + 1..];
                    if let Some(end) = after.find("</a>").or_else(|| after.find("</div>")) {
                        let snippet = strip_html_tags(&after[..end]);
                        if !snippet.is_empty() {
                            results.push(snippet);
                            if results.len() >= 5 {
                                break;
                            }
                        }
                    }
                }
            }
            json!({ "ok": true, "query": query, "snippets": results })
        }
        Ok((code, _)) => json!({ "ok": false, "error": format!("search HTTP {code}") }),
        Err(e) => json!({ "ok": false, "error": e }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn full_access_exposes_all_system_and_pillvyn_tools() {
        let full = tool_definitions(true);
        let arr = full.as_array().unwrap();
        assert!(arr.len() >= 30, "expected >= 30 tools, got {}", arr.len());
        for req in [
            "run_powershell",
            "launch_app",
            "manage_window",
            "file_read",
            "file_write",
            "power_control",
            "web_search",
            "set_timer",
            "switch_theme",
            "control_pillvyn_ui",
            "manage_settings",
            "manage_shelf",
            "manage_notes",
            "manage_notifications",
            "generate_password",
        ] {
            assert!(
                arr.iter().any(|t| t["function"]["name"] == req),
                "missing tool {req}"
            );
        }
        let restricted = tool_definitions(false);
        assert!(restricted.as_array().unwrap().len() < arr.len());
    }

    #[test]
    fn resolves_friendly_user_paths() {
        let desk = resolve_user_path("desktop");
        assert!(desk.to_string_lossy().ends_with("Desktop"));
        let sub = resolve_user_path("desktop/hello.txt");
        assert!(
            sub.to_string_lossy().ends_with("Desktop\\hello.txt")
                || sub.to_string_lossy().ends_with("Desktop/hello.txt")
        );
        let env_p = resolve_user_path("%USERPROFILE%\\Desktop\\demo.txt");
        assert!(!env_p.to_string_lossy().contains('%'));
        assert!(env_p.to_string_lossy().ends_with("Desktop\\demo.txt"));
    }
}
