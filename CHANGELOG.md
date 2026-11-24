# Changelog

All notable changes to **PILLVYN** are documented here.

## [3.4.0] - 2026-11-24

### Added
- **NVIDIA Nemotron 3 Ultra (Free) Brain & Ultra-Fast Inference:** Upgraded the default OpenRouter model to `nvidia/nemotron-3-ultra-550b-a55b:free` (with automatic `nvidia/nemotron-3.5-lightning:free` failover on upstream rate limits), adding a configurable **Response Speed** mode (`Fast` with `reasoning.effort = "none"` for ~1.4s live responses, or `Deep` with `reasoning.effort = "low"`) and automatic migration from v3.3.0 settings.
- **Full Windows System Agent (26 Structured Tools, `src-tauri/src/ai_tools.rs`):** Expanded PILLVYN AI into a complete Windows automation agent (toggleable via **Settings → AI → Full Windows system access**, default on):
  - **System & Shell:** `run_powershell` (UTF-8, 25s timeout, hidden window), `get_system_info` (RAM, CPU, battery, volume, active window), `set_brightness` (WMI display brightness), `power_control` (`lock`, `sleep`, `display_off`, `empty_recycle_bin`, `restart`, `shutdown`), and live `web_search`.
  - **Apps, Windows & Processes:** `launch_app` (smart aliases + Start Menu shortcut search + `ShellExecuteW`), `manage_window` (`list`, `focus`, `minimize`, `maximize`, `restore`, `close`), `list_processes`, `kill_process`, and `keyboard_action` (Unicode Persian/English typing & keyboard shortcuts).
  - **Filesystem:** `file_list`, `file_read`, `file_write`, and `file_search` with friendly Desktop/Documents/Downloads path resolution.
  - **Live Tool Telemetry:** Streams real-time tool execution updates (`Running run_powershell…`) directly on the Dynamic Island pill and chat stream, plus response latency badges.
- **Upgraded Bilingual Persian & English Voice Engine:**
  - **Whisper `small-q5` Default + Fast Greedy Decoding:** Upgraded the default local STT model to `ggml-small-q5_1.bin` (`244M` params, `190 MB`) with `--beam-size 1 --best-of 1 --no-fallback`, background `whisper-server.exe` pre-warming on startup, and selectable **Speech Language** (`Auto` / `فارسی` / `English`).
  - **Sentence-by-Sentence Streaming Edge TTS & Multilingual Neural Voices:** Switched default English voices to Microsoft `en-US-*MultilingualNeural` (`Ava`, `Andrew`, `Emma`, `Brian`) alongside Persian `fa-IR-DilaraNeural` / `fa-IR-FaridNeural`, added `clean_for_tts` (strips markdown, code blocks, URLs, and emojis before synthesis), and implemented concurrent sentence-by-sentence prefetching with automatic per-sentence Persian/English voice switching.
  - **Live Mic & Silero VAD Meter + Natural Follow-Up Listening:** Added real-time Mic (AGC + RNNoise) and Silero VAD speech meters in **Settings → AI**, plus a 7-second conversational follow-up listening window after spoken replies.

### Fixed
- **Wake Word (`"Hello PILLVYN"` / `«سلام پیلوین»`) Reliability:** Enabled hands-free wake-word standby by default (`aiWakeWord: true`), added software microphone Auto-Gain Control (AGC, up to `3.5×` with noise-floor guard) for quiet USB microphones, tuned Silero VAD onset (`0.40`), continuation (`0.24`), 512 ms pre-roll, and snappy endpointing (`448 ms` standby / `704 ms` conversation), and rebuilt `match_wake_word` with fuzzy phonetic + Levenshtein matching for split/misheard English & Persian variants and direct `"PILLVYN …"` / `"پیلوین …"` commands.

## [3.3.0] - 2026-11-24

