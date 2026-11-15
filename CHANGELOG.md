# Changelog

All notable changes to **PILLVYN** will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

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
