# Security & Privacy Policy

## Privacy by Design

- **Local-only processing:** Notifications, clipboard history, notes, file shelf items, and screen OCR text extraction run entirely on your machine. Nothing is uploaded to external servers.
- **DPAPI-encrypted password vault:** Saved credentials in `%APPDATA%\PILLVYN\vault.bin` are protected with Windows DPAPI (`CryptProtectData`, tied to your Windows user account) with application-specific entropy. Copied secrets are cleared from the clipboard automatically after 30 seconds.
- **Single optional network call:** PILLVYN's only outbound network request is checking `https://github.com/EbramJames/PILLVYN/releases/latest/download` for updates (which can be disabled in **Settings → About → Update automatically**).
- **Verified updates:** Downloaded installers are checked against their published SHA-256 checksum (`PILLVYN-Setup.exe.sha256` and `latest.json`) before installation; mismatched files are discarded immediately.

## Reporting a Vulnerability

Please open a private security advisory on [GitHub Security Advisories](https://github.com/EbramJames/PILLVYN/security/advisories) or contact the repository maintainer directly rather than posting exploit details in public issues.
