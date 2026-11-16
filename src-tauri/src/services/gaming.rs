use crate::config::GamingConfig;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fs;
use std::path::PathBuf;
use sysinfo::{Pid, ProcessRefreshKind, RefreshKind, System};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppIntegrationStatus {
    pub spotify_running: bool,
    pub discord_running: bool,
    pub steam_running: bool,
    pub obs_running: bool,
    pub vlc_running: bool,
    pub browser_running: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GamingSnapshot {
    pub active: bool,
    pub game_title: Option<String>,
    pub process_name: Option<String>,
    pub pid: Option<u32>,
    pub steam_app_id: Option<u32>,
    pub process_cpu_percent: Option<f32>,
    pub process_ram_mb: Option<u64>,
    pub foreground_fullscreen: bool,
    pub notifications_suppressed: bool,
    pub fps: Option<f32>,
    pub fps_status: String,
    pub integrations: AppIntegrationStatus,
}

const KNOWN_GAME_PROCESSES: &[(&str, &str)] = &[
    ("valorant-win64-shipping.exe", "VALORANT"),
    ("cs2.exe", "Counter-Strike 2"),
    ("dota2.exe", "Dota 2"),
    ("eldenring.exe", "ELDEN RING"),
    ("cyberpunk2077.exe", "Cyberpunk 2077"),
    ("witcher3.exe", "The Witcher 3"),
    ("gtav.exe", "Grand Theft Auto V"),
    ("gta5.exe", "Grand Theft Auto V"),
    ("rdr2.exe", "Red Dead Redemption 2"),
    ("re2.exe", "Resident Evil 2"),
    ("re4.exe", "Resident Evil 4"),
    ("fortniteclient-win64-shipping.exe", "Fortnite"),
    ("r5apex.exe", "Apex Legends"),
    ("overwatch.exe", "Overwatch 2"),
    ("rocketleague.exe", "Rocket League"),
    ("javaw.exe", "Minecraft"),
    ("minecraft.windows.exe", "Minecraft Bedrock"),
    ("bg3.exe", "Baldur's Gate 3"),
    ("bg3_dx11.exe", "Baldur's Gate 3"),
    ("helldivers2.exe", "HELLDIVERS 2"),
    ("starfield.exe", "Starfield"),
    ("palworld-win64-shipping.exe", "Palworld"),
    ("terraria.exe", "Terraria"),
    ("hollow_knight.exe", "Hollow Knight"),
    ("hades.exe", "Hades"),
    ("hades2.exe", "Hades II"),
    ("stardew valley.exe", "Stardew Valley"),
    ("league of legends.exe", "League of Legends"),
];

const NON_GAME_EXES: &[&str] = &[
    "explorer.exe",
    "shellexperiencehost.exe",
    "searchhost.exe",
    "startmenuexperiencehost.exe",
    "applicationframehost.exe",
    "taskmgr.exe",
    "cmd.exe",
    "powershell.exe",
    "pwsh.exe",
    "windowsterminal.exe",
    "code.exe",
    "devenv.exe",
    "chrome.exe",
    "msedge.exe",
    "firefox.exe",
    "brave.exe",
    "opera.exe",
    "discord.exe",
    "spotify.exe",
    "vlc.exe",
    "obs64.exe",
    "obs32.exe",
    "steam.exe",
    "steamwebhelper.exe",
    "pillvyn.exe",
];

pub struct GamingMonitor {
    sys: System,
}

impl Default for GamingMonitor {
    fn default() -> Self {
        Self::new()
    }
}

impl GamingMonitor {
    pub fn new() -> Self {
        let sys = System::new_with_specifics(
            RefreshKind::nothing().with_processes(ProcessRefreshKind::everything()),
        );
        Self { sys }
    }

    pub fn collect(&mut self, cfg: &GamingConfig) -> (GamingSnapshot, Option<String>) {
        self.sys.refresh_processes_specifics(
            sysinfo::ProcessesToUpdate::All,
            true,
            ProcessRefreshKind::everything(),
        );

        let mut running_names: HashSet<String> = HashSet::new();
        for proc_item in self.sys.processes().values() {
            running_names.insert(proc_item.name().to_string_lossy().to_ascii_lowercase());
        }

        let browser_running = [
            "chrome.exe",
            "msedge.exe",
            "firefox.exe",
            "brave.exe",
            "opera.exe",
        ]
        .iter()
        .find(|b| running_names.contains(**b))
        .map(|b| {
            match *b {
                "chrome.exe" => "Chrome",
                "msedge.exe" => "Edge",
                "firefox.exe" => "Firefox",
                "brave.exe" => "Brave",
                _ => "Opera",
            }
            .to_string()
        });

        let integrations = AppIntegrationStatus {
            spotify_running: running_names.contains("spotify.exe"),
            discord_running: running_names.contains("discord.exe")
                || running_names.contains("discordptb.exe"),
            steam_running: running_names.contains("steam.exe"),
            obs_running: running_names.contains("obs64.exe") || running_names.contains("obs32.exe"),
            vlc_running: running_names.contains("vlc.exe"),
            browser_running,
        };

        let fg = query_foreground_window_process();
        let fg_exe = fg.as_ref().map(|f| f.exe_name.to_ascii_lowercase());

        if !cfg.auto_detect {
            return (
                GamingSnapshot {
                    active: false,
                    game_title: None,
                    process_name: fg.as_ref().map(|f| f.exe_name.clone()),
                    pid: fg.as_ref().map(|f| f.pid),
                    steam_app_id: None,
                    process_cpu_percent: None,
                    process_ram_mb: None,
                    foreground_fullscreen: fg.as_ref().is_some_and(|f| f.fullscreen),
                    notifications_suppressed: false,
                    fps: None,
                    fps_status: "Auto game detection is disabled in Settings.".into(),
                    integrations,
                },
                fg_exe,
            );
        }

        // 1. Check Steam active RunningAppID in Windows Registry (real Steam state)
        let steam_running_id = query_steam_running_app_id();
        let steam_title = steam_running_id.and_then(lookup_steam_game_title);

        // 2. Match known game process or user custom process or fullscreen non-system foreground app
        let custom_set: HashSet<String> = cfg
            .custom_processes
            .iter()
            .map(|s| s.trim().to_ascii_lowercase())
            .filter(|s| !s.is_empty())
            .collect();

        let mut matched_pid: Option<u32> = None;
        let mut matched_exe: Option<String> = None;
        let mut matched_title: Option<String> = None;

        if let Some(ref f) = fg {
            let lower = f.exe_name.to_ascii_lowercase();
            if custom_set.contains(&lower) {
                matched_pid = Some(f.pid);
                matched_exe = Some(f.exe_name.clone());
                matched_title = Some(clean_title_from_exe(&f.exe_name, &f.window_title));
            } else if let Some((_, title)) =
                KNOWN_GAME_PROCESSES.iter().find(|(exe, _)| *exe == lower)
            {
                matched_pid = Some(f.pid);
                matched_exe = Some(f.exe_name.clone());
                matched_title = Some((*title).to_string());
            } else if steam_running_id.is_some()
                && !NON_GAME_EXES.contains(&lower.as_str())
                && f.fullscreen
            {
                matched_pid = Some(f.pid);
                matched_exe = Some(f.exe_name.clone());
                matched_title = steam_title
                    .clone()
                    .or_else(|| Some(clean_title_from_exe(&f.exe_name, &f.window_title)));
            }
        }

        // Also check background running known/custom games or active Steam RunningAppID even when tabbed out
        if matched_title.is_none() {
            for proc_item in self.sys.processes().values() {
                let pname = proc_item.name().to_string_lossy().to_ascii_lowercase();
                if custom_set.contains(&pname) {
                    matched_pid = Some(proc_item.pid().as_u32());
                    matched_exe = Some(proc_item.name().to_string_lossy().to_string());
                    matched_title = Some(clean_title_from_exe(&pname, ""));
                    break;
                }
                if let Some((_, title)) = KNOWN_GAME_PROCESSES.iter().find(|(exe, _)| *exe == pname)
                {
                    matched_pid = Some(proc_item.pid().as_u32());
                    matched_exe = Some(proc_item.name().to_string_lossy().to_string());
                    matched_title = Some((*title).to_string());
                    break;
                }
            }
        }

        if matched_title.is_none() && steam_running_id.is_some() {
            matched_title =
                steam_title.or_else(|| steam_running_id.map(|id| format!("Steam App #{id}")));
        }

        let (process_cpu_percent, process_ram_mb) = matched_pid
            .and_then(|p| self.sys.process(Pid::from_u32(p)))
            .map(|p| (Some(p.cpu_usage()), Some(p.memory() / (1024 * 1024))))
            .unwrap_or((None, None));

        let active = matched_title.is_some();
        let fg_full = fg.as_ref().is_some_and(|f| f.fullscreen);

        (
            GamingSnapshot {
                active,
                game_title: matched_title,
                process_name: matched_exe.or_else(|| fg.as_ref().map(|f| f.exe_name.clone())),
                pid: matched_pid.or_else(|| fg.as_ref().map(|f| f.pid)),
                steam_app_id: steam_running_id,
                process_cpu_percent,
                process_ram_mb,
                foreground_fullscreen: fg_full,
                notifications_suppressed: active && cfg.suppress_notifications,
                fps: None,
                fps_status: "Anti-cheat safe mode: PILLVYN does not inject DLLs into games. Live CPU/RAM/GPU telemetry active; external FPS telemetry requires an optional ETW/PresentMon plugin.".to_string(),
                integrations,
            },
            fg_exe,
        )
    }
}

fn clean_title_from_exe(exe: &str, window_title: &str) -> String {
    let trimmed = window_title.trim();
    if !trimmed.is_empty() && trimmed.len() <= 48 {
        return trimmed.to_string();
    }
    exe.strip_suffix(".exe")
        .or_else(|| exe.strip_suffix(".EXE"))
        .unwrap_or(exe)
        .to_string()
}

#[derive(Debug, Clone)]
pub struct ForegroundInfo {
    pub pid: u32,
    pub exe_name: String,
    pub window_title: String,
    pub fullscreen: bool,
}

#[cfg(target_os = "windows")]
pub fn query_foreground_window_process() -> Option<ForegroundInfo> {
    use windows::core::PWSTR;
    use windows::Win32::Foundation::{CloseHandle, MAX_PATH, RECT};
    use windows::Win32::Graphics::Gdi::{
        GetMonitorInfoW, MonitorFromWindow, MONITORINFO, MONITOR_DEFAULTTONEAREST,
    };
    use windows::Win32::System::Threading::{
        OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_WIN32,
        PROCESS_QUERY_LIMITED_INFORMATION,
    };
    use windows::Win32::UI::WindowsAndMessaging::{
        GetForegroundWindow, GetWindowRect, GetWindowTextW, GetWindowThreadProcessId,
    };

    unsafe {
        let hwnd = GetForegroundWindow();
        if hwnd.0.is_null() {
            return None;
        }
        let mut pid = 0u32;
        GetWindowThreadProcessId(hwnd, Some(&mut pid));
        if pid == 0 {
            return None;
        }

        let mut title_buf = [0u16; 256];
        let tlen = GetWindowTextW(hwnd, &mut title_buf);
        let window_title = if tlen > 0 {
            String::from_utf16_lossy(&title_buf[..tlen as usize])
        } else {
            String::new()
        };

        let mut exe_name = String::new();
        if let Ok(handle) = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) {
            let mut path_buf = [0u16; MAX_PATH as usize];
            let mut size = path_buf.len() as u32;
            if QueryFullProcessImageNameW(
                handle,
                PROCESS_NAME_WIN32,
                PWSTR(path_buf.as_mut_ptr()),
                &mut size,
            )
            .is_ok()
            {
                let full = String::from_utf16_lossy(&path_buf[..size as usize]);
                exe_name = PathBuf::from(full)
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_default();
            }
            let _ = CloseHandle(handle);
        }

        let mut win_rect = RECT::default();
        let mut fullscreen = false;
        if GetWindowRect(hwnd, &mut win_rect).is_ok() {
            let hmon = MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST);
            let mut mi = MONITORINFO {
                cbSize: std::mem::size_of::<MONITORINFO>() as u32,
                ..Default::default()
            };
            if GetMonitorInfoW(hmon, &mut mi).as_bool() {
                let win_covers = win_rect.left <= mi.rcMonitor.left
                    && win_rect.top <= mi.rcMonitor.top
                    && win_rect.right >= mi.rcMonitor.right
                    && win_rect.bottom >= mi.rcMonitor.bottom;
                let shell_fullscreen = !matches!(
                    exe_name.to_ascii_lowercase().as_str(),
                    "explorer.exe" | "shellexperiencehost.exe" | "searchhost.exe" | "pillvyn.exe"
                )
                    && crate::services::win_hooks::is_fullscreen_notification_state();
                fullscreen = win_covers || shell_fullscreen;
            }
        }

        Some(ForegroundInfo {
            pid,
            exe_name,
            window_title,
            fullscreen,
        })
    }
}

