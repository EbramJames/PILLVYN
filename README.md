<div align="center">

<img src="docs/branding/pillvyn-logo.png" width="88" alt="PILLVYN logo">

# PILLVYN

**A sleek, translucent Dynamic Island for Windows.** One refined pill at the top of your screen that shows what is playing,
catches your notifications, remembers what you copied, holds your files, and adapts in real time with Liquid Glass & animated themes.

[![release](https://img.shields.io/github/v/release/EbramJames/PILLVYN?label=release&color=7c4dff)](https://github.com/EbramJames/PILLVYN/releases/latest)
[![downloads](https://img.shields.io/github/downloads/EbramJames/PILLVYN/total?label=downloads&color=555)](https://github.com/EbramJames/PILLVYN/releases)
[![build](https://img.shields.io/github/actions/workflow/status/EbramJames/PILLVYN/build.yml?branch=main&label=build)](https://github.com/EbramJames/PILLVYN/actions/workflows/build.yml)
![size](https://img.shields.io/badge/size-~3.3%20MB-555)
[![license](https://img.shields.io/badge/license-MIT-555)](LICENSE)

[![Windows installer](https://img.shields.io/badge/WINDOWS-INSTALLER-7c4dff?style=for-the-badge)](https://github.com/EbramJames/PILLVYN/releases/latest/download/PILLVYN-Setup.exe)
[![Windows portable exe](https://img.shields.io/badge/WINDOWS-PORTABLE%20.EXE-1f2328?style=for-the-badge)](https://github.com/EbramJames/PILLVYN/releases/latest/download/PILLVYN-Portable.exe)
[![Windows zip](https://img.shields.io/badge/WINDOWS-.ZIP-1f2328?style=for-the-badge)](https://github.com/EbramJames/PILLVYN/releases/latest/download/PILLVYN-Windows.zip)

[Changelog](CHANGELOG.md) · [Contributing](CONTRIBUTING.md) · [Security](SECURITY.md) · [فارسی](#فارسی)

<img src="docs/hero.jpg" width="860" alt="PILLVYN v3.4.0 Ultra Voice AI Dynamic Island for Windows">

</div>

## What it does

- **Hands-Free Ultra Voice AI & Full Windows Agent (`“Hello PILLVYN”` / `«سلام پیلوین»` · `Ctrl+Alt+A`).** Built-in bilingual Persian & English voice pipeline: **Software Mic AGC + RNNoise** neural denoising → **Silero VAD v5** ONNX fast endpointing → pre-warmed local **whisper.cpp (`Small-Q5`)** with `Auto` / `فارسی` / `English` modes → **OpenRouter NVIDIA Nemotron 3 Ultra (free)** (`Fast` ~1.4s / `Deep`) with **26 full-system & PILLVYN tools** (PowerShell, app launcher, window & process control, files, brightness, power, Unicode typing, live web search, timers, music, volume, notes, clipboard, themes, screenshot, OCR) → **streaming sentence-by-sentence Microsoft Edge Neural TTS** (`fa-IR-DilaraNeural`/`Farid` + `en-US-*MultilingualNeural`). API keys are encrypted per-user with Windows DPAPI; local voice models install in one click from **Settings → AI**.
- **Studio-Grade Glass & Atmospheric Themes.** Translucent dark **Liquid Glass** (`glass`) and bright daylight **Frosted Pearl** (`frost`) with optical specular edges, drifting caustics, and cursor-reactive rim light — alongside **Obsidian**, **Aurora**, **Nebula**, **Sapphire**, **Amber Horizon**, and **Venom**, each with cohesive themed dropdown surfaces.
- **Effortless Interactions.** Hover or click the resting pill for instant tools with floating shortcut tooltips, right-click the pill to jump straight into **Settings Studio**, press `Esc` to dismiss open panels, and drag or scroll the interactive volume slider.
- **Now playing.** Title, artist, album art with dynamic cover-accent glow, gradient audio visualizer bars, transport controls, and an interactive hover scrubber for Windows media sessions. Films and fullscreen games are left alone.
- **Categorized Settings Studio inside the island.** Clean **Style**, **Behavior**, **Tools**, **AI**, and **About** tabs with a visual 4×2 Theme Gallery, live Mic & Silero VAD meters, top-edge offset (`0–24px`), optional UI micro-acoustics, and resting pill styles (**Eyes**, **Clock**, or **Logo**) — zero extra windows.
- **Notifications in one line.** Click to open that chat, accompanied by a gentle chime. While you are using the pill, incoming alerts wait neatly beside it instead of interrupting.
- **Focus Timer with presets.** One-click `5m` / `15m` / `25m` chips, scroll-wheel minute adjustment, and a live bottom countdown progress bar that stays visible while running.
- **Smart copy.** Copied links open in one click, math expressions show their answer, dates convert Gregorian ⇄ Shamsi, and hex colours preview their swatch.
- **File shelf.** Drop up to five files onto the pill and drag them back out whenever you need them.
- **Quick notes & reminders.** `Ctrl+Alt+N` from anywhere (`call Ali in 20m`, `at 18:30 gym`, «۱۰ دقیقه دیگه چای»).
- **Encrypted passwords.** Generate and store credentials protected with your Windows account (DPAPI); copied passwords clear from the clipboard after 30 seconds.
- **Offline text grab (OCR).** `Ctrl+Alt+T`, drag a box over any screen region to copy English (Windows OCR) or Persian (bundled Tesseract). Everything stays local.
- **Small signs & synchronized tray.** Subtle dots while the microphone or camera is active, VPN/network transitions, Bluetooth & charger alerts, and a full system tray menu for quick note, OCR, tour, and live-synced startup toggle.
- **Verified self-updates.** Releases download from GitHub Releases and verify their SHA-256 checksum before installing silently.

On its first start PILLVYN greets you and offers *What PILLVYN can do* (always accessible under **Settings → About**).

## Install

Download **[PILLVYN-Setup.exe](https://github.com/EbramJames/PILLVYN/releases/latest/download/PILLVYN-Setup.exe)** and run it.
It installs per-user into `%LOCALAPPDATA%\Programs\PILLVYN` (no administrator prompt) and starts with Windows; both can be configured in **Settings → Behavior**.

Requires Windows 10 or 11 with the WebView2 runtime (built into Windows 11 and modern Windows 10).

> Because the installer is not EV code-signed, Windows SmartScreen may ask once for confirmation. Verify integrity using [PILLVYN-Setup.exe.sha256](https://github.com/EbramJames/PILLVYN/releases/latest/download/PILLVYN-Setup.exe.sha256).

## Privacy

PILLVYN makes no network requests except checking this repository for a new release (which can be disabled in **Settings → About**).
Notifications, clipboard history, notes, OCR scans, and encrypted passwords remain strictly on your device. See [SECURITY.md](SECURITY.md) for details.

## Build & Package

```powershell
# Build & package installer, portable exe, zip, SHA-256 checksums, and latest.json
powershell -ExecutionPolicy Bypass -File scripts/package.ps1
```

Or build the Rust binary directly:

```powershell
cd src-tauri
cargo build --release --locked
```

Icons can be regenerated anytime from master artwork with `scripts/generate-icons.ps1`. See [CONTRIBUTING.md](CONTRIBUTING.md) for development checks and architecture layout.

## License

[MIT](LICENSE). Bundles `tesseract.js` and Tesseract's Persian trained data (Apache 2.0) in `ui/ocr/`.

---

<div dir="rtl">

## فارسی

**PILLVYN یک جزیرهٔ زنده و شیشه‌ای برای ویندوز است.** یک قرص ظریف در بالای صفحه که آهنگ در حال پخش را نشان می‌دهد،
نوتیفیکیشن‌ها را می‌گیرد، چیزهایی که کپی کرده‌ای یادش می‌ماند، فایل‌هایت را نگه می‌دارد و با تم **Liquid Glass** (شیشه‌مورفیسم) و افکت‌های زنده همراهت است.

### امکانات

- **دستیار هوش مصنوعی صوتی فوق‌سریع با دسترسی کامل به ویندوز (فارسی و انگلیسی · `Ctrl+Alt+A`):** با گفتن «سلام پیلوین» یا `Hello PILLVYN` بلافاصله بیدار می‌شود (مجهز به تقویت خودکار میکروفون AGC، حذف نویز زنده **RNNoise**، تشخیص هوشمند پایان جمله **Silero VAD v5** و تطبیق فازی کلمهٔ بیدارباش)؛ تبدیل گفتار به متن دقیق فارسی و انگلیسی با **whisper.cpp (`Small-Q5`)**؛ مغز فوق‌سریع **NVIDIA Nemotron 3 Ultra (free)** روی OpenRouter با ۲۶ ابزار کنترل کامل ویندوز (اجرای PowerShell، باز کردن برنامه‌ها، مدیریت پنجره‌ها و پردازش‌ها، خواندن/نوشتن/جستجوی فایل‌ها، نور صفحه، قفل/خواب/ریستارت، تایپ فارسی/انگلیسی، جستجوی وب، تایمر، موزیک، صدا، یادداشت، کلیپ‌بورد و تم‌ها) و پخش صوتی جمله‌به‌جملهٔ سریع با **Edge TTS** (صداهای فارسی دلارا/فرید و انگلیسی `MultilingualNeural`). کلید API با رمزنگاری Windows DPAPI روی سیستم خودت ذخیره می‌شود.
- **تم‌های شیشه‌ای (Liquid Glass و Frosted Pearl) و تم‌های استودیویی:** تم شیشه‌ای تیره (**Liquid Glass**) و تم شیشه‌ای روشن (**Frosted Pearl**) با انعکاس نور در لبه‌ها و واکنش به مکان‌نما، به‌همراه **Obsidian**، **Aurora**، **Nebula**، **Sapphire**، **Amber Horizon** و **Venom**.
- **تعامل سریع و روان:** کلیک روی جزیره برای باز شدن فوری ابزارها، تولتیپ شناور با نمایش میانبرها، راست‌کلیک برای باز کردن مستقیم تنظیمات، اسلایدر قابل‌کشیدن برای تنظیم صدا و تایمر با کلیدهای سریع ۵، ۱۵ و ۲۵ دقیقه.
- **تنظیمات دسته‌بندی‌شده داخل خود جزیره:** پنج تب منظم (**Style**، **Behavior**، **Tools**، **AI**، **About**) با گالری تصویری تم‌ها، نشانگر زندهٔ میکروفون و Silero VAD، تنظیم فاصله از بالای صفحه (`0–24px`)، افکت‌های صوتی ظریف و انتخاب حالت استراحت جزیره (**Eyes**، **Clock** یا **Logo**).
- **پخش موزیک:** نام آهنگ، خواننده، کاور با هالهٔ رنگی هماهنگ با کاور، اکولایزر گرادیانی زنده، دکمه‌های کنترل و نوار جلو و عقب بردن. با فیلم و بازی تمام‌صفحه کاری ندارد.
- **نوتیفیکیشن در یک خط:** با کلیک همان چت باز می‌شود؛ هنگام کار با جزیره کنار آن منتظر می‌ماند.
- **کپی هوشمند:** لینک‌ها باز می‌شوند، عبارات ریاضی حساب می‌شوند، تاریخ میلادی و شمسی به هم تبدیل می‌شوند و کد رنگ‌ها نمایش داده می‌شوند.
- **نگه‌داشتن فایل:** فایل را روی جزیره رها کن تا کنار هم بمانند و هر وقت خواستی بیرونشان بکشی.
- **یادداشت سریع:** `Ctrl+Alt+N` از هر جا. اگر در متن زمان باشد («۱۰ دقیقه دیگه چای») یادآور می‌شود.
- **رمزها:** رمز می‌سازد و با رمزنگاری حساب ویندوز (DPAPI) ذخیره می‌کند؛ رمز کپی‌شده پس از ۳۰ ثانیه پاک می‌شود.
- **خواندن متن از صفحه (OCR آفلاین):** `Ctrl+Alt+T`، دور متن کادر بکش؛ انگلیسی و فارسی بدون ارسال هیچ داده‌ای به اینترنت.
- **آپدیت خودکار امن:** بررسی چک‌سام SHA-256 پیش از نصب خودکار نسخهٔ جدید.

### نصب

فایل **[PILLVYN-Setup.exe](https://github.com/EbramJames/PILLVYN/releases/latest/download/PILLVYN-Setup.exe)** را دانلود و اجرا کن.
فقط برای کاربر فعلی نصب می‌شود و دسترسی ادمین نمی‌خواهد.

### حریم خصوصی

PILLVYN هیچ درخواست اینترنتی‌ای نمی‌فرستد، به‌جز بررسی همین مخزن برای نسخهٔ جدید (که در تنظیمات خاموش می‌شود).
نوتیفیکیشن‌ها، کلیپ‌بورد، یادداشت‌ها و رمزها روی سیستم خودت می‌مانند.

</div>
