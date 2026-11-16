# Contributing to PILLVYN

Thank you for helping make **PILLVYN** the cleanest, fastest Dynamic Island experience for Windows 10 and Windows 11.

## Engineering Principles
1. **Zero fake data or stubbed controls**: Every control, indicator, and sensor must connect to real Windows APIs or gracefully indicate unavailability.
2. **Low idle footprint**: Prefer event-driven Win32/WinRT hooks (`AddClipboardFormatListener`, `SetWinEventHook`, WASAPI audio callbacks, and GSMTC session listeners) over high-frequency polling.
3. **Strict security boundaries**: Keep Tauri capabilities minimal, validate all IPC inputs in Rust, never embed secrets, and avoid `innerHTML` with untrusted content.
4. **Clean, modular structure**:
   - `src/`: Vanilla JS, HTML5, CSS3 frontend
   - `src-tauri/src/services/`: Independent Windows API and service integrations
   - `src-tauri/src/island/`: Window geometry, DPI positioning, click-through hit-testing, Live Activity coordination

## Local Development Setup
- **Windows 10 / 11 (x64)**
- **Rust** (`rustup`, stable MSVC toolchain)
- **Visual Studio Build Tools 2022** (*Desktop development with C++* + Windows 10/11 SDK)
- **Node.js 20+**
- **NSIS 3.x** (for building the Windows setup executable)

```bash
npm install
npm run check:frontend
cargo test --manifest-path src-tauri/Cargo.toml
npm run dev
```

## Creating a Plugin
Drop a folder inside `%APPDATA%\pillvyn\PILLVYN\config\plugins\<your-plugin>\` containing a `plugin.json`:

```json
{
  "id": "my-plugin",
  "name": "My Utility",
  "version": "2.0.0",
  "author": "You",
  "description": "Triggers a custom Live Activity card.",
  "enabled": true,
  "actions": [
    {
      "id": "pin-status",
      "label": "Pin Status",
      "kind": "activity",
      "target": "Build Finished"
    }
  ]
}
```