### Added
- **PILLVYN Voice AI Assistant Pipeline (End-to-End Bilingual Persian & English):**
  - **Wake Word Detection (`"Hello PILLVYN"` / `«سلام پیلوین»`):** Hands-free wake-word activation with fuzzy phonetic matching and one-breath inline command extraction (e.g., `"Hello PILLVYN, set a 5 minute timer"`), alongside a dedicated global hotkey (`Ctrl+Alt+A`), toolbar button, and System Tray shortcut.
  - **Real-Time Neural Noise Removal (RNNoise):** Native Windows WASAPI 48 kHz microphone capture (`src-tauri/src/ai_mic.rs`) denoised frame-by-frame with official Xiph.org **RNNoise** WebAssembly (`480`-sample 10 ms frames) to strip fan noise, background hum, and microphone hiss.
  - **Neural Voice Activity Detection (Silero VAD ONNX):** Runs official **Silero VAD v5** (`silero_vad.onnx`) via single-threaded SIMD ONNX Runtime Web with stateful LSTM inference, 480 ms pre-roll buffering, and smart silence-based sentence endpointing so incomplete pauses mid-sentence are never sent prematurely.
  - **Local Speech-to-Text (`whisper.cpp`):** Managed local `whisper-server.exe` (`--language auto`, `--suppress-nst`, bilingual initial prompt) with hot-in-RAM multilingual `ggml-base.bin` / `ggml-small-q5_1.bin` models for fast automatic Persian and English recognition.
  - **Smart Brain (`OpenRouter` · `inclusionai/ling-3.2-flash:free`):** Native Windows WinHTTP OpenAI-compatible Chat Completions engine (`src-tauri/src/net_http.rs`, `src-tauri/src/ai.rs`) with DPAPI-encrypted API key storage, multi-turn memory, automatic `429`/`503` retry backoff, valid JSON output, and a multi-step **Function Calling / Tool Calling** loop controlling 12 live PILLVYN/Windows actions (timer, media, volume, mute, notes & reminders, clipboard read/write, theme switching, safe URL opening, screenshot, OCR, and Do Not Disturb).
  - **Neural Text-to-Speech (Microsoft Edge TTS):** Native WinHTTP WebSocket synthesis (`Sec-MS-GEC` SHA-256 protocol) generating MP3 speech in Persian (`fa-IR-DilaraNeural`, `fa-IR-FaridNeural`) and English (`en-US-AvaNeural`, `en-US-AndrewNeural`, `en-US-EmmaNeural`, `en-US-BrianNeural`) with auto-language voice selection, speed (`-50%` to `+100%`), volume (`0%` to `100%`), and echo-guarded playback.
- **Apple-Intelligence-Inspired AI Pill & Settings → AI Studio:** Added an animated energy-reactive AI orb view (`[data-view="ai"]`), inline bilingual conversation panel with tool-call badges, quick prompts, replay/copy controls, one-click local prerequisites downloader with live progress bar, and a 5th **AI** tab inside Settings Studio.

## [3.2.0] - 2026-11-23

### Added
- **Frosted Pearl Daylight Glass Theme (`frost`):** Bright translucent pearlescent glass-morphism with drifting pastel sky/lavender/rose caustics, cursor-reactive prism gleam, adaptive dark ink (`body.daylight`), and matching bright frosted dropdown stacks, tabs, switches, and tooltips.
- **Amber Horizon Theme (`solar`) & Rebuilt Sapphire Prism (`crystal`):** Added a warm obsidian-to-molten-gold solar dusk theme and upgraded Sapphire (`crystal`) into silky optical cobalt-indigo prism refraction waves, paired with cohesive atmospheric stack surfaces across **Aurora**, **Nebula**, **Sapphire**, and **Amber Horizon**.
- **Floating Micro-Tooltips (`#tip`):** Hovering toolbar icons displays sleek floating micro-tooltips with keyboard shortcut pills (such as `Ctrl+Alt+T` and `Ctrl+Alt+N`), hiding cleanly when stacks open or views change without affecting click-through hit-testing.
- **Direct Pill Interactions & Keyboard Escape:** Clicking the resting pill opens the toolbar immediately with a subtle sheen and wink; right-clicking anywhere on the island opens **Settings Studio** directly; pressing `Escape` dismisses open dropdown panels.
- **Draggable Volume Scrubber & Mute Toggle:** Upgraded the volume HUD with a dedicated mute button (`#v-mute`) and an interactive click/drag slider bar (`#v-bar` + `#v-knob`) alongside smooth scroll-wheel volume stepping.
- **Focus Timer Quick Presets, Wheel Adjust & Countdown Hairline:** Added one-click `5m`, `15m`, and `25m` preset chips, scroll-wheel minute adjustment directly on the timer readout, and a warm amber bottom countdown progress bar while running.
- **Album-Art Dynamic Glow Accent:** Samples loaded album covers on an offscreen probe to softly tint `#glow` and the track scrubber in **Liquid Glass** and **Obsidian** modes while keeping text crisp and readable.
- **Optional Micro-Acoustics (`sounds`):** Added subtle Web Audio chimes for incoming notifications, shelf drops, control interactions, and timer/reminder completion, toggleable in **Settings → Behavior**.
- **Live Top Screen Offset (`topOffset`):** Added a DPI-scaled `0–24px` vertical offset stepper (`step 2px`) in **Settings → Behavior** that repositions the island immediately without restarting.
- **Expanded System Tray Menu with Live Sync:** Added quick actions for **Open Settings…**, **What PILLVYN can do**, **Quick Note (`Ctrl+Alt+N`)**, and **Text Grab (`Ctrl+Alt+T`)**, and synchronized the tray's **Start with Windows** checkmark whenever toggled inside Settings Studio.

