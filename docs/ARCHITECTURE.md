# PILLVYN Architecture

PILLVYN is built on a native **Rust + Tauri 2** backend paired with a lightweight, dependency-free **HTML5 / CSS3 / Vanilla ES Module** frontend rendered in Microsoft Edge WebView2.

## Directory Layout

```text
PILLVYN/
├── .github/                  # CI/Release workflows & issue/PR templates
├── docs/                     # Branding sources, screenshots, GitHub Pages site & architecture notes
│   ├── branding/             # Master icon artwork (pillvyn-logo.png)
│   ├── screenshots/          # Verified desktop UI screenshots
│   └── website/              # Static landing site & interactive CSS preview
├── scripts/                  # Build, validation, icon generator & workspace cleanup scripts
│   ├── build-frontend.mjs    # Validates JS modules and copies src/ into dist/
│   ├── check-frontend.mjs    # Verifies required assets/fonts and checks ESM syntax via Node
│   ├── clean.mjs             # Reclaims bulky Cargo debug/intermediate caches (`npm run clean`)
│   ├── dev-server.mjs        # Static dev server for `tauri dev`
│   └── generate-icons.mjs    # Regenerates ICO, PNG, tray icons, and NSIS wizard BMPs
├── src/                      # Frontend runtime source (copied to dist/ during build)
│   ├── assets/               # Application icon and bundled Inter / JetBrains Mono WOFF2 fonts
│   ├── js/                   # Core window controllers, store, theme helpers & 12 island panels
│   └── styles/               # Design tokens, glassmorphic capsule, settings studio & tray CSS
└── src-tauri/                # Native Rust backend & Windows integrations
    ├── capabilities/         # Least-privilege per-window Tauri v2 ACL manifests
    ├── icons/                # Packaged Windows icons and PNG sizes
    ├── nsis/                 # NSIS installer hooks (upgrade stop, Startup registration, cleanup)
    ├── src/
    │   ├── commands.rs       # Tauri IPC commands (`#[tauri::command]`) and shared AppState
    │   ├── config.rs         # Atomic config persistence, bounds clamping & startup migration
    │   ├── island/           # Capsule positioning, DPI math, click-through loop & Live Activities
    │   ├── lib.rs            # App setup, tray menu, hotkey registration & adaptive event loops
    │   └── services/         # Native WinRT/Win32/COM telemetry, media, audio & OS integrations
    └── tests/                # Windows services integration smoke tests
```

## Runtime Topology

### Windows & Capabilities
- **`island`** (`index.html`): Frameless, transparent, always-on-top `WS_EX_TOOLWINDOW` overlay clamped inside the target monitor's logical bounds. Click-through (`WS_EX_TRANSPARENT`) is updated by `spawn_click_through_controller()` whenever the cursor moves relative to the reported capsule hitbox.
- **`settings`** (`settings.html`): Desktop Settings Studio window with `core:window:allow-destroy` capability and a native non-blocking `on_window_event` hook (`restore_saved_appearance`) that cleans up unsaved live previews and Desktop Reposition Mode cleanly on `WM_CLOSE`.
- **`tray`** (`tray.html`): Custom obsidian glass system tray popup clamped within the monitor work area.

### Native Windows Integrations (`src-tauri/src/services/`)
- **Media (`media.rs`)**: WinRT Global System Media Transport Controls (`GSMTC`) event listener + snapshot reader & thumbnail cache.
- **Audio & Display (`system_ctrl.rs`)**: WASAPI `IAudioEndpointVolumeCallback` / `IMMNotificationClient`, Windows COM `IWbemLocator` / DDC-CI brightness, power status, and `GetIfTable2` IEEE 802.11 status.
- **Hardware (`hardware.rs`)**: `sysinfo` CPU + RAM + Disk telemetry, cached DXGI adapter enumeration, PDH 3D engine GPU fallback, and NVIDIA NVML telemetry where available.
- **Gaming & Downloads (`gaming.rs`, `downloads.rs`)**: Lightweight process CPU/memory scans + native Win32 registry Steam AppID/path lookups (`RegGetValueW`) without DLL injection, plus 30s-cached Steam `.acf` & browser partial download tracking.
- **Alerts, SMS Ding & Audio (`notifications.rs`, `sound.rs`)**: SMS/messaging classification, event-checked WinRT Action Center watcher when granted MSIX identity, DND/Focus suppression, and synthesized in-memory PCM bell chimes via `Win32 PlaySoundW`.
- **Window Hooks & Clipboard (`win_hooks.rs`, `clipboard.rs`)**: `SetWinEventHook` foreground tracking, `SHQueryUserNotificationState` fullscreen checks, and `AddClipboardFormatListener` with sensitive token filtering.

### Windows Startup Lifecycle
- Fresh configurations default to `autostart: true` (`autostart_initialized: true`), upgrading legacy `<= v1.4.0` configs once while respecting explicit opt-outs saved in Settings Studio.
- The NSIS post-install hook registers `HKCU\Software\Microsoft\Windows\CurrentVersion\Run\PILLVYN` cleanly during setup and removes it automatically on uninstall.

### Build Profiles & Workspace Size Hygiene
- `src-tauri/Cargo.toml` pairs `opt-level = "s"`, `lto = true`, `codegen-units = 1`, `panic = "abort"`, and `strip = true` in `[profile.release]` with lightweight line-table debuginfo in `[profile.dev]` to avoid multi-gigabyte PDB/DWARF accumulation.
- Run `npm run clean` anytime to prune Cargo intermediate caches while keeping the release binary and NSIS installer intact (`npm run clean:all` removes `dist/` and `src-tauri/target/` completely).
