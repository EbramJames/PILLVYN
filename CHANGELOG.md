# Changelog

All notable changes to **PILLVYN** will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [1.4.0] - 2026-11-16
### Added
- **Full Dynamic Island Repositioning (`Display & Behavior` + Desktop Move)**:
  - Added an **Interactive Screen Positioner** canvas in Settings Studio (`settings.js`, `settings.css`) proportional to the selected monitor's logical workspace (`X` and `Y` drag + click-to-place with gentle center snap and live coordinate badge).
  - Added one-click quick position presets (**Top Left**, **Top Center**, **Top Right**, **Center**, **Bottom Center**) and expanded horizontal & vertical sliders across the full active monitor bounds (`compute_island_window_position` in `island/mod.rs` and `PillvynConfig::sanitize` in `config.rs`).
  - Added **Move on Desktop** (`set_reposition_mode` and `move_island_position` Tauri commands + `Escape` exit + screen-coordinate desktop capsule dragging in `main.js`) with bidirectional live synchronization back into Settings Studio (`pillvyn://position-updated`, `pillvyn://reposition-mode`).
  - Automatically lifts bottom-docked islands cleanly inside the active monitor's logical bounds when expanded (`update_island_hitbox` + `compute_island_window_position`).

### Fixed
- **Settings Studio Unclosable Window Bug**:
  - Removed the Tauri v2 JS `onCloseRequested` / `beforeunload` interception in `settings.js` that routed window close through `this.destroy()`, granted `core:window:allow-destroy` in `src-tauri/capabilities/settings.json`, and moved unsaved preview/reposition cleanup into a native non-blocking `win.on_window_event` hook (`restore_saved_appearance`) inside `open_settings_window()` so Settings Studio closes immediately via the X button, `Alt+F4`, or `WM_CLOSE`.
- **Line-by-Line Coordination & Hardening Fixes**:
  - Serialized config file writes with `CONFIG_IO_LOCK` and PID-scoped atomic temp files (`config.{pid}.tmp`) in `src-tauri/src/config.rs`.
  - Switched Steam installation discovery in `src-tauri/src/services/gaming.rs` to native Win32 registry lookup (`RegGetValueW` on `Software\Valve\Steam\SteamPath`) with no environment-override fallback gaps.
  - Refreshed live weather asynchronously when city/weather settings change in `save_app_config`, cleared stale `"timer-primary"` Live Activities when turning off Widgets, guarded Overview Quick Actions tiles when `modules.actions` is disabled, fixed boolean `false` attribute handling in `el()`, and added `.settings-topbar` flex-wrap support at narrow widths.

## [1.3.0] - 2026-11-16
### Fixed
- **Taskbar Location Indicator & Background Subprocess Elimination**: Replaced `netsh wlan show interfaces` (`Network Command Shell`) polling with pure kernel NDIS `GetIfTable2` (`IF_TYPE_IEEE80211`) link detection in `system_ctrl.rs`, stopping the Windows 11 Location arrow icon from flashing in and out of the taskbar system tray. Also replaced background `powershell.exe` brightness queries/setters with native in-process COM WMI (`ROOT\WMI` `IWbemLocator`) and DDC/CI.
- **ToolWindow Styles & Tray Window Resize Clamping**: Forced `WS_EX_TOOLWINDOW` and cleared `WS_EX_APPWINDOW` on both `island` and `tray` windows, and replaced background hidden `setSize` calls with `resize_tray_window` clamped strictly to visible updates inside monitor `work_area`.
- **Every Drag Bar & Slider Across Island & Settings**:
  - Added shared `createRangeSlider` (`utils.js`) and enlarged clickable range hit targets (`18px` interactive height, `5px`/`6px` visual track) across Hub, Media Seek, System Controls, and Settings Studio.
  - Prevented background telemetry (`timers`, `notifications`, `clipboard`, `weather`, `plugins`, `media`, `system`, `activities`) and capsule swipe gestures from interrupting or rebuilding active slider drags, and kept live `%` / track-time readouts updated continuously during drags.
  - Synchronized Settings Studio slider factory defaults (`opacity = 0.88`, `hardware_interval_ms = 2000`, `clipboard_max_items = 25`) with Rust defaults, fixed `.slider-bound` wrapping, and accurately rendered capsule scale in the live preview stage.
