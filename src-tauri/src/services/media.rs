use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MediaSnapshot {
    pub active: bool,
    pub source_app: String,
    pub source_id: String,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub status: String,
    pub position_sec: f64,
    pub duration_sec: f64,
    pub can_play_pause: bool,
    pub can_next: bool,
    pub can_previous: bool,
    pub can_seek: bool,
    pub thumbnail_data_url: Option<String>,
}

impl Default for MediaSnapshot {
    fn default() -> Self {
        Self {
            active: false,
            source_app: String::new(),
            source_id: String::new(),
            title: String::new(),
            artist: String::new(),
            album: String::new(),
            status: "Closed".to_string(),
            position_sec: 0.0,
            duration_sec: 0.0,
            can_play_pause: false,
            can_next: false,
            can_previous: false,
            can_seek: false,
            thumbnail_data_url: None,
        }
    }
}

pub fn friendly_source_name(aumid: &str) -> String {
    let lower = aumid.to_ascii_lowercase();
    if lower.contains("spotify") {
        "Spotify".to_string()
    } else if lower.contains("vlc") {
        "VLC".to_string()
    } else if lower.contains("chrome") {
        "Chrome".to_string()
    } else if lower.contains("msedge") || lower.contains("edge") {
        "Edge".to_string()
    } else if lower.contains("firefox") {
        "Firefox".to_string()
    } else if lower.contains("brave") {
        "Brave".to_string()
    } else if lower.contains("applemusic") {
        "Apple Music".to_string()
    } else if lower.contains("tidal") {
        "TIDAL".to_string()
    } else if lower.contains("foobar") {
        "foobar2000".to_string()
    } else if lower.contains("zune") || lower.contains("mediaplayer") {
        "Media Player".to_string()
    } else if lower.contains("discord") {
        "Discord".to_string()
    } else {
        aumid
            .split('!')
            .next()
            .unwrap_or(aumid)
            .split('.')
            .next_back()
            .unwrap_or("Media")
            .trim_end_matches(".exe")
            .to_string()
    }
}

#[derive(Default)]
pub struct ThumbnailCache {
    last_key: String,
    last_data_url: Option<String>,
}

impl ThumbnailCache {
    pub fn get_if_matching(&self, key: &str) -> Option<Option<String>> {
        if self.last_key == key {
            Some(self.last_data_url.clone())
        } else {
            None
        }
    }

    pub fn set(&mut self, key: String, data_url: Option<String>) {
        self.last_key = key;
        self.last_data_url = data_url;
    }
}

#[cfg(target_os = "windows")]
pub fn query_media_snapshot(cache: &mut ThumbnailCache) -> MediaSnapshot {
    use windows::Media::Control::{
        GlobalSystemMediaTransportControlsSessionManager,
        GlobalSystemMediaTransportControlsSessionPlaybackStatus,
    };
    use windows::Storage::Streams::DataReader;

    let Ok(mgr_op) = GlobalSystemMediaTransportControlsSessionManager::RequestAsync() else {
        return MediaSnapshot::default();
    };
    let Ok(manager) = mgr_op.get() else {
        return MediaSnapshot::default();
    };

    let Ok(session) = manager.GetCurrentSession() else {
        return MediaSnapshot::default();
    };

    let source_id = session
        .SourceAppUserModelId()
        .map(|s| s.to_string())
        .unwrap_or_default();
    let source_app = friendly_source_name(&source_id);

    let playback = session.GetPlaybackInfo();
    let (status_str, can_play_pause, can_next, can_previous) = match playback {
        Ok(info) => {
            let st = match info.PlaybackStatus() {
                Ok(GlobalSystemMediaTransportControlsSessionPlaybackStatus::Playing) => "Playing",
                Ok(GlobalSystemMediaTransportControlsSessionPlaybackStatus::Paused) => "Paused",
                Ok(GlobalSystemMediaTransportControlsSessionPlaybackStatus::Stopped) => "Stopped",
                Ok(GlobalSystemMediaTransportControlsSessionPlaybackStatus::Changing) => "Changing",
                Ok(GlobalSystemMediaTransportControlsSessionPlaybackStatus::Opened) => "Opened",
                _ => "Closed",
            };
            let controls = info.Controls();
            let pp = controls
                .as_ref()
                .ok()
                .and_then(|c| c.IsPlayPauseToggleEnabled().ok())
                .unwrap_or(true);
            let nx = controls
                .as_ref()
                .ok()
                .and_then(|c| c.IsNextEnabled().ok())
                .unwrap_or(true);
            let pv = controls
                .as_ref()
                .ok()
                .and_then(|c| c.IsPreviousEnabled().ok())
                .unwrap_or(true);
            (st.to_string(), pp, nx, pv)
        }
        Err(_) => ("Closed".to_string(), false, false, false),
    };

    let (title, artist, album, thumb_ref) = match session.TryGetMediaPropertiesAsync() {
        Ok(op) => match op.get() {
            Ok(props) => (
                props.Title().map(|s| s.to_string()).unwrap_or_default(),
                props.Artist().map(|s| s.to_string()).unwrap_or_default(),
                props
                    .AlbumTitle()
                    .map(|s| s.to_string())
                    .unwrap_or_default(),
                props.Thumbnail().ok(),
            ),
            Err(_) => (String::new(), String::new(), String::new(), None),
        },
        Err(_) => (String::new(), String::new(), String::new(), None),
    };

    let (position_sec, duration_sec) = match session.GetTimelineProperties() {
        Ok(tl) => {
            let start = tl.StartTime().map(|d| d.Duration).unwrap_or(0);
            let end = tl.EndTime().map(|d| d.Duration).unwrap_or(0);
            let pos = tl.Position().map(|d| d.Duration).unwrap_or(0);
            let dur = ((end - start).max(0) as f64) / 10_000_000.0;
            let cur = ((pos - start).max(0) as f64) / 10_000_000.0;
            (cur.min(dur.max(cur)), dur)
        }
        Err(_) => (0.0, 0.0),
    };

    let key = format!("{source_id}|{title}|{artist}|{album}");
    let thumbnail_data_url = if title.is_empty() && artist.is_empty() {
        None
    } else if let Some(cached) = cache.get_if_matching(&key) {
        cached
    } else {
        let loaded = thumb_ref.and_then(|tref| {
            let stream = tref.OpenReadAsync().ok()?.get().ok()?;
            let size = stream.Size().ok()? as usize;
            if size == 0 || size > 2_500_000 {
                return None;
            }
            let content_type = stream
                .ContentType()
                .map(|c| c.to_string())
                .unwrap_or_else(|_| "image/png".to_string());
            let mime = if content_type.starts_with("image/") {
                content_type
            } else {
                "image/png".to_string()
            };
            let reader = DataReader::CreateDataReader(&stream).ok()?;
            let read_len = reader.LoadAsync(size as u32).ok()?.get().ok()? as usize;
            let mut bytes = vec![0u8; read_len];
            reader.ReadBytes(&mut bytes).ok()?;
            Some(format!("data:{mime};base64,{}", BASE64.encode(bytes)))
        });
        cache.set(key, loaded.clone());
        loaded
    };

    let active = !title.is_empty() || status_str == "Playing" || status_str == "Paused";

    MediaSnapshot {
        active,
        source_app,
        source_id,
        title,
        artist,
        album,
        status: status_str,
        position_sec,
        duration_sec,
        can_play_pause,
        can_next,
        can_previous,
        can_seek: duration_sec > 1.0,
        thumbnail_data_url,
    }
}

