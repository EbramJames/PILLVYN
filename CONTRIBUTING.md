# Contributing to PILLVYN

Thanks for helping improve **PILLVYN**!

## Prerequisites

- **Windows 10 / 11** with WebView2 Runtime (preinstalled on Windows 11 and modern Windows 10)
- **Rust** (stable toolchain with `rustfmt` and `clippy`)
- **Node.js 20+** (for UI syntax validation)
- **NSIS 3** (`C:\Program Files (x86)\NSIS\makensis.exe`) for building installers

## Project Layout

```text
PILLVYN/
├── .github/workflows/build.yml   # Windows CI lint, test, build & tag release pipeline
├── docs/                         # Branding source, hero screenshot & README assets
├── installer/pillvyn.nsi         # Per-user LZMA NSIS installer & uninstaller
├── scripts/
│   ├── generate-icons.ps1        # Regenerates all PNG & multi-size ICO icons from master artwork
│   └── package.ps1               # Builds release binary, NSIS setup, portable exe, zip & SHA-256
├── src-tauri/                    # Rust + Tauri 2 native Win32/WinRT backend
└── ui/                           # Vanilla HTML/CSS/JS pill UI & offline Tesseract OCR assets
```

## Development Checks

Run all checks locally before opening a pull request:

```powershell
# UI syntax checks
node --check ui/bridge.js
node --check ui/app.js

# Rust formatting, strict Clippy lints, and unit tests
cd src-tauri
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test --locked
```

## Regenerating Icons & Packaging Releases

```powershell
# Regenerate all app, tray, UI, and ICO icons from master artwork
powershell -ExecutionPolicy Bypass -File scripts/generate-icons.ps1

# Build release binary and package all dist/ artifacts + SHA-256 checksums
powershell -ExecutionPolicy Bypass -File scripts/package.ps1
```
