use crate::config::{save_config, PillvynConfig, QuickActionItem};
use crate::island::activities::{now_ms, ActivityCoordinator, ActivityKind, LiveActivity};
use crate::island::{
    apply_window_geometry, list_monitors, HitboxRect, IslandWindowState, MonitorInfo,
};
use crate::services::actions::{execute_quick_action, open_external_url};
use crate::services::ai::{run_ai_prompt, AiPromptResponse};
use crate::services::clipboard::{ClipboardSnapshot, ClipboardStore};
use crate::services::downloads::DownloadsSnapshot;
use crate::services::gaming::GamingSnapshot;
use crate::services::hardware::HardwareSnapshot;
use crate::services::media::{control_media, MediaSnapshot};
use crate::services::network::NetworkSnapshot;
use crate::services::notifications::{
    NotificationHub, NotificationPriority, NotificationsSnapshot,
};
use crate::services::plugins::{
    create_example_plugin, discover_plugins, toggle_plugin_enabled, PluginsSnapshot,
};
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
        app_version: app.package_info().version.to_string(),
    }
}

#[tauri::command]
pub fn get_monitors(app: AppHandle) -> Vec<MonitorInfo> {
    list_monitors(&app)
}

#[tauri::command]
pub fn update_island_hitbox(
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
    if let Ok(mut exp) = state.island_expanded.lock() {
        *exp = expanded;
    }
    if let Some(tab) = active_tab {
        if let Ok(mut t) = state.active_tab.lock() {
            *t = tab;
        }
    }
}

#[tauri::command]
pub fn save_app_config(
    app: AppHandle,
    state: State<'_, AppState>,
    mut new_config: PillvynConfig,
) -> Result<PillvynConfig, String> {
    new_config.sanitize();
    save_config(&new_config)?;

    if let Ok(mut guard) = state.config.lock() {
        *guard = new_config.clone();
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

    // Apply Windows autostart setting cleanly
    let autostart_mgr = app.autolaunch();
    if new_config.autostart {
        let _ = autostart_mgr.enable();
    } else {
        let _ = autostart_mgr.disable();
    }

    let _ = crate::register_configured_hotkeys(&app, &new_config);
    let _ = app.emit("pillvyn://config", new_config.clone());
    Ok(new_config)
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
pub fn media_control(action: String, seek_sec: Option<f64>) -> Result<(), String> {
    control_media(&action, seek_sec)
}

#[tauri::command]
pub fn set_volume(
    app: AppHandle,
    state: State<'_, AppState>,
    is_input: bool,
    level: f32,
) -> Result<(), String> {
    set_endpoint_volume(is_input, level)?;
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
pub fn set_mute(is_input: bool, muted: bool) -> Result<(), String> {
    set_endpoint_mute(is_input, muted)
}

#[tauri::command]
pub fn switch_audio_device(device_id: String) -> Result<(), String> {
    if device_id.trim().is_empty() {
        return Err("Invalid audio device id".into());
    }
    set_default_audio_device(&device_id)
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

#[tauri::command]
pub fn push_notification(
    app: AppHandle,
    state: State<'_, AppState>,
    app_name: String,
    title: String,
    body: String,
    priority: Option<String>,
) -> Result<(), String> {
    let clean_title = title.trim();
    if clean_title.is_empty() {
        return Err("Notification title cannot be empty".into());
    }
    let prio = match priority.as_deref() {
        Some("critical") => NotificationPriority::Critical,
        Some("high") => NotificationPriority::High,
        Some("low") => NotificationPriority::Low,
        _ => NotificationPriority::Normal,
    };

    let dnd = state
        .config
        .lock()
        .map(|c| c.privacy.dnd_mode || c.privacy.focus_mode)
        .unwrap_or(false);

    let item = {
        let mut hub = state
            .notifications
            .lock()
            .map_err(|_| "Notification hub lock error")?;
        let created = hub.push(
            app_name.trim().chars().take(40).collect(),
            clean_title.chars().take(80).collect(),
            body.trim().chars().take(240).collect(),
            prio,
            "app".to_string(),
            dnd,
        );
        let _ = app.emit("pillvyn://notifications", hub.snapshot());
        created
    };

    if !item.suppressed_by_dnd {
        if let Ok(mut act) = state.activities.lock() {
            let now = now_ms();
            act.upsert(LiveActivity {
                id: format!("notif-{}", item.id),
                kind: ActivityKind::Notification,
                title: item.title.clone(),
                subtitle: if item.body.is_empty() {
                    item.app_name.clone()
                } else {
                    format!("{} · {}", item.app_name, item.body)
                },
                accent: None,
                badge: Some(item.app_name),
                progress: None,
                priority: 95,
                updated_at_ms: now,
                expires_at_ms: Some(now + 4500),
            });
            let _ = app.emit("pillvyn://activities", act.snapshot());
        }
    }
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
) -> Result<UpdateCheckResult, String> {
    let (consent, repo) = state
        .config
        .lock()
        .map(|g| {
            (
                g.privacy.update_check_consent,
                g.privacy.update_repo.clone(),
            )
        })
        .unwrap_or((false, "pillvyn/pillvyn".into()));
    let ver = app.package_info().version.to_string();
    check_for_updates(&ver, &repo, consent).await
}

#[tauri::command]
pub fn set_click_through_override(state: State<'_, AppState>, enabled: bool) {
    if let Ok(mut g) = state.window_state.lock() {
        g.force_click_through = enabled;
    }
}