#[cfg(not(target_os = "windows"))]
pub fn query_foreground_window_process() -> Option<ForegroundInfo> {
    None
}

#[cfg(target_os = "windows")]
fn query_steam_running_app_id() -> Option<u32> {
    use windows::core::w;
    use windows::Win32::System::Registry::{RegGetValueW, HKEY_CURRENT_USER, RRF_RT_REG_DWORD};

    let mut value = 0u32;
    let mut size = std::mem::size_of::<u32>() as u32;
    let status = unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            w!("Software\\Valve\\Steam"),
            w!("RunningAppID"),
            RRF_RT_REG_DWORD,
            None,
            Some(&mut value as *mut u32 as *mut core::ffi::c_void),
            Some(&mut size),
        )
    };
    if status.is_ok() && value > 0 {
        Some(value)
    } else {
        None
    }
}

#[cfg(not(target_os = "windows"))]
fn query_steam_running_app_id() -> Option<u32> {
    None
}

pub fn find_steam_library_folders() -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    let mut candidates = vec![
        PathBuf::from(r"C:\Program Files (x86)\Steam"),
        PathBuf::from(r"C:\Program Files\Steam"),
        PathBuf::from(r"D:\SteamLibrary"),
        PathBuf::from(r"H:\SteamLibrary"),
    ];

    #[cfg(target_os = "windows")]
    if let Some(reg_path) = query_steam_install_path() {
        if !candidates.contains(&reg_path) {
            candidates.insert(0, reg_path);
        }
    }

    for base in candidates {
        let steamapps = base.join("steamapps");
        if steamapps.is_dir() && !dirs.contains(&steamapps) {
            dirs.push(steamapps.clone());
        } else if base.is_dir() && base.file_name().is_some_and(|n| n == "SteamLibrary") {
            let sub = base.join("steamapps");
            if sub.is_dir() && !dirs.contains(&sub) {
                dirs.push(sub);
            }
        }
        let vdf = base.join("steamapps").join("libraryfolders.vdf");
        if let Ok(content) = fs::read_to_string(vdf) {
            for line in content.lines() {
                let trimmed = line.trim();
                if trimmed.starts_with("\"path\"") {
                    let parts: Vec<&str> = trimmed.split('"').collect();
                    if parts.len() >= 4 {
                        let lib = PathBuf::from(parts[3].replace("\\\\", "\\")).join("steamapps");
                        if lib.is_dir() && !dirs.contains(&lib) {
                            dirs.push(lib);
                        }
                    }
                }
            }
        }
    }
    dirs
}

