pub mod commands;
pub mod config;
pub mod island;
pub mod services;

use commands::{
    ask_ai_assistant, check_updates, clipboard_action, get_initial_state, get_monitors,
    media_control, notification_action, open_settings_window, open_url, plugin_action,
    push_notification, refresh_weather, run_quick_action, save_app_config,
    set_click_through_override, set_display_brightness, set_island_pinned, set_mute, set_volume,
    switch_audio_device, sync_timer_activity, timer_action, update_island_hitbox, AppState,
};
use config::{load_config, save_config, AppProfile, PillvynConfig};
use island::activities::{now_ms, ActivityCoordinator, ActivityKind, LiveActivity};
use island::{apply_window_geometry, spawn_click_through_controller, IslandWindowState};
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
use std::sync::{Arc, Mutex};
use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_autostart::MacosLauncher;
use tauri_plugin_global_shortcut::{GlobalShortcutExt, ShortcutState};
use tokio::sync::Notify;
use tracing::{info, warn};

pub fn register_configured_hotkeys(app: &AppHandle, cfg: &PillvynConfig) -> Result<(), String> {
    let gs = app.global_shortcut();
    let _ = gs.unregister_all();

    let pairs = [
        (cfg.hotkeys.toggle_expand.trim(), "toggle_expand"),
        (cfg.hotkeys.toggle_visibility.trim(), "toggle_visibility"),
        (cfg.hotkeys.toggle_dnd.trim(), "toggle_dnd"),
        (cfg.hotkeys.clipboard_drawer.trim(), "clipboard_drawer"),
    ];

    for (combo, action_name) in pairs {
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
        }
    }
    Ok(())
}

fn handle_hotkey_action(app: &AppHandle, action: &str) {
    match action {
        "toggle_visibility" => toggle_island_visibility(app),
        "toggle_expand" => {
            if let Some(w) = app.get_webview_window("island") {
                if !w.is_visible().unwrap_or(false) {
                    let _ = w.show();
                }
            }
            let _ = app.emit("pillvyn://hotkey", "toggle_expand");
        }
        "clipboard_drawer" => {
            if let Some(w) = app.get_webview_window("island") {
                if !w.is_visible().unwrap_or(false) {
                    let _ = w.show();
                }
            }
            let _ = app.emit("pillvyn://hotkey", "clipboard_drawer");
        }
        "toggle_dnd" => toggle_dnd_flag(app),
        _ => {}
    }
}

fn toggle_island_visibility(app: &AppHandle) {
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
    }
}

fn toggle_dnd_flag(app: &AppHandle) {
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

fn toggle_focus_flag(app: &AppHandle) {
    let state = app.state::<AppState>();
    let updated = {
        let Ok(mut cfg) = state.config.lock() else {
            return;
        };
        cfg.privacy.focus_mode = !cfg.privacy.focus_mode;
        let _ = save_config(&cfg);
        cfg.clone()
    };
    let _ = app.emit("pillvyn://config", updated);
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
        media_notify,
        system_notify,
        foreground_notify,
    };

    tauri::Builder::default()
        .manage(app_state)
        .plugin(tauri_plugin_single_instance::init(|app, _argv, _cwd| {
            if let Some(win) = app.get_webview_window("island") {
                let _ = win.show();
                let _ = app.emit("pillvyn://hotkey", "toggle_expand");
            }
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

            if let Some(island_win) = handle.get_webview_window("island") {
                if let Ok(cfg) = state.config.lock() {
                    let _ = apply_window_geometry(&island_win, &cfg);
                    let _ = register_configured_hotkeys(&handle, &cfg);
                }
            }

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

fn setup_tray(app: &AppHandle) -> Result<(), tauri::Error> {
    let toggle_item = MenuItem::with_id(
        app,
        "toggle_island",
        "Show / Hide Island",
        true,
        None::<&str>,
    )?;
    let expand_item = MenuItem::with_id(
        app,
        "toggle_expand",
        "Expand / Collapse",
        true,
        None::<&str>,
    )?;
    let dnd_item = MenuItem::with_id(
        app,
        "toggle_dnd",
        "Toggle Do Not Disturb",
        true,
        None::<&str>,
    )?;
    let focus_item =
        MenuItem::with_id(app, "toggle_focus", "Toggle Focus Mode", true, None::<&str>)?;
    let settings_item =
        MenuItem::with_id(app, "open_settings", "Settings Studio", true, None::<&str>)?;
    let quit_item = MenuItem::with_id(app, "quit", "Quit PILLVYN", true, None::<&str>)?;

    let menu = Menu::with_items(
        app,
        &[
            &toggle_item,
            &expand_item,
            &dnd_item,
            &focus_item,
            &settings_item,
            &quit_item,
        ],
    )?;

    let icon = app
        .default_window_icon()
        .cloned()
        .expect("default window icon must exist");

    TrayIconBuilder::with_id("pillvyn-tray")
        .icon(icon)
        .tooltip("PILLVYN — Dynamic Island")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id().as_ref() {
            "toggle_island" => toggle_island_visibility(app),
            "toggle_expand" => handle_hotkey_action(app, "toggle_expand"),
            "toggle_dnd" => toggle_dnd_flag(app),
            "toggle_focus" => toggle_focus_flag(app),
            "open_settings" => {
                let handle = app.clone();
                tauri::async_runtime::spawn(async move {
                    let _ = open_settings_window(handle).await;
                });
            }
            "quit" => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                handle_hotkey_action(tray.app_handle(), "toggle_expand");
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
