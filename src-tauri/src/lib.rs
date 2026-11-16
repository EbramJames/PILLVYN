pub mod commands;
pub mod config;
pub mod island;
pub mod services;

use commands::{
    ask_ai_assistant, cancel_appearance_preview, check_updates, clipboard_action,
    get_initial_state, get_monitors, get_tray_state, media_control, notification_action,
    open_settings_window, open_url, plugin_action, preview_appearance, push_notification,
    refresh_weather, resize_tray_window, run_quick_action, save_app_config, save_settings_config,
    set_click_through_override, set_display_brightness, set_island_pinned, set_mute, set_volume,
    switch_audio_device, sync_timer_activity, timer_action, tray_action, update_island_hitbox,
    validate_quick_action_cmd, AppState, TrayStateSnapshot,
};
use config::{load_config, save_config, AppProfile, PillvynConfig};
use island::activities::{now_ms, ActivityCoordinator, ActivityKind, LiveActivity};
use island::{
    apply_window_geometry, enforce_tool_window, spawn_click_through_controller, IslandWindowState,
};
use services::clipboard::{spawn_clipboard_listener, ClipboardStore};
use services::downloads::{DownloadMonitor, DownloadsSnapshot};
use services::gaming::{query_foreground_window_process, GamingMonitor};
use services::hardware::HardwareCollector;
use services::media::{
    query_media_snapshot, spawn_media_event_listener, MediaSnapshot, ThumbnailCache,
};
use services::network::NetworkMonitor;
use services::notifications::{NotificationHub, NotificationPriority};
use services::system_ctrl::{
    query_system_snapshot, spawn_audio_event_listener, SystemControlsSnapshot,
};
use services::widgets::{fetch_live_weather, WidgetEngine};
use services::win_hooks::{spawn_foreground_hook, take_external_foreground_flag};
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicI32, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{
    AppHandle, Emitter, LogicalSize, Manager, PhysicalPosition, WebviewUrl, WebviewWindow,
    WebviewWindowBuilder, WindowEvent,
};
use tauri_plugin_autostart::{MacosLauncher, ManagerExt as AutostartExt};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, ShortcutState};
use tokio::sync::Notify;
use tracing::{info, warn};

static LAST_TRAY_BLUR_MS: AtomicU64 = AtomicU64::new(0);
static LAST_TRAY_CLICK_X: AtomicI32 = AtomicI32::new(0);
static LAST_TRAY_CLICK_Y: AtomicI32 = AtomicI32::new(0);
static HAS_TRAY_CLICK: AtomicBool = AtomicBool::new(false);

fn normalize_hotkey_combo(combo: &str) -> String {
    combo
        .split('+')
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .map(str::to_ascii_lowercase)
        .collect::<Vec<_>>()
        .join("+")
}

pub fn validate_hotkeys_uniqueness(cfg: &PillvynConfig) -> Result<(), String> {
    let entries = [
        ("Expand / Collapse", cfg.hotkeys.toggle_expand.trim()),
        ("Show / Hide Island", cfg.hotkeys.toggle_visibility.trim()),
        ("Do Not Disturb", cfg.hotkeys.toggle_dnd.trim()),
        ("Clipboard Drawer", cfg.hotkeys.clipboard_drawer.trim()),
    ];
    let mut seen: HashMap<String, &str> = HashMap::new();
    for (label, raw) in entries {
        if raw.is_empty() {
            continue;
        }
        let norm = normalize_hotkey_combo(raw);
        if norm.is_empty() {
            continue;
        }
        if let Some(prev_label) = seen.insert(norm, label) {
            return Err(format!(
                "Duplicate global shortcut '{raw}' assigned to both '{prev_label}' and '{label}'."
            ));
        }
    }
    Ok(())
}

pub fn register_configured_hotkeys(
    app: &AppHandle,
    cfg: &PillvynConfig,
) -> Result<Vec<String>, String> {
    validate_hotkeys_uniqueness(cfg)?;

    let gs = app.global_shortcut();
    let _ = gs.unregister_all();

    let pairs = [
        (
            cfg.hotkeys.toggle_expand.trim(),
            "toggle_expand",
            "Expand / Collapse",
        ),
        (
            cfg.hotkeys.toggle_visibility.trim(),
            "toggle_visibility",
            "Show / Hide Island",
        ),
        (
            cfg.hotkeys.toggle_dnd.trim(),
            "toggle_dnd",
            "Do Not Disturb",
        ),
        (
            cfg.hotkeys.clipboard_drawer.trim(),
            "clipboard_drawer",
            "Clipboard Drawer",
        ),
    ];

    let mut warnings = Vec::new();
    for (combo, action_name, display_label) in pairs {
        if combo.is_empty() {
            continue;
        }
        let action = action_name.to_string();
        if let Err(err) = gs.on_shortcut(combo, move |app_handle, _shortcut, event| {
            if event.state != ShortcutState::Pressed {
                return;
            }
            handle_hotkey_action(app_handle, &action);
        }) {
            warn!("Could not register global shortcut '{combo}' ({action_name}): {err}");
            warnings.push(format!("{display_label} ('{combo}'): {err}"));
        }
    }
    Ok(warnings)
}

pub(crate) fn emit_island_state(app: &AppHandle) {
    let Some(state) = app.try_state::<AppState>() else {
        return;
    };
    let visible = state
        .window_state
        .lock()
        .map(|ws| ws.is_visible)
        .unwrap_or(true);
    let expanded = state.island_expanded.lock().map(|g| *g).unwrap_or(false);
    let _ = app.emit(
        "pillvyn://island-state",
        serde_json::json!({
            "visible": visible,
            "expanded": expanded,
        }),
    );
    let _ = app.emit("pillvyn://tray-refresh", ());
}

