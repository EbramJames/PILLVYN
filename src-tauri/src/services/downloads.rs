use crate::island::activities::now_ms;
use crate::services::gaming::find_steam_library_folders;
use directories::UserDirs;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::time::Instant;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DownloadItem {
    pub id: String,
    pub name: String,
    pub source: String,
    pub downloaded_bytes: u64,
    pub total_bytes: Option<u64>,
    pub speed_bps: f64,
    pub progress_percent: Option<f64>,
    pub eta_seconds: Option<u64>,
    pub status: String, // "downloading" | "completed"
    pub updated_at_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DownloadsSnapshot {
    pub watched_sources: Vec<String>,
    pub active_count: usize,
    pub items: Vec<DownloadItem>,
}

struct TrackedState {
    last_bytes: u64,
    last_ts_ms: u64,
    smoothed_bps: f64,
    name: String,
    source: String,
    total_bytes: Option<u64>,
}

pub struct DownloadMonitor {
    downloads_dir: Option<PathBuf>,
    tracked: HashMap<String, TrackedState>,
    recent_completed: Vec<DownloadItem>,
    steam_dirs_cache: Vec<PathBuf>,
    last_steam_dir_scan: Option<Instant>,
}

impl Default for DownloadMonitor {
    fn default() -> Self {
        Self::new()
    }
}

impl DownloadMonitor {
    pub fn new() -> Self {
        let downloads_dir = UserDirs::new().and_then(|u| u.download_dir().map(|p| p.to_path_buf()));
        Self {
            downloads_dir,
            tracked: HashMap::new(),
            recent_completed: Vec::new(),
            steam_dirs_cache: Vec::new(),
            last_steam_dir_scan: None,
        }
    }

    fn cached_steam_library_folders(&mut self) -> Vec<PathBuf> {
        let stale = self
            .last_steam_dir_scan
            .is_none_or(|t| t.elapsed().as_secs() >= 30);
        if stale {
            self.steam_dirs_cache = find_steam_library_folders();
            self.last_steam_dir_scan = Some(Instant::now());
        }
        self.steam_dirs_cache.clone()
    }

    pub fn collect(&mut self) -> DownloadsSnapshot {
        let now = now_ms();
        let mut active_items = Vec::new();
        let mut seen_ids = Vec::new();
        let mut watched_sources = Vec::new();

        // 1. Scan user's Downloads folder for active partial files (.crdownload, .part, .opdownload, .tmp)
        if let Some(dir) = &self.downloads_dir {
            watched_sources.push(dir.to_string_lossy().to_string());
            if let Ok(entries) = fs::read_dir(dir) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if !path.is_file() {
                        continue;
                    }
                    let fname = entry.file_name().to_string_lossy().to_string();
                    let lower = fname.to_ascii_lowercase();
                    let browser_source = if lower.ends_with(".crdownload") {
                        Some("Browser (Chromium)")
                    } else if lower.ends_with(".part") {
                        Some("Firefox")
                    } else if lower.ends_with(".opdownload") {
                        Some("Opera")
                    } else {
                        None
                    };

                    if let Some(src) = browser_source {
                        let Ok(meta) = entry.metadata() else {
                            continue;
                        };
                        let bytes = meta.len();
                        let clean_name = fname
                            .trim_end_matches(".crdownload")
                            .trim_end_matches(".part")
                            .trim_end_matches(".opdownload")
                            .to_string();
                        let id = format!("file:{}", path.to_string_lossy());
                        seen_ids.push(id.clone());

                        let speed = self.update_speed(
                            &id,
                            clean_name.clone(),
                            src.to_string(),
                            bytes,
                            None,
                            now,
                        );
                        active_items.push(DownloadItem {
                            id,
                            name: clean_name,
                            source: src.to_string(),
                            downloaded_bytes: bytes,
                            total_bytes: None,
                            speed_bps: speed,
                            progress_percent: None,
                            eta_seconds: None,
                            status: "downloading".to_string(),
                            updated_at_ms: now,
                        });
                    }
                }
            }
        }

        // 2. Scan Steam appmanifest_*.acf for active Steam game downloads (30s directory scan TTL)
        let steam_dirs = self.cached_steam_library_folders();
        if !steam_dirs.is_empty() {
            watched_sources.push("Steam Libraries".to_string());
        }
        for sdir in steam_dirs {
            let Ok(entries) = fs::read_dir(&sdir) else {
                continue;
            };
            for entry in entries.flatten() {
                let fname = entry.file_name().to_string_lossy().to_string();
                if !fname.starts_with("appmanifest_") || !fname.ends_with(".acf") {
                    continue;
                }
                let Ok(content) = fs::read_to_string(entry.path()) else {
                    continue;
                };
                if let Some(dl) = parse_steam_acf_download(&content) {
                    let id = format!("steam:{}", dl.app_id);
                    seen_ids.push(id.clone());
                    let speed = self.update_speed(
                        &id,
                        dl.name.clone(),
                        "Steam".to_string(),
                        dl.bytes_downloaded,
                        Some(dl.bytes_to_download),
                        now,
                    );
                    let pct = if dl.bytes_to_download > 0 {
                        Some(
                            ((dl.bytes_downloaded as f64 / dl.bytes_to_download as f64) * 100.0)
                                .min(100.0),
                        )
                    } else {
                        None
                    };
                    let remaining = dl.bytes_to_download.saturating_sub(dl.bytes_downloaded);
                    let eta = if speed > 1024.0 && remaining > 0 {
                        Some((remaining as f64 / speed).round() as u64)
                    } else {
                        None
                    };
                    active_items.push(DownloadItem {
                        id,
                        name: dl.name,
                        source: "Steam".to_string(),
                        downloaded_bytes: dl.bytes_downloaded,
                        total_bytes: Some(dl.bytes_to_download),
                        speed_bps: speed,
                        progress_percent: pct,
                        eta_seconds: eta,
                        status: "downloading".to_string(),
                        updated_at_ms: now,
                    });
                }
            }
        }

        // Move disappeared active items to recent completed (kept for 4 minutes)
        let disappeared: Vec<String> = self
            .tracked
            .keys()
            .filter(|k| !seen_ids.contains(*k))
            .cloned()
            .collect();
        for key in disappeared {
            if let Some(prev) = self.tracked.remove(&key) {
                if prev.last_bytes > 0 {
                    self.recent_completed.insert(
                        0,
                        DownloadItem {
                            id: key,
                            name: prev.name,
                            source: prev.source,
                            downloaded_bytes: prev.total_bytes.unwrap_or(prev.last_bytes),
                            total_bytes: prev.total_bytes,
                            speed_bps: 0.0,
                            progress_percent: Some(100.0),
                            eta_seconds: Some(0),
                            status: "completed".to_string(),
                            updated_at_ms: now,
                        },
                    );
                }
            }
        }
        self.recent_completed
            .retain(|item| now.saturating_sub(item.updated_at_ms) < 240_000);
        self.recent_completed.truncate(6);

        let active_count = active_items.len();
        active_items.extend(self.recent_completed.iter().cloned());

        DownloadsSnapshot {
            watched_sources,
            active_count,
            items: active_items,
        }
    }

    fn update_speed(
        &mut self,
        id: &str,
        name: String,
        source: String,
        bytes: u64,
        total_bytes: Option<u64>,
        now_ms: u64,
    ) -> f64 {
        match self.tracked.get_mut(id) {
            Some(st) => {
                let dt_sec = (now_ms.saturating_sub(st.last_ts_ms) as f64) / 1000.0;
                if dt_sec >= 0.25 {
                    let delta = bytes.saturating_sub(st.last_bytes) as f64;
                    let instant = delta / dt_sec;
                    st.smoothed_bps = if st.smoothed_bps <= 0.0 {
                        instant
                    } else {
                        st.smoothed_bps * 0.55 + instant * 0.45
                    };
                    st.last_bytes = bytes;
                    st.last_ts_ms = now_ms;
                }
                st.name = name;
                st.total_bytes = total_bytes;
                st.smoothed_bps
            }
            None => {
                self.tracked.insert(
                    id.to_string(),
                    TrackedState {
                        last_bytes: bytes,
                        last_ts_ms: now_ms,
                        smoothed_bps: 0.0,
                        name,
                        source,
                        total_bytes,
                    },
                );
                0.0
            }
        }
    }
}

