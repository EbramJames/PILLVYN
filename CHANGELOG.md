# Changelog

All notable changes to **PILLVYN** are documented here.

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