pub(crate) fn handle_hotkey_action(app: &AppHandle, action: &str) {
    match action {
        "toggle_visibility" => toggle_island_visibility(app),
        "toggle_expand" => {
            let mut next_expanded = true;
            if let Some(w) = app.get_webview_window("island") {
                if let Some(state) = app.try_state::<AppState>() {
                    let mut was_hidden = false;
                    if let Ok(mut ws) = state.window_state.lock() {
                        if !ws.is_visible {
                            ws.is_visible = true;
                            was_hidden = true;
                        }
                    }
                    if let Ok(mut exp) = state.island_expanded.lock() {
                        next_expanded = if was_hidden { true } else { !*exp };
                        *exp = next_expanded;
                    }
                    if next_expanded {
                        if let Ok(mut ts) = state.last_expanded_ms.lock() {
                            *ts = now_ms();
                        }
                    } else if let Ok(mut pinned) = state.island_pinned.lock() {
                        *pinned = false;
                    }
                    if was_hidden {
                        if let Ok(cfg) = state.config.lock() {
                            let _ = apply_window_geometry(&w, &cfg);
                        }
                    }
                }
                if !w.is_visible().unwrap_or(false) {
                    let _ = w.show();
                }
                enforce_tool_window(&w);
            }
            emit_island_state(app);
            let _ = app.emit("pillvyn://set-expanded", next_expanded);
        }
        "clipboard_drawer" => {
            if let Some(w) = app.get_webview_window("island") {
                if let Some(state) = app.try_state::<AppState>() {
                    let mut was_hidden = false;
                    if let Ok(mut ws) = state.window_state.lock() {
                        if !ws.is_visible {
                            ws.is_visible = true;
                            was_hidden = true;
                        }
                    }
                    if let Ok(mut exp) = state.island_expanded.lock() {
                        *exp = true;
                    }
                    if let Ok(mut ts) = state.last_expanded_ms.lock() {
                        *ts = now_ms();
                    }
                    if was_hidden {
                        if let Ok(cfg) = state.config.lock() {
                            let _ = apply_window_geometry(&w, &cfg);
                        }
                    }
                }
                if !w.is_visible().unwrap_or(false) {
                    let _ = w.show();
                }
                enforce_tool_window(&w);
            }
            emit_island_state(app);
            let _ = app.emit("pillvyn://hotkey", "clipboard_drawer");
        }
        "toggle_dnd" => toggle_dnd_flag(app),
        _ => {}
    }
}

pub(crate) fn toggle_island_visibility(app: &AppHandle) {
    let state = app.state::<AppState>();
    if let Some(win) = app.get_webview_window("island") {
        let visible = win.is_visible().unwrap_or(true);
        if visible {
            let _ = win.hide();
            if let Ok(mut ws) = state.window_state.lock() {
                ws.is_visible = false;
            }
        } else {
            if let Ok(cfg) = state.config.lock() {
                let _ = apply_window_geometry(&win, &cfg);
            }
            let _ = win.show();
            if let Ok(mut ws) = state.window_state.lock() {
                ws.is_visible = true;
            }
        }
        emit_island_state(app);
    }
}

pub(crate) fn toggle_dnd_flag(app: &AppHandle) {
    let state = app.state::<AppState>();
    let updated = {
        let Ok(mut cfg) = state.config.lock() else {
            return;
        };
        cfg.privacy.dnd_mode = !cfg.privacy.dnd_mode;
        let _ = save_config(&cfg);
        cfg.clone()
    };
    let _ = app.emit("pillvyn://config", updated.clone());
    let _ = app.emit("pillvyn://tray-refresh", ());

    if let Ok(mut act) = state.activities.lock() {
        let now = now_ms();
        act.upsert(LiveActivity {
            id: "hud-dnd".into(),
            kind: ActivityKind::SystemHud,
            title: if updated.privacy.dnd_mode {
                "Do Not Disturb On".into()
            } else {
                "Do Not Disturb Off".into()
            },
            subtitle: if updated.privacy.dnd_mode {
                "Non-critical notifications muted".into()
            } else {
                "All alerts enabled".into()
            },
            accent: None,
            badge: Some(
                if updated.privacy.dnd_mode {
                    "DND"
                } else {
                    "LIVE"
                }
                .into(),
            ),
            progress: None,
            priority: 90,
            updated_at_ms: now,
            expires_at_ms: Some(now + 2600),
        });
        let _ = app.emit("pillvyn://activities", act.snapshot());
    };
}

