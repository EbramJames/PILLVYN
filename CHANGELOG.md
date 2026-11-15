# Changelog

All notable changes to **PILLVYN** will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

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
