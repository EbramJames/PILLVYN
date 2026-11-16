use crate::config::{save_config, AppProfile, AppearanceConfig, PillvynConfig, QuickActionItem};
use crate::island::activities::{now_ms, ActivityCoordinator, ActivityKind, LiveActivity};
use crate::island::{
    apply_window_geometry, compute_island_window_position, enforce_tool_window, list_monitors,
    HitboxRect, IslandWindowState, MonitorInfo,
};
use crate::services::actions::{execute_quick_action, open_external_url};
use crate::services::ai::{run_ai_prompt, AiPromptResponse};
use crate::services::clipboard::{ClipboardSnapshot, ClipboardStore};
use crate::services::downloads::DownloadsSnapshot;
use crate::services::gaming::GamingSnapshot;
use crate::services::hardware::HardwareSnapshot;
use crate::services::media::{control_media_for_source, MediaSnapshot};
use crate::services::network::NetworkSnapshot;
use crate::services::notifications::{
    AppNotification, NotificationHub, NotificationPriority, NotificationRequest,
    NotificationsSnapshot,
};
use crate::services::plugins::{
    create_example_plugin, discover_plugins, toggle_plugin_enabled, PluginsSnapshot,
};
use crate::services::sound::play_sms_ding;
use crate::services::system_ctrl::{
    set_brightness_percent, set_default_audio_device, set_endpoint_mute, set_endpoint_volume,
    SystemControlsSnapshot,
};
use crate::services::updater::{check_for_updates, UpdateCheckResult};
use crate::services::widgets::{fetch_live_weather, TimerEntry, WeatherSnapshot, WidgetEngine};
use serde::Serialize;
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter, Manager, State, WebviewUrl, WebviewWindowBuilder};
use tauri_plugin_autostart::ManagerExt as AutostartExt;
use tokio::sync::Notify;

pub struct AppState {
    pub config: Mutex<PillvynConfig>,
    pub window_state: Arc<Mutex<IslandWindowState>>,
    pub activities: Arc<Mutex<ActivityCoordinator>>,
    pub notifications: Arc<Mutex<NotificationHub>>,
    pub clipboard: Arc<Mutex<ClipboardStore>>,
    pub widgets: Arc<Mutex<WidgetEngine>>,
    pub latest_media: Mutex<MediaSnapshot>,
    pub latest_system: Mutex<SystemControlsSnapshot>,
    pub latest_hardware: Mutex<Option<HardwareSnapshot>>,
    pub latest_gaming: Mutex<Option<GamingSnapshot>>,
    pub latest_downloads: Mutex<DownloadsSnapshot>,
    pub latest_network: Mutex<Option<NetworkSnapshot>>,
    pub island_expanded: Mutex<bool>,
    pub active_tab: Mutex<String>,
    pub active_profile: Mutex<Option<AppProfile>>,
    pub island_pinned: Mutex<bool>,
    pub reposition_mode: Mutex<bool>,
    pub preview_appearance: Mutex<Option<AppearanceConfig>>,
    pub last_expanded_ms: Mutex<u64>,
    pub last_local_audio_action_ms: Mutex<u64>,
    pub config_warnings: Mutex<Vec<String>>,
    pub media_notify: Arc<Notify>,
    pub system_notify: Arc<Notify>,
    pub foreground_notify: Arc<Notify>,
    pub telemetry_notify: Arc<Notify>,
}

#[derive(Debug, Clone, Serialize)]
pub struct InitialDashboardPayload {
    pub config: PillvynConfig,
    pub monitors: Vec<MonitorInfo>,
    pub activities: Vec<LiveActivity>,
    pub media: MediaSnapshot,
    pub system: SystemControlsSnapshot,
    pub hardware: Option<HardwareSnapshot>,
    pub gaming: Option<GamingSnapshot>,
    pub downloads: DownloadsSnapshot,
    pub notifications: NotificationsSnapshot,
    pub clipboard: ClipboardSnapshot,
    pub network: Option<NetworkSnapshot>,
    pub timers: Vec<TimerEntry>,
    pub stopwatch_ms: u64,
    pub stopwatch_running: bool,
    pub weather: WeatherSnapshot,
    pub plugins: PluginsSnapshot,
    pub active_profile: Option<AppProfile>,
    pub island_pinned: bool,
    pub island_visible: bool,
    pub island_expanded: bool,
    pub config_warnings: Vec<String>,
    pub app_version: String,
}