pub(crate) fn toggle_focus_flag(app: &AppHandle) {
    let state = app.state::<AppState>();
    let updated = {
        let Ok(mut cfg) = state.config.lock() else {
            return;
        };
        cfg.privacy.focus_mode = !cfg.privacy.focus_mode;
        let _ = save_config(&cfg);
        cfg.clone()
    };
    let _ = app.emit("pillvyn://config", updated.clone());
    let _ = app.emit("pillvyn://tray-refresh", ());

    if let Ok(mut act) = state.activities.lock() {
        let now = now_ms();
        act.upsert(LiveActivity {
            id: "hud-focus".into(),
            kind: ActivityKind::SystemHud,
            title: if updated.privacy.focus_mode {
                "Focus Mode On".into()
            } else {
                "Focus Mode Off".into()
            },
            subtitle: if updated.privacy.focus_mode {
                "Quiet minimal workspace active".into()
            } else {
                "Standard island feed restored".into()
            },
            accent: None,
            badge: Some(
                if updated.privacy.focus_mode {
                    "FOCUS"
                } else {
                    "LIVE"
                }
                .into(),
            ),
            progress: None,
            priority: 88,
            updated_at_ms: now,
            expires_at_ms: Some(now + 2400),
        });
        let _ = app.emit("pillvyn://activities", act.snapshot());
    };
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let _ = tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "pillvyn=info".into()),
        )
        .try_init();

    let initial_cfg = load_config();
    let media_notify = Arc::new(Notify::new());
    let system_notify = Arc::new(Notify::new());
    let foreground_notify = Arc::new(Notify::new());

    let app_state = AppState {
        config: Mutex::new(initial_cfg.clone()),
        window_state: Arc::new(Mutex::new(IslandWindowState::default())),
        activities: Arc::new(Mutex::new(ActivityCoordinator::default())),
        notifications: Arc::new(Mutex::new(NotificationHub::new())),
        clipboard: Arc::new(Mutex::new(ClipboardStore::new(
            initial_cfg.privacy.clipboard_enabled,
            initial_cfg.privacy.clipboard_skip_sensitive,
            initial_cfg.privacy.clipboard_max_items,
        ))),
        widgets: Arc::new(Mutex::new(WidgetEngine::new())),
        latest_media: Mutex::new(MediaSnapshot::default()),
        latest_system: Mutex::new(SystemControlsSnapshot::default()),
        latest_hardware: Mutex::new(None),
        latest_gaming: Mutex::new(None),
        latest_downloads: Mutex::new(DownloadsSnapshot {
            watched_sources: Vec::new(),
            active_count: 0,
            items: Vec::new(),
        }),
        latest_network: Mutex::new(None),
        island_expanded: Mutex::new(false),
        active_tab: Mutex::new("overview".to_string()),
        active_profile: Mutex::new(None),
        island_pinned: Mutex::new(false),
        last_expanded_ms: Mutex::new(0),
        last_local_audio_action_ms: Mutex::new(0),
        config_warnings: Mutex::new(Vec::new()),
        media_notify,
        system_notify,
        foreground_notify,
    };

    tauri::Builder::default()
        .manage(app_state)
        .plugin(tauri_plugin_single_instance::init(|app, _argv, _cwd| {
            handle_hotkey_action(app, "toggle_expand");
        }))
        .plugin(tauri_plugin_autostart::init(
            MacosLauncher::LaunchAgent,
            None,
        ))
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .setup(|app| {
            let handle = app.handle().clone();
            let state = handle.state::<AppState>();

            if let Ok(actual_autostart) = handle.autolaunch().is_enabled() {
                if let Ok(mut cfg) = state.config.lock() {
                    if cfg.autostart != actual_autostart {
                        cfg.autostart = actual_autostart;
                        let _ = save_config(&cfg);
                    }
                }
            }

            if let Some(island_win) = handle.get_webview_window("island") {
                if let Ok(cfg) = state.config.lock() {
                    let _ = apply_window_geometry(&island_win, &cfg);
                    match register_configured_hotkeys(&handle, &cfg) {
                        Ok(warnings) => {
                            if let Ok(mut guard) = state.config_warnings.lock() {
                                *guard = warnings;
                            }
                        }
                        Err(err) => {
                            warn!("Hotkey configuration error: {err}");
                            if let Ok(mut guard) = state.config_warnings.lock() {
                                *guard = vec![err];
                            }
                        }
                    }
                }
            }

            let _ = ensure_tray_window(&handle);
            setup_tray(&handle)?;

            spawn_click_through_controller(handle.clone(), state.window_state.clone());
            spawn_clipboard_listener(handle.clone(), state.clipboard.clone());
            spawn_background_services(handle);

            info!("PILLVYN initialized cleanly");
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_initial_state,
            get_monitors,
            update_island_hitbox,
            set_island_pinned,
            save_app_config,
            save_settings_config,
            validate_quick_action_cmd,
            preview_appearance,
            cancel_appearance_preview,
            get_tray_state,
            resize_tray_window,
            tray_action,
            open_settings_window,
            media_control,
            set_volume,
            set_mute,
            switch_audio_device,
            set_display_brightness,
            push_notification,
            notification_action,
            clipboard_action,
            run_quick_action,
            open_url,
            timer_action,
            refresh_weather,
            plugin_action,
            ask_ai_assistant,
            check_updates,
            set_click_through_override
        ])
        .run(tauri::generate_context!())
        .expect("Failed to start PILLVYN");
}

const TRAY_MENU_LOGICAL_WIDTH: f64 = 262.0;
const TRAY_MENU_LOGICAL_HEIGHT: f64 = 324.0;

pub(crate) fn compute_tray_popup_position(
    click: (i32, i32),
    win_size: (i32, i32),
    work_area: (i32, i32, i32, i32),
    pad: i32,
) -> (i32, i32) {
    let (click_x, click_y) = click;
    let (win_w, win_h) = win_size;
    let (wa_x, wa_y, wa_w, wa_h) = work_area;
    let min_x = wa_x + pad;
    let max_x = (wa_x + wa_w - win_w - pad).max(min_x);
    let x = (click_x - win_w / 2).clamp(min_x, max_x);

    let min_y = wa_y + pad;
    let max_y = (wa_y + wa_h - win_h - pad).max(min_y);
    let above_y = click_y - win_h - pad;
    let y = if above_y >= min_y {
        above_y.clamp(min_y, max_y)
    } else {
        (click_y + pad).clamp(min_y, max_y)
    };
    (x, y)
}

pub(crate) fn hide_tray_window(app: &AppHandle) {
    if let Some(win) = app.get_webview_window("tray") {
        let _ = win.hide();
    }
}

