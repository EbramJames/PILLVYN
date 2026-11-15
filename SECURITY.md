# Security Policy

## Supported Versions

| Version | Supported |
| ------- | --------- |
| 1.0.x   | Yes       |

## Security Architecture
- **Restrictive Tauri Capabilities**: `island` and `settings` windows each carry isolated capability files (`src-tauri/capabilities/default.json` and `settings.json`) with no unrestricted filesystem or shell plugins.
- **Validated IPC Commands**: Quick Actions, external URLs, hex color themes, AI endpoints, and repository slugs are strictly validated in Rust before execution.
- **No Anti-Cheat / Process Injection**: PILLVYN never injects DLLs into games or privileged system processes.
- **Privacy Controls**:
  - Clipboard history can be paused or cleared at any time and skips common secret/token prefixes by default.
  - Update checks are opt-in and only contact GitHub Releases when explicitly consented to in Settings.
  - Optional AI integration defaults to disabled and requires no paid API keys.

## Reporting a Vulnerability
Please **do not** open a public GitHub issue for security vulnerabilities.
Instead, use GitHub's **Private Vulnerability Reporting** ("Security -> Advisories -> Report a vulnerability") with steps to reproduce and potential impact. We aim to acknowledge reports within 48 hours.
