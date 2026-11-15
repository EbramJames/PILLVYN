use crate::island::activities::now_ms;
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;

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
        let suppressed = dnd_active && priority != NotificationPriority::Critical;
        let id = format!("notif-{}-{}", now_ms(), self.next_seq);
        self.next_seq += 1;

        let item = AppNotification {
            id,
            app_name,
            title,
            body,
            priority,
            timestamp_ms: now_ms(),
            read: false,
            suppressed_by_dnd: suppressed,
            source,
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
        assert!(normal.suppressed_by_dnd);
        assert!(!crit.suppressed_by_dnd);
        assert_eq!(hub.snapshot().unread_count, 2);
    }
}