fn ensure_tray_window(app: &AppHandle) -> Option<WebviewWindow> {
    if let Some(existing) = app.get_webview_window("tray") {
        return Some(existing);
    }

    let builder = WebviewWindowBuilder::new(app, "tray", WebviewUrl::App("tray.html".into()))
        .title("PILLVYN Menu")
        .inner_size(TRAY_MENU_LOGICAL_WIDTH, TRAY_MENU_LOGICAL_HEIGHT)
        .decorations(false)
        .transparent(true)
        .resizable(false)
        .maximizable(false)
        .minimizable(false)
        .skip_taskbar(true)
        .always_on_top(true)
        .shadow(false)
        .visible(false)
        .focused(false)
        .background_color(tauri::window::Color(0, 0, 0, 0));

    match builder.build() {
        Ok(win) => {
            enforce_tool_window(&win);
            let win_clone = win.clone();
            win.on_window_event(move |event| match event {
                WindowEvent::Focused(false) => {
                    LAST_TRAY_BLUR_MS.store(now_ms(), Ordering::Relaxed);
                    let _ = win_clone.hide();
                }
                WindowEvent::CloseRequested { api, .. } => {
                    api.prevent_close();
                    let _ = win_clone.hide();
                }
                _ => {}
            });
            Some(win)
        }
        Err(err) => {
            warn!("Failed to build tray menu window: {err}");
            None
        }
    }
}

pub(crate) fn position_and_resize_tray(
    app: &AppHandle,
    tray_win: &WebviewWindow,
    logical_height: Option<f64>,
) {
    enforce_tool_window(tray_win);

    if let Some(h) = logical_height {
        let clamped_h = h.clamp(240.0, 440.0);
        let _ = tray_win.set_size(LogicalSize::new(TRAY_MENU_LOGICAL_WIDTH, clamped_h));
    }

    if !HAS_TRAY_CLICK.load(Ordering::Relaxed) {
        return;
    }
    let click_x = LAST_TRAY_CLICK_X.load(Ordering::Relaxed);
    let click_y = LAST_TRAY_CLICK_Y.load(Ordering::Relaxed);

    let monitor = app
        .available_monitors()
        .ok()
        .and_then(|list| {
            list.into_iter().find(|m| {
                let p = m.position();
                let s = m.size();
                click_x >= p.x
                    && click_x <= p.x + s.width as i32
                    && click_y >= p.y
                    && click_y <= p.y + s.height as i32
            })
        })
        .or_else(|| app.primary_monitor().ok().flatten());

    if let Some(m) = monitor {
        let scale = m.scale_factor().max(1.0);
        let size = tray_win.outer_size().unwrap_or_else(|_| {
            tauri::PhysicalSize::new(
                (TRAY_MENU_LOGICAL_WIDTH * scale).round() as u32,
                (TRAY_MENU_LOGICAL_HEIGHT * scale).round() as u32,
            )
        });
        let wa = m.work_area();
        let pad = (10.0 * scale).round() as i32;
        let (x, y) = compute_tray_popup_position(
            (click_x, click_y),
            (size.width as i32, size.height as i32),
            (
                wa.position.x,
                wa.position.y,
                wa.size.width as i32,
                wa.size.height as i32,
            ),
            pad,
        );
        let _ = tray_win.set_position(PhysicalPosition::new(x, y));
    }
}

fn show_tray_menu_near(app: &AppHandle, position: PhysicalPosition<f64>) {
    let Some(tray_win) = ensure_tray_window(app) else {
        return;
    };

    if tray_win.is_visible().unwrap_or(false) {
        let _ = tray_win.hide();
        return;
    }

    let elapsed_since_blur = now_ms().saturating_sub(LAST_TRAY_BLUR_MS.load(Ordering::Relaxed));
    if elapsed_since_blur < 220 {
        return;
    }

    LAST_TRAY_CLICK_X.store(position.x.round() as i32, Ordering::Relaxed);
    LAST_TRAY_CLICK_Y.store(position.y.round() as i32, Ordering::Relaxed);
    HAS_TRAY_CLICK.store(true, Ordering::Relaxed);

    position_and_resize_tray(app, &tray_win, None);

    let _ = app.emit("pillvyn://tray-refresh", ());
    let _ = tray_win.set_always_on_top(true);
    let _ = tray_win.show();
    let _ = tray_win.set_focus();
}

pub(crate) fn build_tray_state_snapshot(app: &AppHandle, state: &AppState) -> TrayStateSnapshot {
    let cfg = state.config.lock().map(|g| g.clone()).unwrap_or_default();
    let visible = state
        .window_state
        .lock()
        .map(|ws| ws.is_visible)
        .unwrap_or(true);
    let expanded = state.island_expanded.lock().map(|g| *g).unwrap_or(false);
    let media = state
        .latest_media
        .lock()
        .map(|g| g.clone())
        .unwrap_or_default();
    let active_activities = state
        .activities
        .lock()
        .map(|g| g.snapshot().len())
        .unwrap_or(0);

    TrayStateSnapshot {
        version: app.package_info().version.to_string(),
        visible,
        expanded,
        dnd: cfg.privacy.dnd_mode,
        focus_mode: cfg.privacy.focus_mode,
        theme: cfg.appearance.theme,
        accent_color: cfg.appearance.accent_color,
        opacity: cfg.appearance.opacity,
        media_active: media.active && !media.title.trim().is_empty(),
        media_title: media.title,
        media_artist: media.artist,
        media_status: media.status,
        media_app: media.source_app,
        active_activities,
    }
}