- **Buttons, Focus & Sync Accuracy**:
  - Fixed `open_external_url()` (`actions.rs`) so the **Wi-Fi Flyout** (`ms-availablenetworks:`) button works alongside `ms-settings:` and HTTPS links.
  - Fixed Clipboard search input losing focus after each typed character (`clipboard.js`), preserved AI draft prompts during re-renders (`plugins.js`), fixed disabled styling/states on media transport & hover-peek buttons, stopwatch `Reset`, and notification `Read All`.
  - Fixed Settings Studio **Check Now** failing when **Allow Update Checks** was toggled before saving, added initial `island_expanded` state & dynamic `Hide Island`/`Show Island` and `Expand`/`Collapse` sidebar labels, fixed synchronous `toggle_expand` tray/hotkey state updates (`pillvyn://set-expanded`), and surfaced actionable error banners instead of swallowing IPC failures.

## [1.2.0] - 2026-11-16
### Added
- **Custom Obsidian Glass Tray Menu (`tray.html`)**: Replaced the native Windows tray right-click menu with a bespoke frameless Obsidian Glass popover featuring live Island status, optional Now Playing strip, instant toggles for Visibility, Expand/Collapse, Do Not Disturb, Focus Mode, Settings Studio, and Quit — automatically clamped inside the active monitor work area for both taskbar and tray-overflow clicks.
- **Redesigned Settings Studio**: Reorganized Settings Studio into 5 intuitive categories (`Appearance`, `Display & Behavior`, `Modules & Tools`, `Shortcuts & Apps`, `Privacy & System`) with a live capsule preview stage, curated accent color dots + live hex readout, real-time `preview_appearance` slider synchronization, interactive global hotkey recorder, and full CRUD management for custom Quick Actions and per-app Profiles.
- **Unified Validation & Sync IPC**: Added `save_settings_config` (surfacing duplicate hotkey errors and OS registration/autostart warnings cleanly), `validate_quick_action_cmd`, `preview_appearance`, `cancel_appearance_preview`, `get_tray_state`, `tray_action`, and `pillvyn://island-state` real-time visibility/expansion synchronization.

### Fixed
- **Accent Color Across All Themes**: Fixed `[data-theme]` rules in `tokens.css` overriding user-selected `--accent`, `--accent-soft`, and `--accent-glow` variables on `aurora`, `midnight`, `crimson`, and `frosted` themes; extracted shared theme/slider styling utilities into `src/js/theme.js`.
- **Settings Sliders & Missing Controls**: Fixed range slider filled-track progress bars, live numeric value readouts, single-click per-slider `Default` reset buttons, unified the AI module & engine enable switches (`modules.ai` / `ai.enabled`), and exposed `x_offset`, `auto_collapse_ms`, and `clock_24h` controls in Settings Studio.
- **Window Visibility & Disabled-Module Coordination**: Fixed `toggle_expand`, `clipboard_drawer`, and single-instance wakeups updating `IslandWindowState::is_visible` and geometry properly; cleared stale `force_click_through` flags and Live Activities when Gaming, Downloads, or Music modules are turned off; prevented clicking disabled telemetry cards in Overview.

