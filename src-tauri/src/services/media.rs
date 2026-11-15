use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};
use tokio::sync::Notify;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MediaSessionSummary {
    pub source_id: String,
    pub source_app: String,
    pub title: String,
    pub artist: String,
    pub status: String,
    pub is_active: bool,
}

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
    #[serde(default)]
    pub sessions: Vec<MediaSessionSummary>,
}

static PREFERRED_SOURCE_ID: Mutex<Option<String>> = Mutex::new(None);

pub fn select_media_session(source_id: Option<&str>) {
    if let Ok(mut guard) = PREFERRED_SOURCE_ID.lock() {
        *guard = source_id
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(ToOwned::to_owned);
    }
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
            sessions: Vec::new(),
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
    } else if lower.contains("opera") {
        "Opera".to_string()
    } else if lower.contains("yandex") {
        "Yandex Browser".to_string()
    } else if lower.contains("applemusic") {
        "Apple Music".to_string()
    } else if lower.contains("tidal") {
        "TIDAL".to_string()
    } else if lower.contains("amazonmusic") {
        "Amazon Music".to_string()
    } else if lower.contains("foobar") {
        "foobar2000".to_string()
    } else if lower.contains("musicbee") {
        "MusicBee".to_string()
    } else if lower.contains("winamp") {
        "Winamp".to_string()
    } else if lower.contains("mpv") {
        "mpv".to_string()
    } else if lower.contains("kodi") {
        "Kodi".to_string()
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
fn playback_status_str(
    session: &windows::Media::Control::GlobalSystemMediaTransportControlsSession,
) -> (String, bool, bool, bool) {
    use windows::Media::Control::GlobalSystemMediaTransportControlsSessionPlaybackStatus;

    match session.GetPlaybackInfo() {
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
    }
}

#[cfg(target_os = "windows")]
fn resolve_sessions(
    manager: &windows::Media::Control::GlobalSystemMediaTransportControlsSessionManager,
    preferred_source_id: Option<&str>,
) -> (
    Option<windows::Media::Control::GlobalSystemMediaTransportControlsSession>,
    Vec<MediaSessionSummary>,
) {
    let mut list = Vec::new();
    let mut picked: Option<windows::Media::Control::GlobalSystemMediaTransportControlsSession> =
        None;
    let current_id = manager
        .GetCurrentSession()
        .ok()
        .and_then(|s| s.SourceAppUserModelId().ok())
        .map(|s| s.to_string());

    if let Ok(sessions) = manager.GetSessions() {
        let count = sessions.Size().unwrap_or(0).min(8);
        let mut items = Vec::with_capacity(count as usize);
        for i in 0..count {
            if let Ok(sess) = sessions.GetAt(i) {
                let sid = sess
                    .SourceAppUserModelId()
                    .map(|s| s.to_string())
                    .unwrap_or_default();
                let (status, _, _, _) = playback_status_str(&sess);
                let (title, artist) = match sess.TryGetMediaPropertiesAsync() {
                    Ok(op) => match op.get() {
                        Ok(props) => (
                            props.Title().map(|s| s.to_string()).unwrap_or_default(),
                            props.Artist().map(|s| s.to_string()).unwrap_or_default(),
                        ),
                        Err(_) => (String::new(), String::new()),
                    },
                    Err(_) => (String::new(), String::new()),
                };
                items.push((sess, sid, status, title, artist));
            }
        }

        // Priority: explicit user selection (if present) -> playing current session -> any playing session -> current session -> first session
        let selected_idx = preferred_source_id
            .and_then(|pref| items.iter().position(|(_, sid, _, _, _)| sid == pref))
            .or_else(|| {
                items.iter().position(|(_, sid, status, _, _)| {
                    status == "Playing" && current_id.as_deref() == Some(sid.as_str())
                })
            })
            .or_else(|| {
                items
                    .iter()
                    .position(|(_, _, status, _, _)| status == "Playing")
            })
            .or_else(|| {
                items
                    .iter()
                    .position(|(_, sid, _, _, _)| current_id.as_deref() == Some(sid.as_str()))
            })
            .or_else(|| (!items.is_empty()).then_some(0));

        for (idx, (sess, sid, status, title, artist)) in items.into_iter().enumerate() {
            let is_active = selected_idx == Some(idx);
            if is_active {
                picked = Some(sess);
            }
            list.push(MediaSessionSummary {
                source_app: friendly_source_name(&sid),
                source_id: sid,
                title,
                artist,
                status,
                is_active,
            });
        }
    }

    if picked.is_none() {
        picked = manager.GetCurrentSession().ok();
    }
    (picked, list)
}

#[cfg(target_os = "windows")]
pub fn query_media_snapshot(cache: &mut ThumbnailCache) -> MediaSnapshot {
    use windows::Media::Control::GlobalSystemMediaTransportControlsSessionManager;
    use windows::Storage::Streams::DataReader;

    let Ok(mgr_op) = GlobalSystemMediaTransportControlsSessionManager::RequestAsync() else {
        return MediaSnapshot::default();
    };
    let Ok(manager) = mgr_op.get() else {
        return MediaSnapshot::default();
    };

    let preferred = PREFERRED_SOURCE_ID
        .lock()
        .ok()
        .and_then(|guard| guard.clone());
    let (maybe_session, sessions) = resolve_sessions(&manager, preferred.as_deref());
    let Some(session) = maybe_session else {
        return MediaSnapshot {
            sessions,
            ..MediaSnapshot::default()
        };
    };

    let source_id = session
        .SourceAppUserModelId()
        .map(|s| s.to_string())
        .unwrap_or_default();
    let source_app = friendly_source_name(&source_id);

    let (status_str, can_play_pause, can_next, can_previous) = playback_status_str(&session);

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
        sessions,
    }
}