#[tauri::command]
pub fn get_initial_state(app: AppHandle, state: State<'_, AppState>) -> InitialDashboardPayload {
    let config = state.config.lock().map(|g| g.clone()).unwrap_or_default();
    let monitors = list_monitors(&app);
    let activities = state
        .activities
        .lock()
        .map(|g| g.snapshot())
        .unwrap_or_default();
    let media = state
        .latest_media
        .lock()
        .map(|g| g.clone())
        .unwrap_or_default();
    let system = state
        .latest_system
        .lock()
        .map(|g| g.clone())
        .unwrap_or_else(|_| crate::services::system_ctrl::query_system_snapshot(false, &mut None));
    let hardware = state.latest_hardware.lock().ok().and_then(|g| g.clone());
    let gaming = state.latest_gaming.lock().ok().and_then(|g| g.clone());
    let downloads = state
        .latest_downloads
        .lock()
        .map(|g| g.clone())
        .unwrap_or(DownloadsSnapshot {
            watched_sources: Vec::new(),
            active_count: 0,
            items: Vec::new(),
        });
    let notifications =
        state
            .notifications
            .lock()
            .map(|g| g.snapshot())
            .unwrap_or(NotificationsSnapshot {
                windows_listener_status: "unavailable".into(),
                windows_listener_detail: String::new(),
                unread_count: 0,
                items: Vec::new(),
            });
    let clipboard = state
        .clipboard
        .lock()
        .map(|g| g.snapshot())
        .unwrap_or(ClipboardSnapshot {
            enabled: true,
            items: Vec::new(),
        });
    let network = state.latest_network.lock().ok().and_then(|g| g.clone());
    let (timers, stopwatch_ms, stopwatch_running, weather) = state
        .widgets
        .lock()
        .map(|w| {
            (
                w.timers.clone(),
                w.stopwatch_elapsed_ms,
                w.stopwatch_running,
                w.weather.clone(),
            )
        })
        .unwrap_or_else(|_| (Vec::new(), 0, false, WeatherSnapshot::default()));
    let plugins = discover_plugins();
    let active_profile = state.active_profile.lock().ok().and_then(|g| g.clone());
    let island_pinned = state.island_pinned.lock().map(|g| *g).unwrap_or(false);
    let island_visible = state
        .window_state
        .lock()
        .map(|g| g.is_visible)
        .unwrap_or(true);
    let island_expanded = state.island_expanded.lock().map(|g| *g).unwrap_or(false);
    let config_warnings = state
        .config_warnings
        .lock()
        .map(|g| g.clone())
        .unwrap_or_default();

    InitialDashboardPayload {
        config,
        monitors,
        activities,
        media,
        system,
        hardware,
        gaming,
        downloads,
        notifications,
        clipboard,
        network,
        timers,
        stopwatch_ms,
        stopwatch_running,
        weather,
        plugins,
        active_profile,
        island_pinned,
        island_visible,
        island_expanded,
        config_warnings,
        app_version: app.package_info().version.to_string(),
    }
}

#[tauri::command]
pub fn get_monitors(app: AppHandle) -> Vec<MonitorInfo> {
    list_monitors(&app)
}

#[tauri::command]
pub fn update_island_hitbox(
    app: AppHandle,
    state: State<'_, AppState>,
    hitbox: HitboxRect,
    expanded: bool,
    active_tab: Option<String>,
) {
    if let Ok(mut guard) = state.window_state.lock() {
        guard.hitbox = HitboxRect {
            x: hitbox.x.clamp(0.0, 600.0),
            y: hitbox.y.clamp(0.0, 600.0),
            width: hitbox.width.clamp(40.0, 560.0),
            height: hitbox.height.clamp(16.0, 500.0),
        };
    }
    let mut became_expanded = false;
    let mut changed_expanded = false;
    if let Ok(mut exp) = state.island_expanded.lock() {
        if *exp != expanded {
            changed_expanded = true;
        }
        if expanded && !*exp {
            became_expanded = true;
        }
        *exp = expanded;
        if !expanded {
            if let Ok(mut pinned) = state.island_pinned.lock() {
                *pinned = false;
            }
        }
    }
    if became_expanded {
        if let Ok(mut ts) = state.last_expanded_ms.lock() {
            *ts = now_ms();
        }
    }
    if let Some(tab) = active_tab {
        if let Ok(mut t) = state.active_tab.lock() {
            *t = tab;
        }
    }
    if changed_expanded {
        if let Some(island) = app.get_webview_window("island") {
            let effective = effective_island_config(&state);
            let _ = apply_window_geometry(&island, &effective);
        }
        if became_expanded {
            state.telemetry_notify.notify_one();
        }
        crate::emit_island_state(&app);
    }
}

#[tauri::command]
pub fn set_island_pinned(app: AppHandle, state: State<'_, AppState>, pinned: bool) {
    if let Ok(mut p) = state.island_pinned.lock() {
        *p = pinned;
    }
    let _ = app.emit("pillvyn://pinned", pinned);
}