fn setup_tray(app: &AppHandle) -> Result<(), tauri::Error> {
    let icon = app
        .default_window_icon()
        .cloned()
        .expect("default window icon must exist");

    TrayIconBuilder::with_id("pillvyn-tray")
        .icon(icon)
        .tooltip("PILLVYN — Dynamic Island")
        .show_menu_on_left_click(false)
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button,
                button_state: MouseButtonState::Up,
                position,
                ..
            } = event
            {
                let app = tray.app_handle();
                match button {
                    MouseButton::Left => {
                        hide_tray_window(app);
                        handle_hotkey_action(app, "toggle_expand");
                    }
                    MouseButton::Right => {
                        show_tray_menu_near(app, position);
                    }
                    _ => {}
                }
            }
        })
        .build(app)?;

    Ok(())
}

fn spawn_background_services(app: AppHandle) {
    let (media_notify, system_notify, fg_notify) = {
        let st = app.state::<AppState>();
        (
            Arc::clone(&st.media_notify),
            Arc::clone(&st.system_notify),
            Arc::clone(&st.foreground_notify),
        )
    };

    // Start native Windows event listeners (WinRT GSMTC, WASAPI callbacks, SetWinEventHook)
    spawn_media_event_listener(Arc::clone(&media_notify));
    spawn_audio_event_listener(Arc::clone(&system_notify));
    spawn_foreground_hook(Arc::clone(&fg_notify));

    // 1. Event-driven fast loop: GSMTC media, active timers/stopwatch, Live Activity expiry
    let app_fast = app.clone();
    let media_wake = Arc::clone(&media_notify);
    tauri::async_runtime::spawn(async move {
        let mut thumb_cache = ThumbnailCache::default();
        loop {
            let (media_enabled, expanded) = {
                let state = app_fast.state::<AppState>();
                let m = state.config.lock().map(|c| c.modules.music).unwrap_or(true);
                let e = state.island_expanded.lock().map(|g| *g).unwrap_or(false);
                (m, e)
            };

            let mut any_fast_active = expanded;

            if media_enabled {
                let (snap, next_cache) = tokio::task::spawn_blocking(move || {
                    let s = query_media_snapshot(&mut thumb_cache);
                    (s, thumb_cache)
                })
                .await
                .unwrap_or_else(|_| (MediaSnapshot::default(), ThumbnailCache::default()));
                thumb_cache = next_cache;

                if snap.status == "Playing" {
                    any_fast_active = true;
                }

                let state = app_fast.state::<AppState>();
                let changed = if let Ok(mut prev) = state.latest_media.lock() {
                    let diff = *prev != snap;
                    if diff {
                        *prev = snap.clone();
                    }
                    diff
                } else {
                    false
                };

                if changed {
                    let _ = app_fast.emit("pillvyn://media", snap.clone());
                    if let Ok(mut act) = state.activities.lock() {
                        if snap.active && (snap.status == "Playing" || snap.status == "Paused") {
                            let prog = if snap.duration_sec > 0.5 {
                                Some((snap.position_sec / snap.duration_sec).clamp(0.0, 1.0))
                            } else {
                                None
                            };
                            act.upsert(LiveActivity {
                                id: "media".into(),
                                kind: ActivityKind::Media,
                                title: if snap.title.is_empty() {
                                    "Now Playing".into()
                                } else {
                                    snap.title.clone()
                                },
                                subtitle: if snap.artist.is_empty() {
                                    snap.source_app.clone()
                                } else {
                                    format!("{} · {}", snap.artist, snap.source_app)
                                },
                                accent: None,
                                badge: Some(
                                    if snap.status == "Playing" {
                                        "PLAY"
                                    } else {
                                        "PAUSE"
                                    }
                                    .into(),
                                ),
                                progress: prog,
                                priority: if snap.status == "Playing" { 75 } else { 45 },
                                updated_at_ms: now_ms(),
                                expires_at_ms: None,
                            });
                        } else {
                            act.remove("media");
                        }
                        let _ = app_fast.emit("pillvyn://activities", act.snapshot());
                    }
                }
            } else {
                let state = app_fast.state::<AppState>();
                if let Ok(mut act) = state.activities.lock() {
                    if act.remove("media") {
                        let _ = app_fast.emit("pillvyn://activities", act.snapshot());
                    }
                };
            }

            // Advance active timers & stopwatch
            {
                let state = app_fast.state::<AppState>();
                let (has_active, completed_list, timers, sw_ms, sw_run) =
                    if let Ok(mut w) = state.widgets.lock() {
                        let active_now = w.stopwatch_running
                            || w.timers.iter().any(|t| t.running && !t.completed);
                        let done = if active_now { w.tick() } else { Vec::new() };
                        (
                            active_now,
                            done,
                            w.timers.clone(),
                            w.stopwatch_elapsed_ms,
                            w.stopwatch_running,
                        )
                    } else {
                        (false, Vec::new(), Vec::new(), 0, false)
                    };

                if has_active {
                    any_fast_active = true;
                    sync_timer_activity(&app_fast, &state, &timers);
                    let _ = app_fast.emit(
                        "pillvyn://widgets-timers",
                        serde_json::json!({
                            "timers": timers,
                            "stopwatch_ms": sw_ms,
                            "stopwatch_running": sw_run,
                        }),
                    );
                }

                for done_timer in completed_list {
                    let dnd = state
                        .config
                        .lock()
                        .map(|c| c.privacy.dnd_mode || c.privacy.focus_mode)
                        .unwrap_or(false);
                    if let Ok(mut hub) = state.notifications.lock() {
                        hub.push(
                            "Timer".into(),
                            format!("{} Finished", done_timer.label),
                            "Countdown reached 00:00".into(),
                            NotificationPriority::High,
                            "widgets".into(),
                            dnd,
                        );
                        let _ = app_fast.emit("pillvyn://notifications", hub.snapshot());
                    }
                    if let Ok(mut act) = state.activities.lock() {
                        let now = now_ms();
                        act.upsert(LiveActivity {
                            id: format!("timer-done-{}", done_timer.id),
                            kind: ActivityKind::Notification,
                            title: format!("{} Done", done_timer.label),
                            subtitle: "Countdown finished".into(),
                            accent: None,
                            badge: Some("ALARM".into()),
                            progress: Some(1.0),
                            priority: 98,
                            updated_at_ms: now,
                            expires_at_ms: Some(now + 6000),
                        });
                        let _ = app_fast.emit("pillvyn://activities", act.snapshot());
                    }
                }
            }

            // Prune expired transient activities and keep wake interval tight only while expiring
            let has_expiring = {
                let state = app_fast.state::<AppState>();
                let res = if let Ok(mut act) = state.activities.lock() {
                    if act.prune_expired() {
                        let _ = app_fast.emit("pillvyn://activities", act.snapshot());
                    }
                    act.snapshot().iter().any(|a| a.expires_at_ms.is_some())
                } else {
                    false
                };
                res
            };

            let delay_ms = if any_fast_active {
                900
            } else if has_expiring {
                550
            } else {
                10_000
            };

            tokio::select! {
                _ = media_wake.notified() => {
                    // Coalesce burst WinRT property events briefly so track switches settle
                    tokio::time::sleep(std::time::Duration::from_millis(30)).await;
                }
                _ = tokio::time::sleep(std::time::Duration::from_millis(delay_ms)) => {}
            }
        }
    });

    // 2. Event-driven Audio / System Controls loop (WASAPI callbacks + periodic battery/display sync)
    let app_audio = app.clone();
    let audio_wake = Arc::clone(&system_notify);
    let hud_expire_wake = Arc::clone(&media_notify);
    tauri::async_runtime::spawn(async move {
        let mut cached_brightness: Option<Option<u32>> = None;
        let mut cycle: u64 = 0;
        let mut prev_audio: Option<(f32, bool, String, String)> = None;

        loop {
            let force_slow = cycle % 3 == 0;
            let (sys_snap, next_cached_b) = tokio::task::spawn_blocking(move || {
                let mut cb = cached_brightness;
                let snap = query_system_snapshot(force_slow, &mut cb);
                (snap, cb)
            })
            .await
            .unwrap_or_else(|_| (query_system_snapshot(false, &mut None), cached_brightness));
            cached_brightness = next_cached_b;

            let active_out_id = sys_snap
                .output_devices
                .iter()
                .find(|d| d.is_default)
                .map(|d| (d.id.clone(), d.name.clone()))
                .unwrap_or_default();

            {
                let st = app_audio.state::<AppState>();
                if let Ok(mut g) = st.latest_system.lock() {
                    *g = sys_snap.clone();
                }
                let _ = app_audio.emit("pillvyn://system", sys_snap.clone());

                if let Some((pv, pm, pid, _)) = prev_audio.as_ref() {
                    let vol_changed = (sys_snap.output_volume - *pv).abs() > 0.008;
                    let mute_changed = sys_snap.output_muted != *pm;
                    let dev_changed = !active_out_id.0.is_empty() && active_out_id.0 != *pid;
                    let last_local = st
                        .last_local_audio_action_ms
                        .lock()
                        .map(|g| *g)
                        .unwrap_or(0);
                    let show_hud = st
                        .config
                        .lock()
                        .map(|c| c.appearance.show_volume_hud)
                        .unwrap_or(true);

                    if show_hud
                        && (vol_changed || mute_changed || dev_changed)
                        && now_ms().saturating_sub(last_local) > 650
                    {
                        let (title, subtitle, prog) = if dev_changed && !active_out_id.1.is_empty()
                        {
                            ("Audio Output", active_out_id.1.clone(), None)
                        } else if sys_snap.output_muted {
                            ("Muted", "System audio muted".into(), Some(0.0))
                        } else {
                            let pct =
                                (sys_snap.output_volume.clamp(0.0, 1.0) * 100.0).round() as u32;
                            (
                                "Volume",
                                format!("{pct}%"),
                                Some(sys_snap.output_volume.clamp(0.0, 1.0) as f64),
                            )
                        };
                        if let Ok(mut act) = st.activities.lock() {
                            let now = now_ms();
                            act.upsert(LiveActivity {
                                id: "hud-volume".into(),
                                kind: ActivityKind::SystemHud,
                                title: title.into(),
                                subtitle,
                                accent: None,
                                badge: Some("SYS".into()),
                                progress: prog,
                                priority: 92,
                                updated_at_ms: now,
                                expires_at_ms: Some(now + 2100),
                            });
                            let _ = app_audio.emit("pillvyn://activities", act.snapshot());
                            hud_expire_wake.notify_one();
                        }
                    }
                }
                prev_audio = Some((
                    sys_snap.output_volume,
                    sys_snap.output_muted,
                    active_out_id.0,
                    active_out_id.1,
                ));
            }

            cycle = cycle.wrapping_add(1);
            tokio::select! {
                _ = audio_wake.notified() => {
                    tokio::time::sleep(std::time::Duration::from_millis(28)).await;
                }
                _ = tokio::time::sleep(std::time::Duration::from_secs(10)) => {}
            }
        }
    });

    // 3. Event-driven Foreground Hook & Global App Profiles loop
    let app_fg = app.clone();
    let fg_wake = Arc::clone(&fg_notify);
    tauri::async_runtime::spawn(async move {
        let mut last_profile_id: Option<String> = None;
        loop {
            tokio::select! {
                _ = fg_wake.notified() => {
                    tokio::time::sleep(std::time::Duration::from_millis(40)).await;
                }
                _ = tokio::time::sleep(std::time::Duration::from_secs(6)) => {}
            }

            let cfg = app_fg
                .state::<AppState>()
                .config
                .lock()
                .map(|c| c.clone())
                .unwrap_or_default();

            if take_external_foreground_flag() && cfg.appearance.click_outside_dismiss {
                let st = app_fg.state::<AppState>();
                let expanded = st.island_expanded.lock().map(|g| *g).unwrap_or(false);
                let pinned = st.island_pinned.lock().map(|g| *g).unwrap_or(false);
                let last_exp = st.last_expanded_ms.lock().map(|g| *g).unwrap_or(0);
                if expanded && !pinned && now_ms().saturating_sub(last_exp) > 350 {
                    let _ = app_fg.emit("pillvyn://dismiss-expanded", "foreground_change");
                }
            }

            let fg = tokio::task::spawn_blocking(query_foreground_window_process)
                .await
                .ok()
                .flatten();

            if let Some(info) = fg {
                if !info.exe_name.eq_ignore_ascii_case("pillvyn.exe") {
                    let matched: Option<AppProfile> = cfg
                        .profiles
                        .iter()
                        .find(|p| p.process_name.trim().eq_ignore_ascii_case(&info.exe_name))
                        .cloned();
                    let next_id = matched.as_ref().map(|p| p.id.clone());
                    if next_id != last_profile_id {
                        last_profile_id = next_id;
                        let st = app_fg.state::<AppState>();
                        if let Ok(mut ap) = st.active_profile.lock() {
                            *ap = matched.clone();
                        }
                        let _ = app_fg.emit("pillvyn://profile-applied", matched);
                    }
                }
            }
        }
    });

    // 4. Adaptive Telemetry loop: Hardware, Gaming, Downloads, Network
    let app_slow = app.clone();
    tauri::async_runtime::spawn(async move {
        let mut hw_collector = HardwareCollector::new();
        let mut gaming_mon = GamingMonitor::new();
        let mut dl_mon = DownloadMonitor::new();
        let mut net_mon = NetworkMonitor::new();

        // Prime initial weather in its own async task so first telemetry ticks run immediately
        let app_weather = app_slow.clone();
        tauri::async_runtime::spawn(async move {
            let init_cfg = app_weather
                .state::<AppState>()
                .config
                .lock()
                .map(|c| c.widgets.clone())
                .ok();
            if let Some(wcfg) = init_cfg {
                if wcfg.weather_enabled {
                    if let Ok(wsnap) = fetch_live_weather(&wcfg).await {
                        let st = app_weather.state::<AppState>();
                        if let Ok(mut w) = st.widgets.lock() {
                            w.weather = wsnap.clone();
                        }
                        let _ = app_weather.emit("pillvyn://weather", wsnap);
                    }
                }
            }
        });

        loop {
            let (cfg, on_battery_low_power) = {
                let st = app_slow.state::<AppState>();
                let cfg = st.config.lock().map(|c| c.clone()).unwrap_or_default();
                let bat_low = cfg.performance.low_power_on_battery
                    && st
                        .latest_system
                        .lock()
                        .ok()
                        .is_some_and(|s| s.battery.present && !s.battery.ac_online);
                (cfg, bat_low)
            };

            if cfg.modules.hardware || cfg.modules.gaming {
                let (hw_snap, next_hw) = tokio::task::spawn_blocking(move || {
                    let s = hw_collector.collect();
                    (s, hw_collector)
                })
                .await
                .unwrap();
                hw_collector = next_hw;

                let st = app_slow.state::<AppState>();
                if let Ok(mut g) = st.latest_hardware.lock() {
                    *g = Some(hw_snap.clone());
                }
                let _ = app_slow.emit("pillvyn://hardware", hw_snap);
            }

            if cfg.modules.gaming {
                let gaming_cfg = cfg.gaming.clone();
                let ((gm_snap, _fg_exe), next_gm) = tokio::task::spawn_blocking(move || {
                    let res = gaming_mon.collect(&gaming_cfg);
                    (res, gaming_mon)
                })
                .await
                .unwrap();
                gaming_mon = next_gm;

                let st = app_slow.state::<AppState>();
                if let Ok(mut g) = st.latest_gaming.lock() {
                    *g = Some(gm_snap.clone());
                }
                if let Ok(mut ws) = st.window_state.lock() {
                    ws.force_click_through = gm_snap.active && cfg.gaming.click_through_hud;
                }
                if let Ok(mut act) = st.activities.lock() {
                    if gm_snap.active {
                        let title = gm_snap
                            .game_title
                            .clone()
                            .unwrap_or_else(|| "Game Active".into());
                        let sub = match (gm_snap.process_cpu_percent, gm_snap.process_ram_mb) {
                            (Some(c), Some(r)) => format!("CPU {:.0}% · {} MB", c, r),
                            _ => "Gaming Focus Active".into(),
                        };
                        act.upsert(LiveActivity {
                            id: "gaming".into(),
                            kind: ActivityKind::Gaming,
                            title,
                            subtitle: sub,
                            accent: None,
                            badge: Some(
                                if gm_snap.notifications_suppressed {
                                    "DND"
                                } else {
                                    "GAME"
                                }
                                .into(),
                            ),
                            progress: None,
                            priority: 72,
                            updated_at_ms: now_ms(),
                            expires_at_ms: None,
                        });
                    } else {
                        act.remove("gaming");
                    }
                    let _ = app_slow.emit("pillvyn://activities", act.snapshot());
                }

                let _ = app_slow.emit("pillvyn://gaming", gm_snap);
            } else {
                let st = app_slow.state::<AppState>();
                if let Ok(mut ws) = st.window_state.lock() {
                    ws.force_click_through = false;
                }
                if let Ok(mut act) = st.activities.lock() {
                    if act.remove("gaming") {
                        let _ = app_slow.emit("pillvyn://activities", act.snapshot());
                    }
                };
            }

            if cfg.modules.downloads {
                let (dl_snap, next_dl) = tokio::task::spawn_blocking(move || {
                    let s = dl_mon.collect();
                    (s, dl_mon)
                })
                .await
                .unwrap();
                dl_mon = next_dl;

                let st = app_slow.state::<AppState>();
                if let Ok(mut g) = st.latest_downloads.lock() {
                    *g = dl_snap.clone();
                }
                if let Ok(mut act) = st.activities.lock() {
                    if let Some(first_active) =
                        dl_snap.items.iter().find(|d| d.status == "downloading")
                    {
                        act.upsert(LiveActivity {
                            id: "download".into(),
                            kind: ActivityKind::Download,
                            title: first_active.name.clone(),
                            subtitle: format_dl_subtitle(
                                first_active.speed_bps,
                                first_active.progress_percent,
                            ),
                            accent: None,
                            badge: Some(
                                first_active
                                    .progress_percent
                                    .map(|p| format!("{:.0}%", p))
                                    .unwrap_or_else(|| "DL".into()),
                            ),
                            progress: first_active
                                .progress_percent
                                .map(|p| (p / 100.0).clamp(0.0, 1.0)),
                            priority: 78,
                            updated_at_ms: now_ms(),
                            expires_at_ms: None,
                        });
                    } else {
                        act.remove("download");
                    }
                    let _ = app_slow.emit("pillvyn://activities", act.snapshot());
                }
                let _ = app_slow.emit("pillvyn://downloads", dl_snap);
            } else {
                let st = app_slow.state::<AppState>();
                if let Ok(mut act) = st.activities.lock() {
                    if act.remove("download") {
                        let _ = app_slow.emit("pillvyn://activities", act.snapshot());
                    }
                };
            }

            if cfg.modules.network {
                let (net_snap, next_net) = tokio::task::spawn_blocking(move || {
                    let s = net_mon.collect();
                    (s, net_mon)
                })
                .await
                .unwrap();
                net_mon = next_net;

                let st = app_slow.state::<AppState>();
                if let Ok(mut g) = st.latest_network.lock() {
                    *g = Some(net_snap.clone());
                }
                let _ = app_slow.emit("pillvyn://network", net_snap);
            }

            let base_ms = cfg.performance.hardware_interval_ms.max(1200);
            let interval = if on_battery_low_power {
                base_ms.saturating_mul(2)
            } else {
                base_ms
            };
            tokio::time::sleep(std::time::Duration::from_millis(interval)).await;
        }
    });
}