#[cfg(not(target_os = "windows"))]
pub fn query_media_snapshot(_cache: &mut ThumbnailCache) -> MediaSnapshot {
    MediaSnapshot::default()
}

pub fn control_media(action: &str, seek_sec: Option<f64>) -> Result<(), String> {
    control_media_for_source(action, seek_sec, None)
}

#[cfg(target_os = "windows")]
pub fn control_media_for_source(
    action: &str,
    seek_sec: Option<f64>,
    source_id: Option<&str>,
) -> Result<(), String> {
    use windows::Media::Control::GlobalSystemMediaTransportControlsSessionManager;

    if action == "select_session" {
        select_media_session(source_id);
        return Ok(());
    }

    let manager = GlobalSystemMediaTransportControlsSessionManager::RequestAsync()
        .map_err(|e| format!("GSMTC unavailable: {e}"))?
        .get()
        .map_err(|e| format!("GSMTC manager error: {e}"))?;

    let target_pref = source_id.map(ToOwned::to_owned).or_else(|| {
        PREFERRED_SOURCE_ID
            .lock()
            .ok()
            .and_then(|guard| guard.clone())
    });
    let (session_opt, _) = resolve_sessions(&manager, target_pref.as_deref());
    let session = session_opt.ok_or_else(|| "No active Windows media session".to_string())?;

    match action {
        "play_pause" => {
            session
                .TryTogglePlayPauseAsync()
                .map_err(|e| format!("Play/Pause failed: {e}"))?
                .get()
                .map_err(|e| format!("Play/Pause error: {e}"))?;
        }
        "play" => {
            session
                .TryPlayAsync()
                .map_err(|e| format!("Play failed: {e}"))?
                .get()
                .map_err(|e| format!("Play error: {e}"))?;
        }
        "pause" => {
            session
                .TryPauseAsync()
                .map_err(|e| format!("Pause failed: {e}"))?
                .get()
                .map_err(|e| format!("Pause error: {e}"))?;
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
pub fn control_media_for_source(
    _action: &str,
    _seek_sec: Option<f64>,
    _source_id: Option<&str>,
) -> Result<(), String> {
    Err("Media controls require Windows".into())
}

/// Spawns a WinRT GSMTC event listener thread that wakes `notify` immediately whenever
/// media sessions change, playback state toggles, metadata updates, or timeline scrubs.
pub fn spawn_media_event_listener(notify: Arc<Notify>) {
    #[cfg(target_os = "windows")]
    {
        std::thread::Builder::new()
            .name("pillvyn-gsmtc-events".into())
            .spawn(move || {
                use std::sync::mpsc;
                use windows::Foundation::{EventRegistrationToken, TypedEventHandler};
                use windows::Media::Control::{
                    CurrentSessionChangedEventArgs, GlobalSystemMediaTransportControlsSession,
                    GlobalSystemMediaTransportControlsSessionManager,
                    MediaPropertiesChangedEventArgs, PlaybackInfoChangedEventArgs,
                    SessionsChangedEventArgs, TimelinePropertiesChangedEventArgs,
                };
                use windows::Win32::System::Com::{
                    CoInitializeEx, CoUninitialize, COINIT_MULTITHREADED,
                };

                struct ComScope(bool);
                impl Drop for ComScope {
                    fn drop(&mut self) {
                        if self.0 {
                            unsafe {
                                CoUninitialize();
                            }
                        }
                    }
                }

                let hr = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) };
                let _com = ComScope(hr.is_ok());

                let mut manager: Option<GlobalSystemMediaTransportControlsSessionManager> = None;
                for _ in 0..4 {
                    if let Ok(op) = GlobalSystemMediaTransportControlsSessionManager::RequestAsync()
                    {
                        if let Ok(mgr) = op.get() {
                            manager = Some(mgr);
                            break;
                        }
                    }
                    std::thread::sleep(std::time::Duration::from_millis(750));
                }
                let Some(manager) = manager else {
                    return;
                };

                let (rebind_tx, rebind_rx) = mpsc::channel::<()>();

                let n_cur = Arc::clone(&notify);
                let tx_cur = rebind_tx.clone();
                let _cur_tok =
                    manager.CurrentSessionChanged(&TypedEventHandler::<
                        GlobalSystemMediaTransportControlsSessionManager,
                        CurrentSessionChangedEventArgs,
                    >::new(move |_, _| {
                        n_cur.notify_one();
                        let _ = tx_cur.send(());
                        Ok(())
                    }));

                let n_sess = Arc::clone(&notify);
                let tx_sess = rebind_tx.clone();
                let _sess_tok = manager.SessionsChanged(&TypedEventHandler::<
                    GlobalSystemMediaTransportControlsSessionManager,
                    SessionsChangedEventArgs,
                >::new(move |_, _| {
                    n_sess.notify_one();
                    let _ = tx_sess.send(());
                    Ok(())
                }));

                type BoundSession = (
                    GlobalSystemMediaTransportControlsSession,
                    EventRegistrationToken,
                    EventRegistrationToken,
                    EventRegistrationToken,
                );
                let mut bound: Vec<BoundSession> = Vec::new();

                let _ = rebind_tx.send(());
                while rebind_rx.recv().is_ok() {
                    while rebind_rx.try_recv().is_ok() {}

                    for (sess, pb, mp, tl) in bound.drain(..) {
                        if pb.Value != 0 {
                            let _ = sess.RemovePlaybackInfoChanged(pb);
                        }
                        if mp.Value != 0 {
                            let _ = sess.RemoveMediaPropertiesChanged(mp);
                        }
                        if tl.Value != 0 {
                            let _ = sess.RemoveTimelinePropertiesChanged(tl);
                        }
                    }

                    if let Ok(sessions) = manager.GetSessions() {
                        let count = sessions.Size().unwrap_or(0).min(8);
                        for i in 0..count {
                            if let Ok(sess) = sessions.GetAt(i) {
                                let n1 = Arc::clone(&notify);
                                let pb = sess
                                    .PlaybackInfoChanged(&TypedEventHandler::<
                                        GlobalSystemMediaTransportControlsSession,
                                        PlaybackInfoChangedEventArgs,
                                    >::new(
                                        move |_, _| {
                                            n1.notify_one();
                                            Ok(())
                                        },
                                    ))
                                    .unwrap_or_default();

                                let n2 = Arc::clone(&notify);
                                let mp = sess
                                    .MediaPropertiesChanged(&TypedEventHandler::<
                                        GlobalSystemMediaTransportControlsSession,
                                        MediaPropertiesChangedEventArgs,
                                    >::new(
                                        move |_, _| {
                                            n2.notify_one();
                                            Ok(())
                                        },
                                    ))
                                    .unwrap_or_default();

                                let n3 = Arc::clone(&notify);
                                let tl = sess
                                    .TimelinePropertiesChanged(&TypedEventHandler::<
                                        GlobalSystemMediaTransportControlsSession,
                                        TimelinePropertiesChangedEventArgs,
                                    >::new(
                                        move |_, _| {
                                            n3.notify_one();
                                            Ok(())
                                        },
                                    ))
                                    .unwrap_or_default();

                                bound.push((sess, pb, mp, tl));
                            }
                        }
                    }
                    notify.notify_one();
                }
            })
            .ok();
    }

    #[cfg(not(target_os = "windows"))]
    let _ = notify;
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