struct SteamDownloadParse {
    app_id: String,
    name: String,
    bytes_downloaded: u64,
    bytes_to_download: u64,
}

fn parse_steam_acf_download(content: &str) -> Option<SteamDownloadParse> {
    let mut app_id = String::new();
    let mut name = String::new();
    let mut state_flags = 0u32;
    let mut bytes_to_download = 0u64;
    let mut bytes_downloaded = 0u64;

    for line in content.lines() {
        let parts: Vec<&str> = line.trim().split('"').collect();
        if parts.len() < 4 {
            continue;
        }
        match parts[1] {
            "appid" => app_id = parts[3].to_string(),
            "name" => name = parts[3].to_string(),
            "StateFlags" => state_flags = parts[3].parse().unwrap_or(0),
            "BytesToDownload" => bytes_to_download = parts[3].parse().unwrap_or(0),
            "BytesDownloaded" => bytes_downloaded = parts[3].parse().unwrap_or(0),
            _ => {}
        }
    }

    // Require Steam active transfer bits (256 = UpdateRunning or 1024 = UpdateStarted)
    // and ensure UpdatePaused (512) is not set so scheduled/paused backlog items don't spam the island.
    let paused = (state_flags & 512) != 0;
    let active_transfer = (state_flags & (256 | 1024)) != 0;
    if active_transfer && !paused && bytes_to_download > 0 && bytes_downloaded < bytes_to_download {
        Some(SteamDownloadParse {
            app_id,
            name: if name.is_empty() {
                "Steam Game".to_string()
            } else {
                name
            },
            bytes_downloaded,
            bytes_to_download,
        })
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn steam_acf_parser_detects_active_download() {
        let sample = r#"
        "AppState"
        {
            "appid" "730"
            "name" "Counter-Strike 2"
            "StateFlags" "1026"
            "BytesToDownload" "1000000"
            "BytesDownloaded" "450000"
        }
        "#;
        let parsed = parse_steam_acf_download(sample).expect("should parse active download");
        assert_eq!(parsed.app_id, "730");
        assert_eq!(parsed.name, "Counter-Strike 2");
        assert_eq!(parsed.bytes_downloaded, 450_000);

        let paused_sample = r#"
        "AppState"
        {
            "appid" "1304930"
            "name" "The Outlast Trials"
            "StateFlags" "518"
            "BytesToDownload" "23811509054"
            "BytesDownloaded" "0"
        }
        "#;
        assert!(parse_steam_acf_download(paused_sample).is_none());
    }
}
