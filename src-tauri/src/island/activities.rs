use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ActivityKind {
    Media,
    Timer,
    Download,
    Gaming,
    Notification,
    Sms,
    SystemHud,
    Plugin,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LiveActivity {
    pub id: String,
    pub kind: ActivityKind,
    pub title: String,
    pub subtitle: String,
    pub accent: Option<String>,
    pub badge: Option<String>,
    pub progress: Option<f64>,
    pub priority: u8,
    pub updated_at_ms: u64,
    pub expires_at_ms: Option<u64>,
}

#[derive(Debug, Default)]
pub struct ActivityCoordinator {
    activities: HashMap<String, LiveActivity>,
}

pub fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

impl ActivityCoordinator {
    pub fn upsert(&mut self, mut activity: LiveActivity) -> bool {
        if activity.updated_at_ms == 0 {
            activity.updated_at_ms = now_ms();
        }
        let changed = match self.activities.get(&activity.id) {
            Some(existing) => {
                existing.title != activity.title
                    || existing.subtitle != activity.subtitle
                    || existing.progress != activity.progress
                    || existing.badge != activity.badge
                    || existing.priority != activity.priority
                    || existing.expires_at_ms != activity.expires_at_ms
            }
            None => true,
        };
        self.activities.insert(activity.id.clone(), activity);
        self.prune_expired();
        changed
    }

    pub fn remove(&mut self, id: &str) -> bool {
        self.activities.remove(id).is_some()
    }

    pub fn prune_expired(&mut self) -> bool {
        let now = now_ms();
        let before = self.activities.len();
        self.activities
            .retain(|_, item| item.expires_at_ms.is_none_or(|exp| exp > now));
        self.activities.len() != before
    }

    pub fn snapshot(&self) -> Vec<LiveActivity> {
        let now = now_ms();
        let mut items: Vec<LiveActivity> = self
            .activities
            .values()
            .filter(|item| item.expires_at_ms.is_none_or(|exp| exp > now))
            .cloned()
            .collect();
        items.sort_by(|a, b| {
            b.priority
                .cmp(&a.priority)
                .then_with(|| b.updated_at_ms.cmp(&a.updated_at_ms))
        });
        items.truncate(8);
        items
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn activity_ordering_and_expiration() {
        let mut coord = ActivityCoordinator::default();
        let t = now_ms();
        coord.upsert(LiveActivity {
            id: "media".into(),
            kind: ActivityKind::Media,
            title: "Track A".into(),
            subtitle: "Artist".into(),
            accent: None,
            badge: None,
            progress: Some(0.4),
            priority: 60,
            updated_at_ms: t,
            expires_at_ms: None,
        });
        coord.upsert(LiveActivity {
            id: "notif".into(),
            kind: ActivityKind::Notification,
            title: "Alert".into(),
            subtitle: "Details".into(),
            accent: None,
            badge: Some("NEW".into()),
            progress: None,
            priority: 95,
            updated_at_ms: t,
            expires_at_ms: Some(t + 5000),
        });
        coord.upsert(LiveActivity {
            id: "old".into(),
            kind: ActivityKind::SystemHud,
            title: "Expired".into(),
            subtitle: "".into(),
            accent: None,
            badge: None,
            progress: None,
            priority: 100,
            updated_at_ms: t - 5000,
            expires_at_ms: Some(t - 10),
        });

        let list = coord.snapshot();
        assert_eq!(list.len(), 2);
        assert_eq!(list[0].id, "notif");
        assert_eq!(list[1].id, "media");
    }
}