### Fixed
- **Daylight Class Coordination (`ui/app.js`, `ui/style.css`):** Centralized `syncDaylight()` and `syncSurfaceClasses()` so switching between `frost`, `cloud`, and `ice` never leaves stale light/dark styling or drops icon contrast on active buttons and toggles.
- **Zero-Bound Settings Stepper Handling (`ui/app.js`):** Replaced truthy fallback with `Number.isFinite` so `0px` values in `topOffset` render and step accurately with properly disabled boundary buttons.

## [3.1.0] - 2026-11-22

### Added
- **New Logo v2 Identity:** Updated master branding artwork (`docs/branding/pillvyn-logo.png`), multi-resolution Windows `.ico` (`16–256px`), system tray icon (`32x32`), UI emblem (`ui/icon.png`), and hero showcase (`docs/hero.jpg`), backed by a reproducible `scripts/generate-icons.ps1` pipeline.
- **Liquid Glass (Glass-Morphism) Theme (`glass`):** Signature translucent smoked-crystal default theme with frosted depth, optical specular top/bottom edge refraction, drifting pearlescent caustics, cursor-reactive rim glint, audio-reactive prismatic shimmer, and matching translucent frosted-glass dropdown stacks, settings tabs, and notification pills.
- **Aurora Theme (`aurora`):** Flowing Northern Lights emerald, cyan, and violet ribbon theme with starlight and audio-reactive intensity.
- **Categorized Settings Studio:** Rebuilt inline settings inside the island with four intuitive tabs (**Style**, **Behavior**, **Tools**, **About**), a visual 4×2 **Theme Gallery** with live pill previews, clean section headers, and a branded About card with quick tour, update check, and GitHub links.
- **Resting Logo Mode (`Logo`) & Shimmer Effects:** Added a third resting pill mode displaying the luminous PILLVYN v2 mark alongside live time (`Eyes`, `Clock`, `Logo`), plus toggleable glass shimmer sweeps (`shimmer`) on notifications, track changes, drops, and theme changes, and gradient audio visualizer bars.
- **Release Tooling & Project Hygiene:** Added `scripts/generate-icons.ps1`, `scripts/package.ps1`, `.editorconfig`, `CHANGELOG.md`, `CONTRIBUTING.md`, `SECURITY.md`, expanded CI checks (UI JS syntax + packaged installer checksum verification), and unit tests in `src-tauri/src/settings.rs`.

### Fixed
- **Independent Global Hotkey Registration (`src-tauri/src/hit.rs`):** Decoupled `Ctrl+Alt+N` (quick note) and `Ctrl+Alt+T` (text grab) so a conflict on one shortcut never disables the other.
- **Settings Controls Consistency (`ui/app.js`):** Fixed rapid toggle clicks reading stale closure state, synchronized `notifSeconds` stepper range (`2–15s`) with backend bounds (`settings.rs`) with disabled boundary states, and hardened theme lookup against unknown keys.
- **Visualizer Debug Array Handling (`ui/app.js`):** Fixed `sampleLevels()` handling array spectrum inputs in `fakeLevels` without producing `NaN`.

## [3.0.0] - 2026-11-20

- Rebuilt PILLVYN on a lean single-pill Rust + Tauri 2 + Vanilla JS Dynamic Island architecture (`~3.3 MB` per-user NSIS installer, `~45 MB` live working set).
- Added Now Playing with scrubber & visualizer, animated themes, one-line notifications, smart copy, file shelf, clipboard history with pinned images, quick notes & reminders (`Ctrl+Alt+N`), offline English/Persian OCR (`Ctrl+Alt+T`), DPAPI password vault, and SHA-256 GitHub Releases self-updater.

## [2.0.0] - 2026-11-16

- Spring motion overhaul, SMS arrival ding audio + shimmer effect, toggleable contextual emojis across island and tray.

## [1.0.0 - 1.5.0] - 2026-11-14 to 2026-11-16

- Initial Windows Dynamic Island releases, Settings Studio, Windows Startup registration, and performance/size optimizations.