fn format_dl_subtitle(bps: f64, pct: Option<f64>) -> String {
    let speed = if bps >= 1_048_576.0 {
        format!("{:.1} MB/s", bps / 1_048_576.0)
    } else if bps >= 1024.0 {
        format!("{:.0} KB/s", bps / 1024.0)
    } else {
        "Active".to_string()
    };
    match pct {
        Some(p) => format!("{:.0}% · {}", p, speed),
        None => speed,
    }
}

#[cfg(test)]
mod tests {
    use super::{compute_tray_popup_position, normalize_hotkey_combo, validate_hotkeys_uniqueness};
    use crate::config::PillvynConfig;
    use crate::services::actions::validate_quick_action;

    #[test]
    fn normalizes_and_rejects_duplicate_hotkeys() {
        assert_eq!(
            normalize_hotkey_combo("Ctrl + Alt + Space"),
            "ctrl+alt+space"
        );
        let mut cfg = PillvynConfig::default();
        assert!(validate_hotkeys_uniqueness(&cfg).is_ok());

        cfg.hotkeys.toggle_dnd = format!("  {}  ", cfg.hotkeys.toggle_expand.to_lowercase());
        let err = validate_hotkeys_uniqueness(&cfg).unwrap_err();
        assert!(err.contains("Duplicate global shortcut"));

        cfg.hotkeys.toggle_dnd.clear();
        assert!(validate_hotkeys_uniqueness(&cfg).is_ok());
    }