#[cfg(not(target_os = "windows"))]
pub fn query_media_snapshot(_cache: &mut ThumbnailCache) -> MediaSnapshot {
    MediaSnapshot::default()
}

#[cfg(target_os = "windows")]
pub fn control_media(action: &str, seek_sec: Option<f64>) -> Result<(), String> {
    use windows::Media::Control::GlobalSystemMediaTransportControlsSessionManager;

    let manager = GlobalSystemMediaTransportControlsSessionManager::RequestAsync()
        .map_err(|e| format!("GSMTC unavailable: {e}"))?
        .get()
        .map_err(|e| format!("GSMTC manager error: {e}"))?;

    let session = manager
        .GetCurrentSession()
        .map_err(|_| "No active Windows media session".to_string())?;

    match action {
        "play_pause" => {
            session
                .TryTogglePlayPauseAsync()
                .map_err(|e| format!("Play/Pause failed: {e}"))?
                .get()
                .map_err(|e| format!("Play/Pause error: {e}"))?;
        }
        "next" => {
            session
                .TrySkipNextAsync()
                .map_err(|e| format!("Next track failed: {e}"))?
                .get()
                .map_err(|e| format!("Next track error: {e}"))?;
        }
        "previous" => {
            session
                .TrySkipPreviousAsync()
                .map_err(|e| format!("Previous track failed: {e}"))?
                .get()
                .map_err(|e| format!("Previous track error: {e}"))?;
        }
        "seek" => {
            let sec = seek_sec.unwrap_or(0.0).max(0.0);
            let ticks = (sec * 10_000_000.0).round() as i64;
            session
                .TryChangePlaybackPositionAsync(ticks)
                .map_err(|e| format!("Seek failed: {e}"))?
                .get()
                .map_err(|e| format!("Seek error: {e}"))?;
        }
        other => return Err(format!("Unsupported media action: {other}")),
    }

    Ok(())
}

#[cfg(not(target_os = "windows"))]
pub fn control_media(_action: &str, _seek_sec: Option<f64>) -> Result<(), String> {
    Err("Media controls require Windows".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn source_name_mapping_recognizes_common_players() {
        assert_eq!(
            friendly_source_name("SpotifyAB.SpotifyMusic_zpdnekdrzrea0!Spotify"),
            "Spotify"
        );
        assert_eq!(friendly_source_name("chrome.exe"), "Chrome");
        assert_eq!(friendly_source_name("msedge.exe"), "Edge");
        assert_eq!(friendly_source_name("vlc.exe"), "VLC");
    }
}
