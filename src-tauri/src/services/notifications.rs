use crate::island::activities::now_ms;
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;

/// Classifies whether an incoming notification represents an SMS or messaging arrival
/// that warrants the Dynamic Island "ding" effect.
pub fn is_sms_notification(
    app_name: &str,
    title: &str,
    body: &str,
    kind_hint: Option<&str>,
) -> bool {
    if let Some(hint) = kind_hint.map(str::trim) {
        if hint.eq_ignore_ascii_case("sms")
            || hint.eq_ignore_ascii_case("message")
            || hint.eq_ignore_ascii_case("chat")
        {
            return true;
        }
    }

    let app_lc = app_name.to_ascii_lowercase();
    const SMS_APPS: &[&str] = &[
        "sms",
        "messages",
        "messaging",
        "phone link",
        "your phone",
        "whatsapp",
        "telegram",
        "signal",
        "viber",
        "imessage",
        "wechat",
        "messenger",
        "discord",
        "slack",
        "microsoft teams",
        "google messages",
        "android messages",
    ];
    if SMS_APPS.iter().any(|needle| app_lc.contains(needle)) {
        return true;
    }

    let title_lc = title.to_ascii_lowercase();
    let body_lc = body.to_ascii_lowercase();
    title_lc.contains("sms")
        || title_lc.contains("text message")
        || title_lc.contains("new message from")
        || body_lc.contains("sms ")
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum NotificationPriority {
    Critical,
    High,
    Normal,
    Low,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppNotification {
    pub id: String,
    pub app_name: String,
    pub title: String,
    pub body: String,
    pub priority: NotificationPriority,
    pub timestamp_ms: u64,
    pub read: bool,
    pub suppressed_by_dnd: bool,
    pub source: String,
    #[serde(default)]
    pub is_sms: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotificationsSnapshot {
    pub windows_listener_status: String,
    pub windows_listener_detail: String,
    pub unread_count: usize,
    pub items: Vec<AppNotification>,
}

pub struct NotificationHub {
    items: VecDeque<AppNotification>,
    windows_status: String,
    windows_detail: String,
    next_seq: u64,
}

impl Default for NotificationHub {
    fn default() -> Self {
        Self::new()
    }
}

pub struct NotificationRequest<'a> {
    pub app_name: String,
    pub title: String,
    pub body: String,
    pub priority: NotificationPriority,
    pub source: String,
    pub kind_hint: Option<&'a str>,
}

impl NotificationHub {
    pub fn new() -> Self {
        let (windows_status, windows_detail) = probe_windows_listener();
        Self {
            items: VecDeque::with_capacity(100),
            windows_status,
            windows_detail,
            next_seq: 1,
        }
    }

    pub fn push(
        &mut self,
        app_name: String,
        title: String,
        body: String,
        priority: NotificationPriority,
        source: String,
        dnd_active: bool,
    ) -> AppNotification {
        self.push_with_kind(
            NotificationRequest {
                app_name,
                title,
                body,
                priority,
                source,
                kind_hint: None,
            },
            dnd_active,
        )
    }

    pub fn push_with_kind(
        &mut self,
        req: NotificationRequest<'_>,
        dnd_active: bool,
    ) -> AppNotification {
        let suppressed = dnd_active && req.priority != NotificationPriority::Critical;
        let is_sms = is_sms_notification(&req.app_name, &req.title, &req.body, req.kind_hint);
        let id = format!("notif-{}-{}", now_ms(), self.next_seq);
        self.next_seq += 1;

        let item = AppNotification {
            id,
            app_name: req.app_name,
            title: req.title,
            body: req.body,
            priority: req.priority,
            timestamp_ms: now_ms(),
            read: false,
            suppressed_by_dnd: suppressed,
            source: req.source,
            is_sms,
        };
        self.items.push_front(item.clone());
        while self.items.len() > 100 {
            self.items.pop_back();
        }
        item
    }

    pub fn mark_read(&mut self, id: &str) -> bool {
        if let Some(item) = self.items.iter_mut().find(|n| n.id == id) {
            item.read = true;
            return true;
        }
        false
    }

    pub fn mark_all_read(&mut self) {
        for item in &mut self.items {
            item.read = true;
        }
    }

    pub fn dismiss(&mut self, id: &str) -> bool {
        let before = self.items.len();
        self.items.retain(|n| n.id != id);
        self.items.len() != before
    }

    pub fn clear_all(&mut self) {
        self.items.clear();
    }

    pub fn refresh_windows_listener(&mut self) {
        let (st, detail) = probe_windows_listener();
        self.windows_status = st;
        self.windows_detail = detail;
    }

    pub fn snapshot(&self) -> NotificationsSnapshot {
        let unread_count = self.items.iter().filter(|n| !n.read).count();
        NotificationsSnapshot {
            windows_listener_status: self.windows_status.clone(),
            windows_listener_detail: self.windows_detail.clone(),
            unread_count,
            items: self.items.iter().cloned().collect(),
        }
    }
}

#[cfg(target_os = "windows")]
fn probe_windows_listener() -> (String, String) {
    use windows::UI::Notifications::Management::{
        UserNotificationListener, UserNotificationListenerAccessStatus,
    };

    let Ok(listener) = UserNotificationListener::Current() else {
        return (
            "unavailable".to_string(),
            "Windows UserNotificationListener API is unavailable on this OS build.".to_string(),
        );
    };

    match listener.GetAccessStatus() {
        Ok(UserNotificationListenerAccessStatus::Allowed) => (
            "granted".to_string(),
            "Connected to Windows Action Center notifications.".to_string(),
        ),
        Ok(UserNotificationListenerAccessStatus::Denied) => (
            "denied".to_string(),
            "Access denied in Windows Privacy settings (Notifications).".to_string(),
        ),
        Ok(UserNotificationListenerAccessStatus::Unspecified) => (
            "requires_package_identity".to_string(),
            "Windows Action Center capture requires MSIX package identity; PILLVYN internal alerts, timers, downloads, and plugin notifications remain active.".to_string(),
        ),
        _ => (
            "requires_package_identity".to_string(),
            "Windows Action Center capture requires MSIX package identity; internal and plugin notifications remain active.".to_string(),
        ),
    }
}

#[cfg(not(target_os = "windows"))]
fn probe_windows_listener() -> (String, String) {
    (
        "unavailable".to_string(),
        "Windows Notification Listener is only supported on Windows 10/11.".to_string(),
    )
}

#[cfg(target_os = "windows")]
fn read_windows_toast_entries() -> Vec<(u32, String, String, String)> {
    use windows::UI::Notifications::Management::{
        UserNotificationListener, UserNotificationListenerAccessStatus,
    };
    use windows::UI::Notifications::{KnownNotificationBindings, NotificationKinds};

    let Ok(listener) = UserNotificationListener::Current() else {
        return Vec::new();
    };
    if !matches!(
        listener.GetAccessStatus(),
        Ok(UserNotificationListenerAccessStatus::Allowed)
    ) {
        return Vec::new();
    }
    let Ok(op) = listener.GetNotificationsAsync(NotificationKinds::Toast) else {
        return Vec::new();
    };
    let Ok(list) = op.get() else {
        return Vec::new();
    };
    let generic_id = KnownNotificationBindings::ToastGeneric().ok();

    let count = list.Size().unwrap_or(0);
    let mut out = Vec::with_capacity(count as usize);
    for idx in 0..count {
        let Ok(item) = list.GetAt(idx) else {
            continue;
        };
        let Ok(id) = item.Id() else {
            continue;
        };
        let app_name = item
            .AppInfo()
            .and_then(|info| info.DisplayInfo())
            .and_then(|disp| disp.DisplayName())
            .map(|s| s.to_string())
            .unwrap_or_else(|_| "Windows".to_string());

        let mut lines = Vec::new();
        if let (Some(gen_id), Ok(visual)) =
            (&generic_id, item.Notification().and_then(|n| n.Visual()))
        {
            if let Ok(binding) = visual.GetBinding(gen_id) {
                if let Ok(elements) = binding.GetTextElements() {
                    let text_count = elements.Size().unwrap_or(0);
                    for j in 0..text_count {
                        if let Ok(el) = elements.GetAt(j) {
                            if let Ok(txt) = el.Text() {
                                let clean = txt.to_string().trim().to_string();
                                if !clean.is_empty() {
                                    lines.push(clean);
                                }
                            }
                        }
                    }
                }
            }
        }
        let title = lines.first().cloned().unwrap_or_else(|| app_name.clone());
        let body = if lines.len() > 1 {
            lines[1..].join(" ")
        } else {
            String::new()
        };
        out.push((id, app_name, title, body));
    }
    out
}

/// Spawns a lightweight Windows Action Center watcher ONLY when `UserNotificationListener`
/// permission is `"granted"` (e.g. MSIX-packaged installs). Standard unpackaged EXEs exit
/// immediately with zero background overhead.
pub fn spawn_windows_notification_watcher(app: tauri::AppHandle) {
    #[cfg(target_os = "windows")]
    {
        if probe_windows_listener().0 != "granted" {
            return;
        }
        tauri::async_runtime::spawn(async move {
            use std::collections::HashSet;
            use tauri::Manager;

            let initial = tokio::task::spawn_blocking(read_windows_toast_entries)
                .await
                .unwrap_or_default();
            let mut seen: HashSet<u32> = initial.into_iter().map(|(id, _, _, _)| id).collect();

            loop {
                tokio::time::sleep(std::time::Duration::from_millis(1800)).await;
                let entries = tokio::task::spawn_blocking(read_windows_toast_entries)
                    .await
                    .unwrap_or_default();
                if seen.len() > 512 {
                    seen = entries.iter().map(|(id, _, _, _)| *id).collect();
                    continue;
                }
                for (id, app_name, title, body) in entries {
                    if !seen.insert(id) {
                        continue;
                    }
                    if let Some(state) = app.try_state::<crate::commands::AppState>() {
                        let _ = crate::commands::dispatch_notification_and_ding(
                            &app,
                            &state,
                            NotificationRequest {
                                app_name,
                                title,
                                body,
                                priority: NotificationPriority::Normal,
                                source: "windows".to_string(),
                                kind_hint: None,
                            },
                        );
                    }
                }
            }
        });
    }
    #[cfg(not(target_os = "windows"))]
    let _ = app;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dnd_suppresses_normal_notifications_but_allows_critical() {
        let mut hub = NotificationHub::new();
        let normal = hub.push(
            "System".into(),
            "Download Complete".into(),
            "archive.zip".into(),
            NotificationPriority::Normal,
            "downloads".into(),
            true,
        );
        let crit = hub.push(
            "Power".into(),
            "Critical Battery".into(),
            "5% remaining".into(),
            NotificationPriority::Critical,
            "battery".into(),
            true,
        );
        assert!(!normal.is_sms);
        assert!(normal.suppressed_by_dnd);
        assert!(!crit.suppressed_by_dnd);
        assert_eq!(hub.snapshot().unread_count, 2);
    }

    #[test]
    fn classifies_sms_and_messaging_alerts() {
        assert!(is_sms_notification(
            "Phone Link",
            "Alex",
            "On my way!",
            None
        ));
        assert!(is_sms_notification(
            "WhatsApp",
            "Mom",
            "Dinner tonight?",
            None
        ));
        assert!(is_sms_notification("Messages", "Code", "481203", None));
        assert!(is_sms_notification(
            "CustomApp",
            "Alert",
            "Body",
            Some("sms")
        ));
        assert!(!is_sms_notification(
            "Steam",
            "Update Ready",
            "1.2 GB",
            None
        ));
        assert!(!is_sms_notification(
            "Timer",
            "Focus Done",
            "Countdown reached 00:00",
            None
        ));
    }
}