    #[test]
    fn clamps_tray_popup_inside_monitor_work_area() {
        // Bottom-right taskbar click on 1920x1080 display (taskbar 48px -> work_area height 1032)
        // Expected y clamps cleanly inside work area above taskbar (1032 - 324 - 10 = 698)
        let (x, y) = compute_tray_popup_position((1840, 1055), (262, 324), (0, 0, 1920, 1032), 10);
        assert_eq!(x, 1920 - 262 - 10);
        assert_eq!(y, 1032 - 324 - 10);

        // Overflow flyout click positions cleanly directly above overflow button
        let (x2, y2) = compute_tray_popup_position((1700, 920), (262, 324), (0, 0, 1920, 1032), 10);
        assert!(x2 >= 10 && x2 + 262 <= 1910);
        assert_eq!(y2, 920 - 324 - 10);

        // Top taskbar click places menu neatly below cursor
        let (_x3, y3) = compute_tray_popup_position((400, 36), (262, 324), (0, 48, 1920, 1032), 10);
        assert_eq!(y3, 58);
    }

    #[test]
    fn validates_quick_action_targets() {
        assert_eq!(
            validate_quick_action("system_tool", " wt.exe ").unwrap(),
            "wt.exe"
        );
        assert!(validate_quick_action("system_tool", "rm -rf /").is_err());
        assert_eq!(
            validate_quick_action("uri", "ms-settings:sound").unwrap(),
            "ms-settings:sound"
        );
        assert!(validate_quick_action("uri", "javascript:alert(1)").is_err());
        assert!(validate_quick_action("app", "cmd & whoami").is_err());
    }
}