## [1.1.0] - 2026-11-16
### Added
- **Unified Brand Identity**: Applied the sculpted obsidian & platinum **PILLVYN** emblem (`pillvyn-logo.png`) across app icons, titlebars, tray PNGs, Settings Studio, NSIS wizard bitmaps, README, and website assets.
- **Intuitive Dynamic Island Gestures & Dismissal**: Added pointer swipe gestures across Minimized, Compact, and Expanded states, hover-to-peek inline playback buttons on Compact, scroll-wheel master volume control (`Shift+Wheel` for brightness where supported), keyboard navigation (`Escape`, `ArrowLeft`/`ArrowRight`, `Space`), click-outside dismissal, and a header **Pin** button (`set_island_pinned`).
- **Multi-Session Media Switcher**: Expanded GSMTC integration with event-driven WinRT session/timeline listeners (`spawn_media_event_listener`), multi-app source selector pills, and expanded player recognition (Spotify, Chrome, Edge, Firefox, Opera, Yandex Browser, VLC, Apple Music, Amazon Music, MusicBee, Winamp, mpv, Kodi).
- **Event-Driven Windows Sync**: Added Win32 `SetWinEventHook` foreground window hook + `SHQueryUserNotificationState` fullscreen/quiet-hours detection, native Win32 `RegGetValueW` Steam AppID lookup (zero child processes), and WASAPI COM callbacks (`IAudioEndpointVolumeCallback` + `IMMNotificationClient`) for instantaneous audio/mute/device HUD sync.
- **Settings Studio v2 Config**: Added toggles for `minimal_idle`, `hover_peek`, `click_outside_dismiss`, and `show_volume_hud`, plus automatic migration from `pillvyn/pillvyn` to `EbramJames/PILLVYN`.

### Changed
- **Quiet Obsidian Glass UI Redesign**: Streamlined the Expanded hub from 13 top-level tabs into a calm, monochrome 7-tab dock (`Hub`, `Media`, `Controls`, `Monitor`, `Alerts`, `Clip`, `Tools`) with clean segmented sub-navigation, hairline progress indicators, uniform stroke-1.65 monochrome SVG icons, and reduced idle capsule clutter.
- **System Telemetry Caching**: Added TTL caching for WMI/DDC-CI brightness and Win32 Wi-Fi radio queries so unsupported monitors never stutter or spawn redundant work.

## [1.0.0] - 2026-11-15
### Added
- **Dynamic Island Core**: Top-center floating obsidian glass capsule with Minimized, Compact, and Expanded states, spring transitions, smart cursor hit-testing click-through around the capsule, multi-monitor selection, and per-monitor DPI scaling.
- **Live Activities Coordinator**: Simultaneous priority-ordered Live Activities strip (Media, Downloads, Timers, Gaming, Notifications, System HUD, Plugins).
- **Music Player**: Windows GSMTC session integration with album artwork extraction, track/artist/source detection (Spotify, Chrome, Edge, Firefox, VLC, Apple Music), seek bar, and transport controls.
- **System Controls**: WASAPI CoreAudio master & microphone volume/mute, audio output/input device switching (`IPolicyConfig`), display brightness via WMI/DDC-CI where supported, battery status, and Windows Wi-Fi/Bluetooth shortcuts.
- **Hardware Monitor**: Real-time CPU, RAM, Disk storage, DXGI + PDH GPU usage, and NVIDIA NVML temperature/VRAM/power telemetry where supported.
- **Gaming Mode**: Steam `RunningAppID` and foreground process/fullscreen detection, automatic notification suppression, optional click-through HUD, and ecosystem status for Steam, Discord, OBS, Spotify, VLC, and browsers.
- **Download Monitor**: Real-time tracking of active browser downloads (`.crdownload`, `.part`, `.opdownload`) and Steam `.acf` manifest progress, speed, and ETA.
- **Clipboard Manager**: Event-driven Win32 `AddClipboardFormatListener` capture for text and images with search, pin, restore, sensitive token filtering, and instant clear.
- **Network Monitor**: Real adapter Rx/Tx throughput (`GetIfTable2`), ICMP ping latency, and VPN tunnel detection.
- **Quick Actions, Widgets, Plugins, & Optional AI**: Validated tool/URI shortcuts, timers/stopwatch, Open-Meteo weather with offline cache, local `plugin.json` manifest loader, optional Ollama / OpenAI-compatible AI tab, Settings Studio, system tray, global hotkeys, Windows autostart, and branded NSIS x64 setup installer.