#[cfg(target_os = "windows")]
fn query_steam_install_path() -> Option<PathBuf> {
    use windows::core::w;
    use windows::Win32::System::Registry::{RegGetValueW, HKEY_CURRENT_USER, RRF_RT_REG_SZ};

    let mut buf = [0u16; 512];
    let mut size = (buf.len() * std::mem::size_of::<u16>()) as u32;
    let status = unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            w!("Software\\Valve\\Steam"),
            w!("SteamPath"),
            RRF_RT_REG_SZ,
            None,
            Some(buf.as_mut_ptr() as *mut core::ffi::c_void),
            Some(&mut size),
        )
    };
    if status.is_ok() && size >= 2 {
        let len = (size as usize / 2).saturating_sub(1);
        let s = String::from_utf16_lossy(&buf[..len])
            .trim_end_matches('\0')
            .to_string();
        if !s.is_empty() {
            return Some(PathBuf::from(s));
        }
    }
    None
}

fn lookup_steam_game_title(app_id: u32) -> Option<String> {
    for dir in find_steam_library_folders() {
        let manifest = dir.join(format!("appmanifest_{app_id}.acf"));
        if let Ok(text) = fs::read_to_string(manifest) {
            for line in text.lines() {
                let trimmed = line.trim();
                if trimmed.starts_with("\"name\"") {
                    let parts: Vec<&str> = trimmed.split('"').collect();
                    if parts.len() >= 4 {
                        return Some(parts[3].to_string());
                    }
                }
            }
        }
    }
    None
}