#[tauri::command]
pub fn save_app_config(
    app: AppHandle,
    state: State<'_, AppState>,
    mut new_config: PillvynConfig,
) -> Result<PillvynConfig, String> {
    new_config.autostart_initialized = true;
    new_config.sanitize();
    crate::validate_hotkeys_uniqueness(&new_config)?;
    save_config(&new_config)?;

    let prev_widgets = state.config.lock().ok().map(|g| {
        (
            g.modules.widgets,
            g.widgets.weather_enabled,
            g.widgets.weather_city.clone(),
        )
    });

    if let Ok(mut guard) = state.config.lock() {
        *guard = new_config.clone();
    }
    if let Ok(mut prev) = state.preview_appearance.lock() {
        *prev = None;
    }
    let was_reposition = state
        .reposition_mode
        .lock()
        .map(|mut r| {
            let prev = *r;
            *r = false;
            prev
        })
        .unwrap_or(false);
    if was_reposition {
        let _ = app.emit("pillvyn://reposition-mode", false);
    }
    if !new_config.modules.widgets {
        if let Ok(mut act) = state.activities.lock() {
            if act.remove("timer-primary") {
                let _ = app.emit("pillvyn://activities", act.snapshot());
            }
        }
    }
    if let Ok(mut cb) = state.clipboard.lock() {
        cb.update_policy(
            new_config.privacy.clipboard_enabled,
            new_config.privacy.clipboard_skip_sensitive,
            new_config.privacy.clipboard_max_items,
        );
    }
    if let Some(island) = app.get_webview_window("island") {
        let _ = apply_window_geometry(&island, &new_config);
    }
    let game_active = state
        .latest_gaming
        .lock()
        .ok()
        .and_then(|g| g.as_ref().map(|s| s.active))
        .unwrap_or(false);
    if let Ok(mut ws) = state.window_state.lock() {
        ws.force_click_through = new_config.modules.gaming
            && new_config.gaming.auto_detect
            && new_config.gaming.click_through_hud
            && game_active;
    }

    // Apply Windows autostart setting cleanly
    let autostart_mgr = app.autolaunch();
    let mut warnings: Vec<String> = Vec::new();
    if new_config.autostart {
        if let Err(err) = autostart_mgr.enable() {
            warnings.push(format!("Could not enable Windows startup: {err}"));
        }
    } else if let Err(err) = autostart_mgr.disable() {
        warnings.push(format!("Could not disable Windows startup: {err}"));
    }

    warnings.extend(crate::register_configured_hotkeys(&app, &new_config)?);
    if let Ok(mut ws_warn) = state.config_warnings.lock() {
        *ws_warn = warnings.clone();
    }

    let should_refresh_weather = new_config.modules.widgets
        && new_config.widgets.weather_enabled
        && prev_widgets.is_none_or(|(was_mod, was_weather, prev_city)| {
            !was_mod || !was_weather || prev_city != new_config.widgets.weather_city
        });
    if should_refresh_weather {
        let app_clone = app.clone();
        let weather_cfg = new_config.widgets.clone();
        tauri::async_runtime::spawn(async move {
            if let Ok(snap) = fetch_live_weather(&weather_cfg).await {
                if let Some(st) = app_clone.try_state::<AppState>() {
                    if let Ok(mut w) = st.widgets.lock() {
                        w.weather = snap.clone();
                    }
                }
                let _ = app_clone.emit("pillvyn://weather", snap);
            }
        });
    }

    state.foreground_notify.notify_one();
    state.system_notify.notify_one();
    state.telemetry_notify.notify_one();
    let _ = app.emit("pillvyn://config", new_config.clone());
    let _ = app.emit("pillvyn://config-warnings", warnings);
    let _ = app.emit("pillvyn://tray-refresh", ());
    Ok(new_config)
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct SaveConfigOutcome {
    pub config: PillvynConfig,
    pub warnings: Vec<String>,
}

#[tauri::command]
pub fn save_settings_config(
    app: AppHandle,
    state: State<'_, AppState>,
    new_config: PillvynConfig,
) -> Result<SaveConfigOutcome, String> {
    let config = save_app_config(app, state.clone(), new_config)?;
    let warnings = state
        .config_warnings
        .lock()
        .map(|g| g.clone())
        .unwrap_or_default();
    Ok(SaveConfigOutcome { config, warnings })
}

#[tauri::command]
pub fn validate_quick_action_cmd(kind: String, target: String) -> Result<String, String> {
    crate::services::actions::validate_quick_action(&kind, &target)
}

pub(crate) fn effective_island_config(state: &AppState) -> PillvynConfig {
    let mut cfg = state.config.lock().map(|c| c.clone()).unwrap_or_default();
    if let Ok(prev) = state.preview_appearance.lock() {
        if let Some(app_cfg) = prev.as_ref() {
            cfg.appearance = app_cfg.clone();
        }
    }
    cfg.sanitize();
    cfg
}

pub(crate) fn restore_saved_appearance(app: &AppHandle) {
    let Some(state) = app.try_state::<AppState>() else {
        return;
    };
    let had_preview = state
        .preview_appearance
        .lock()
        .map(|mut prev| prev.take().is_some())
        .unwrap_or(false);
    let was_reposition = state
        .reposition_mode
        .lock()
        .map(|mut rep| {
            let prev = *rep;
            *rep = false;
            prev
        })
        .unwrap_or(false);
    if was_reposition {
        let _ = app.emit("pillvyn://reposition-mode", false);
    }
    let cfg = state.config.lock().map(|c| c.clone()).unwrap_or_default();
    if had_preview || was_reposition {
        if let Some(w) = app.get_webview_window("island") {
            let _ = apply_window_geometry(&w, &cfg);
        }
        let _ = app.emit("pillvyn://appearance-preview", cfg.appearance.clone());
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct IslandPositionUpdate {
    pub top_offset: i32,
    pub x_offset: i32,
    pub monitor_name: Option<String>,
    pub committed: bool,
}

#[tauri::command]
pub fn preview_appearance(
    app: AppHandle,
    state: State<'_, AppState>,
    appearance: AppearanceConfig,
) -> Result<(), String> {
    let mut sanitized = state.config.lock().map(|c| c.clone()).unwrap_or_default();
    sanitized.appearance = appearance;
    sanitized.sanitize();
    if let Ok(mut prev) = state.preview_appearance.lock() {
        *prev = Some(sanitized.appearance.clone());
    }
    if let Some(w) = app.get_webview_window("island") {
        let _ = apply_window_geometry(&w, &sanitized);
    }
    let _ = app.emit("pillvyn://appearance-preview", sanitized.appearance);
    Ok(())
}

#[tauri::command]
pub fn cancel_appearance_preview(app: AppHandle, state: State<'_, AppState>) {
    let _ = state;
    restore_saved_appearance(&app);
}

#[tauri::command]
pub fn set_reposition_mode(
    app: AppHandle,
    state: State<'_, AppState>,
    enabled: bool,
) -> Result<bool, String> {
    if let Ok(mut guard) = state.reposition_mode.lock() {
        *guard = enabled;
    }
    if enabled {
        if let Ok(mut exp) = state.island_expanded.lock() {
            *exp = false;
        }
        if let Ok(mut pinned) = state.island_pinned.lock() {
            *pinned = false;
        }
        if let Some(island) = app.get_webview_window("island") {
            if let Ok(mut ws) = state.window_state.lock() {
                ws.is_visible = true;
            }
            let effective_cfg = effective_island_config(&state);
            let _ = apply_window_geometry(&island, &effective_cfg);
            let _ = island.show();
            enforce_tool_window(&island);
        }
        crate::emit_island_state(&app);
    }
    let _ = app.emit("pillvyn://reposition-mode", enabled);
    Ok(enabled)
}

#[tauri::command]
pub fn move_island_position(
    app: AppHandle,
    state: State<'_, AppState>,
    top_offset: i32,
    x_offset: i32,
    monitor_name: Option<String>,
    commit: bool,
) -> Result<IslandPositionUpdate, String> {
    let monitors = list_monitors(&app);
    let mut cfg = if commit {
        state.config.lock().map(|g| g.clone()).unwrap_or_default()
    } else {
        effective_island_config(&state)
    };

    if let Some(m_name) = monitor_name {
        let trimmed = m_name.trim().to_string();
        cfg.appearance.monitor_name = if trimmed.is_empty() {
            None
        } else {
            Some(trimmed)
        };
    }

    let target = cfg
        .appearance
        .monitor_name
        .as_ref()
        .and_then(|wanted| monitors.iter().find(|m| &m.name == wanted))
        .or_else(|| monitors.iter().find(|m| m.is_primary))
        .or_else(|| monitors.first());

    let (clamped_x, clamped_top) = if let Some(m) = target {
        let (_, clamped) = compute_island_window_position(m, top_offset, x_offset, false);
        clamped
    } else {
        (x_offset.clamp(-3000, 3000), top_offset.clamp(0, 2400))
    };

    cfg.appearance.x_offset = clamped_x;
    cfg.appearance.top_offset = clamped_top;
    cfg.sanitize();

    if commit {
        save_config(&cfg)?;
        if let Ok(mut guard) = state.config.lock() {
            *guard = cfg.clone();
        }
        if let Ok(mut prev) = state.preview_appearance.lock() {
            *prev = None;
        }
        let _ = app.emit("pillvyn://config", cfg.clone());
    } else {
        if let Ok(mut prev) = state.preview_appearance.lock() {
            *prev = Some(cfg.appearance.clone());
        }
        let _ = app.emit("pillvyn://appearance-preview", cfg.appearance.clone());
    }

    if let Some(island) = app.get_webview_window("island") {
        let _ = apply_window_geometry(&island, &cfg);
    }

    let payload = IslandPositionUpdate {
        top_offset: cfg.appearance.top_offset,
        x_offset: cfg.appearance.x_offset,
        monitor_name: cfg.appearance.monitor_name.clone(),
        committed: commit,
    };
    let _ = app.emit("pillvyn://position-updated", payload.clone());
    Ok(payload)
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct TrayStateSnapshot {
    pub version: String,
    pub visible: bool,
    pub expanded: bool,
    pub dnd: bool,
    pub focus_mode: bool,
    pub theme: String,
    pub accent_color: String,
    pub opacity: f64,
    pub show_emojis: bool,
    pub reduced_motion: bool,
    pub media_active: bool,
    pub media_title: String,
    pub media_artist: String,
    pub media_status: String,
    pub media_app: String,
    pub active_activities: usize,
}

#[tauri::command]
pub fn get_tray_state(app: AppHandle, state: State<'_, AppState>) -> TrayStateSnapshot {
    crate::build_tray_state_snapshot(&app, &state)
}

#[tauri::command]
pub fn resize_tray_window(app: AppHandle, height: f64) {
    if let Some(tray_win) = app.get_webview_window("tray") {
        if tray_win.is_visible().unwrap_or(false) {
            crate::position_and_resize_tray(&app, &tray_win, Some(height));
        }
    }
}

#[tauri::command]
pub async fn tray_action(
    app: AppHandle,
    state: State<'_, AppState>,
    action: String,
) -> Result<TrayStateSnapshot, String> {
    match action.as_str() {
        "toggle_visibility" => crate::toggle_island_visibility(&app),
        "toggle_expand" => crate::handle_hotkey_action(&app, "toggle_expand"),
        "toggle_dnd" => crate::toggle_dnd_flag(&app),
        "toggle_focus" => crate::toggle_focus_flag(&app),
        "open_settings" => {
            crate::hide_tray_window(&app);
            open_settings_window(app.clone()).await?;
        }
        "hide_tray" => crate::hide_tray_window(&app),
        "quit" => {
            crate::hide_tray_window(&app);
            app.exit(0);
        }
        other => return Err(format!("Unknown tray action: {other}")),
    }
    Ok(crate::build_tray_state_snapshot(&app, &state))
}

#[tauri::command]
pub async fn open_settings_window(app: AppHandle) -> Result<(), String> {
    if let Some(existing) = app.get_webview_window("settings") {
        let _ = existing.show();
        let _ = existing.unminimize();
        let _ = existing.set_focus();
        return Ok(());
    }

    let builder =
        WebviewWindowBuilder::new(&app, "settings", WebviewUrl::App("settings.html".into()))
            .title("PILLVYN — Settings Studio")
            .inner_size(940.0, 660.0)
            .min_inner_size(760.0, 520.0)
            .background_color(tauri::window::Color(10, 13, 21, 255))
            .center()
            .decorations(true)
            .resizable(true);

    let win = builder
        .build()
        .map_err(|e| format!("Failed to open Settings Studio: {e}"))?;

    let handle_for_close = app.clone();
    win.on_window_event(move |event| {
        if matches!(
            event,
            tauri::WindowEvent::CloseRequested { .. } | tauri::WindowEvent::Destroyed
        ) {
            restore_saved_appearance(&handle_for_close);
        }
    });

    #[cfg(target_os = "windows")]
    apply_dark_titlebar(&win);

    let _ = win.set_focus();
    Ok(())
}

#[cfg(target_os = "windows")]
fn apply_dark_titlebar(window: &tauri::WebviewWindow) {
    use windows::Win32::Foundation::HWND;
    use windows::Win32::Graphics::Dwm::{DwmSetWindowAttribute, DWMWA_USE_IMMERSIVE_DARK_MODE};
    if let Ok(hwnd) = window.hwnd() {
        let dark: i32 = 1;
        unsafe {
            let _ = DwmSetWindowAttribute(
                HWND(hwnd.0 as *mut _),
                DWMWA_USE_IMMERSIVE_DARK_MODE,
                &dark as *const i32 as *const _,
                std::mem::size_of::<i32>() as u32,
            );
        }
    }
}

#[tauri::command]
pub fn media_control(
    state: State<'_, AppState>,
    action: String,
    seek_sec: Option<f64>,
    source_id: Option<String>,
) -> Result<(), String> {
    control_media_for_source(&action, seek_sec, source_id.as_deref())?;
    state.media_notify.notify_one();
    Ok(())
}

#[tauri::command]
pub fn set_volume(
    app: AppHandle,
    state: State<'_, AppState>,
    is_input: bool,
    level: f32,
) -> Result<(), String> {
    if let Ok(mut ts) = state.last_local_audio_action_ms.lock() {
        *ts = now_ms();
    }
    set_endpoint_volume(is_input, level)?;
    state.system_notify.notify_one();
    let pct = (level.clamp(0.0, 1.0) * 100.0).round() as u32;
    push_transient_hud(
        &app,
        &state,
        if is_input { "Microphone" } else { "Volume" },
        format!("{pct}%"),
        Some(level.clamp(0.0, 1.0) as f64),
    );
    Ok(())
}

#[tauri::command]
pub fn set_mute(state: State<'_, AppState>, is_input: bool, muted: bool) -> Result<(), String> {
    if let Ok(mut ts) = state.last_local_audio_action_ms.lock() {
        *ts = now_ms();
    }
    set_endpoint_mute(is_input, muted)?;
    state.system_notify.notify_one();
    Ok(())
}

#[tauri::command]
pub fn switch_audio_device(state: State<'_, AppState>, device_id: String) -> Result<(), String> {
    if device_id.trim().is_empty() {
        return Err("Invalid audio device id".into());
    }
    if let Ok(mut ts) = state.last_local_audio_action_ms.lock() {
        *ts = now_ms();
    }
    set_default_audio_device(&device_id)?;
    state.system_notify.notify_one();
    Ok(())
}

#[tauri::command]
pub fn set_display_brightness(
    app: AppHandle,
    state: State<'_, AppState>,
    percent: u32,
) -> Result<(), String> {
    set_brightness_percent(percent)?;
    push_transient_hud(
        &app,
        &state,
        "Brightness",
        format!("{}%", percent.min(100)),
        Some((percent.min(100) as f64) / 100.0),
    );
    Ok(())
}

fn push_transient_hud(
    app: &AppHandle,
    state: &State<'_, AppState>,
    title: &str,
    subtitle: String,
    progress: Option<f64>,
) {
    if let Ok(mut act) = state.activities.lock() {
        let now = now_ms();
        act.upsert(LiveActivity {
            id: format!("hud-{}", title.to_ascii_lowercase()),
            kind: ActivityKind::SystemHud,
            title: title.to_string(),
            subtitle,
            accent: None,
            badge: Some("LIVE".into()),
            progress,
            priority: 88,
            updated_at_ms: now,
            expires_at_ms: Some(now + 2400),
        });
        let _ = app.emit("pillvyn://activities", act.snapshot());
    }
}

pub(crate) fn dispatch_notification_and_ding(
    app: &AppHandle,
    state: &AppState,
    req: NotificationRequest<'_>,
) -> Result<AppNotification, String> {
    let clean_title = req.title.trim().to_string();
    if clean_title.is_empty() {
        return Err("Notification title cannot be empty".into());
    }

    let (dnd, ding_enabled, ding_sound) = state
        .config
        .lock()
        .map(|c| {
            (
                c.privacy.dnd_mode || c.privacy.focus_mode,
                c.appearance.sms_ding_enabled,
                c.appearance.sms_ding_sound,
            )
        })
        .unwrap_or((false, true, true));

    let prio = req.priority.clone();
    let explicit_sms = matches!(req.kind_hint, Some(h) if h.eq_ignore_ascii_case("sms"));
    let clean_req = NotificationRequest {
        app_name: req.app_name.trim().chars().take(40).collect(),
        title: clean_title.chars().take(80).collect(),
        body: req.body.trim().chars().take(240).collect(),
        priority: req.priority,
        source: req.source,
        kind_hint: req.kind_hint,
    };

    let item = {
        let mut hub = state
            .notifications
            .lock()
            .map_err(|_| "Notification hub lock error")?;
        let created = hub.push_with_kind(clean_req, dnd);
        let _ = app.emit("pillvyn://notifications", hub.snapshot());
        created
    };

    if !item.suppressed_by_dnd {
        if let Ok(mut act) = state.activities.lock() {
            let now = now_ms();
            let is_sms = item.is_sms;
            act.upsert(LiveActivity {
                id: format!("notif-{}", item.id),
                kind: if is_sms {
                    ActivityKind::Sms
                } else {
                    ActivityKind::Notification
                },
                title: item.title.clone(),
                subtitle: if item.body.is_empty() {
                    item.app_name.clone()
                } else {
                    format!("{} · {}", item.app_name, item.body)
                },
                accent: None,
                badge: Some(item.app_name.clone()),
                progress: None,
                priority: if is_sms { 99 } else { 95 },
                updated_at_ms: now,
                expires_at_ms: Some(now + if is_sms { 5600 } else { 4500 }),
            });
            let _ = app.emit("pillvyn://activities", act.snapshot());
        }

        if ding_enabled && (item.is_sms || explicit_sms || prio == NotificationPriority::Critical) {
            let should_chime = ding_sound && (item.is_sms || explicit_sms);
            let _ = app.emit(
                "pillvyn://notification-ding",
                serde_json::json!({
                    "id": item.id,
                    "app_name": item.app_name,
                    "title": item.title,
                    "body": item.body,
                    "is_sms": item.is_sms,
                    "play_sound": should_chime,
                    "timestamp_ms": item.timestamp_ms,
                }),
            );
            if should_chime {
                play_sms_ding();
            }
        }
    }

    Ok(item)
}

#[tauri::command]
pub fn push_notification(
    app: AppHandle,
    state: State<'_, AppState>,
    app_name: String,
    title: String,
    body: String,
    priority: Option<String>,
    kind: Option<String>,
) -> Result<(), String> {
    let prio = match priority.as_deref() {
        Some("critical") => NotificationPriority::Critical,
        Some("high") => NotificationPriority::High,
        Some("low") => NotificationPriority::Low,
        _ => NotificationPriority::Normal,
    };
    dispatch_notification_and_ding(
        &app,
        &state,
        NotificationRequest {
            app_name,
            title,
            body,
            priority: prio,
            source: "app".to_string(),
            kind_hint: kind.as_deref(),
        },
    )?;
    Ok(())
}

#[tauri::command]
pub fn trigger_test_sms_ding(app: AppHandle, state: State<'_, AppState>) -> Result<(), String> {
    dispatch_notification_and_ding(
        &app,
        &state,
        NotificationRequest {
            app_name: "Messages".to_string(),
            title: "New SMS · Alex".to_string(),
            body: "Hey! PILLVYN v2.0.0 is crystal smooth.".to_string(),
            priority: NotificationPriority::High,
            source: "test".to_string(),
            kind_hint: Some("sms"),
        },
    )?;
    Ok(())
}

#[tauri::command]
pub fn notification_action(
    app: AppHandle,
    state: State<'_, AppState>,
    action: String,
    id: Option<String>,
) -> Result<NotificationsSnapshot, String> {
    let mut hub = state
        .notifications
        .lock()
        .map_err(|_| "Notification hub lock error")?;
    match action.as_str() {
        "mark_read" => {
            if let Some(target) = id.as_deref() {
                hub.mark_read(target);
            }
        }
        "mark_all_read" => hub.mark_all_read(),
        "dismiss" => {
            if let Some(target) = id.as_deref() {
                hub.dismiss(target);
            }
        }
        "clear_all" => hub.clear_all(),
        "refresh_listener" => hub.refresh_windows_listener(),
        _ => return Err("Unsupported notification action".into()),
    }
    let snap = hub.snapshot();
    let _ = app.emit("pillvyn://notifications", snap.clone());
    Ok(snap)
}

#[tauri::command]
pub fn clipboard_action(
    app: AppHandle,
    state: State<'_, AppState>,
    action: String,
    id: Option<String>,
) -> Result<ClipboardSnapshot, String> {
    let mut store = state.clipboard.lock().map_err(|_| "Clipboard lock error")?;
    match action.as_str() {
        "restore" => {
            let target = id.ok_or("Clipboard entry id required")?;
            store.restore_item(&target)?;
        }
        "pin" => {
            let target = id.ok_or("Clipboard entry id required")?;
            store.toggle_pin(&target);
        }
        "delete" => {
            let target = id.ok_or("Clipboard entry id required")?;
            store.delete_item(&target);
        }
        "clear" => store.clear(),
        "toggle_enabled" => {
            store.enabled = !store.enabled;
            if let Ok(mut cfg) = state.config.lock() {
                cfg.privacy.clipboard_enabled = store.enabled;
                let _ = save_config(&cfg);
                let _ = app.emit("pillvyn://config", cfg.clone());
            }
        }
        _ => return Err("Unsupported clipboard action".into()),
    }
    let snap = store.snapshot();
    let _ = app.emit("pillvyn://clipboard", snap.clone());
    Ok(snap)
}

#[tauri::command]
pub fn run_quick_action(item: QuickActionItem) -> Result<(), String> {
    execute_quick_action(&item)
}

#[tauri::command]
pub fn open_url(url: String) -> Result<(), String> {
    open_external_url(&url)
}

#[tauri::command]
pub fn timer_action(
    app: AppHandle,
    state: State<'_, AppState>,
    action: String,
    id: Option<String>,
    label: Option<String>,
    seconds: Option<u64>,
) -> Result<(), String> {
    let (timers, sw_ms, sw_run) = {
        let mut w = state.widgets.lock().map_err(|_| "Widget lock error")?;
        match action.as_str() {
            "add" => {
                w.add_timer(
                    label.unwrap_or_else(|| "Timer".into()),
                    seconds.unwrap_or(300),
                );
            }
            "toggle" => {
                if let Some(tid) = id.as_deref() {
                    w.toggle_timer(tid);
                }
            }
            "remove" => {
                if let Some(tid) = id.as_deref() {
                    w.remove_timer(tid);
                }
            }
            "stopwatch_start" => w.control_stopwatch("start"),
            "stopwatch_pause" => w.control_stopwatch("pause"),
            "stopwatch_reset" => w.control_stopwatch("reset"),
            _ => return Err("Unsupported timer action".into()),
        }
        (
            w.timers.clone(),
            w.stopwatch_elapsed_ms,
            w.stopwatch_running,
        )
    };

    sync_timer_activity(&app, &state, &timers);
    let _ = app.emit(
        "pillvyn://widgets-timers",
        serde_json::json!({
            "timers": timers,
            "stopwatch_ms": sw_ms,
            "stopwatch_running": sw_run,
        }),
    );
    Ok(())
}

pub fn sync_timer_activity(app: &AppHandle, state: &AppState, timers: &[TimerEntry]) {
    if let Ok(mut act) = state.activities.lock() {
        if let Some(running) = timers.iter().find(|t| t.running && !t.completed) {
            let mins = running.remaining_seconds / 60;
            let secs = running.remaining_seconds % 60;
            let prog = if running.total_seconds > 0 {
                1.0 - (running.remaining_seconds as f64 / running.total_seconds as f64)
            } else {
                0.0
            };
            act.upsert(LiveActivity {
                id: "timer-primary".into(),
                kind: ActivityKind::Timer,
                title: running.label.clone(),
                subtitle: format!("{mins:02}:{secs:02} remaining"),
                accent: None,
                badge: Some(format!("{mins:02}:{secs:02}")),
                progress: Some(prog.clamp(0.0, 1.0)),
                priority: 82,
                updated_at_ms: now_ms(),
                expires_at_ms: None,
            });
        } else {
            act.remove("timer-primary");
        }
        let _ = app.emit("pillvyn://activities", act.snapshot());
    }
}

#[tauri::command]
pub async fn refresh_weather(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<WeatherSnapshot, String> {
    let cfg = state
        .config
        .lock()
        .map(|g| g.widgets.clone())
        .unwrap_or_default();
    let snap = fetch_live_weather(&cfg).await?;
    if let Ok(mut w) = state.widgets.lock() {
        w.weather = snap.clone();
    }
    let _ = app.emit("pillvyn://weather", snap.clone());
    Ok(snap)
}

#[tauri::command]
pub fn plugin_action(
    app: AppHandle,
    state: State<'_, AppState>,
    action: String,
    plugin_id: Option<String>,
    target: Option<String>,
) -> Result<PluginsSnapshot, String> {
    let snap = match action.as_str() {
        "refresh" => discover_plugins(),
        "create_sample" => {
            create_example_plugin()?;
            discover_plugins()
        }
        "toggle" => {
            let id = plugin_id.ok_or("Plugin id required")?;
            let current = discover_plugins();
            let cur_enabled = current
                .plugins
                .iter()
                .find(|p| p.manifest.id == id)
                .map(|p| p.manifest.enabled)
                .unwrap_or(true);
            toggle_plugin_enabled(&id, !cur_enabled)?
        }
        "trigger_activity" => {
            let title = plugin_id.unwrap_or_else(|| "Plugin".into());
            let detail = target.unwrap_or_else(|| "Active plugin event".into());
            if let Ok(mut act) = state.activities.lock() {
                let now = now_ms();
                act.upsert(LiveActivity {
                    id: format!("plugin-{title}"),
                    kind: ActivityKind::Plugin,
                    title,
                    subtitle: detail,
                    accent: None,
                    badge: Some("PLUGIN".into()),
                    progress: None,
                    priority: 76,
                    updated_at_ms: now,
                    expires_at_ms: Some(now + 12_000),
                });
                let _ = app.emit("pillvyn://activities", act.snapshot());
            }
            discover_plugins()
        }
        _ => return Err("Unsupported plugin action".into()),
    };
    let _ = app.emit("pillvyn://plugins", snap.clone());
    Ok(snap)
}

#[tauri::command]
pub async fn ask_ai_assistant(
    state: State<'_, AppState>,
    prompt: String,
    api_key: Option<String>,
) -> Result<AiPromptResponse, String> {
    let cfg = state
        .config
        .lock()
        .map(|g| g.ai.clone())
        .unwrap_or_default();
    run_ai_prompt(&cfg, &prompt, api_key).await
}

#[tauri::command]
pub async fn check_updates(
    app: AppHandle,
    state: State<'_, AppState>,
    consent_override: Option<bool>,
    repo_override: Option<String>,
) -> Result<UpdateCheckResult, String> {
    let (saved_consent, saved_repo) = state
        .config
        .lock()
        .map(|g| {
            (
                g.privacy.update_check_consent,
                g.privacy.update_repo.clone(),
            )
        })
        .unwrap_or((false, "EbramJames/PILLVYN".into()));
    let consent = consent_override.unwrap_or(saved_consent);
    let repo = repo_override
        .map(|r| r.trim().to_string())
        .filter(|r| !r.is_empty())
        .unwrap_or(saved_repo);
    let ver = app.package_info().version.to_string();
    check_for_updates(&ver, &repo, consent).await
}

#[tauri::command]
pub fn set_click_through_override(state: State<'_, AppState>, enabled: bool) {
    if let Ok(mut g) = state.window_state.lock() {
        g.force_click_through = enabled;
    }
}
