use crate::config::plugins_dir;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginAction {
    pub id: String,
    pub label: String,
    pub kind: String, // "url" | "activity"
    pub target: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginManifest {
    pub id: String,
    pub name: String,
    pub version: String,
    pub author: String,
    pub description: String,
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default)]
    pub badge: Option<String>,
    #[serde(default)]
    pub actions: Vec<PluginAction>,
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoadedPlugin {
    pub manifest: PluginManifest,
    pub path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginsSnapshot {
    pub plugins_dir: String,
    pub plugins: Vec<LoadedPlugin>,
}

pub fn ensure_plugins_dir_and_sample() -> PathBuf {
    let dir = plugins_dir();
    let _ = fs::create_dir_all(&dir);
    let readme = dir.join("README.txt");
    if !readme.exists() {
        let _ = fs::write(
            &readme,
            "PILLVYN Plugin Directory\r\nPlace each plugin in its own subfolder containing a valid plugin.json manifest.\r\n",
        );
    }
    dir
}

pub fn discover_plugins() -> PluginsSnapshot {
    let dir = ensure_plugins_dir_and_sample();
    let mut plugins = Vec::new();

    if let Ok(entries) = fs::read_dir(&dir) {
        for entry in entries.flatten() {
            let sub = entry.path();
            if !sub.is_dir() {
                continue;
            }
            let manifest_path = sub.join("plugin.json");
            if let Ok(raw) = fs::read_to_string(&manifest_path) {
                if let Ok(mut manifest) = serde_json::from_str::<PluginManifest>(&raw) {
                    if !manifest.id.trim().is_empty() && !manifest.name.trim().is_empty() {
                        manifest.actions.truncate(8);
                        plugins.push(LoadedPlugin {
                            manifest,
                            path: manifest_path.to_string_lossy().to_string(),
                        });
                    }
                }
            }
        }
    }

    plugins.sort_by(|a, b| a.manifest.name.cmp(&b.manifest.name));

    PluginsSnapshot {
        plugins_dir: dir.to_string_lossy().to_string(),
        plugins,
    }
}

pub fn create_example_plugin() -> Result<LoadedPlugin, String> {
    let dir = ensure_plugins_dir_and_sample().join("focus-pomodoro");
    fs::create_dir_all(&dir).map_err(|e| format!("Failed to create plugin folder: {e}"))?;
    let manifest_path = dir.join("plugin.json");
    let manifest = PluginManifest {
        id: "focus-pomodoro".into(),
        name: "Pomodoro Live Card".into(),
        version: "1.3.0".into(),
        author: "PILLVYN".into(),
        description:
            "Adds a one-click 25-minute Deep Work Live Activity action to the Plugins drawer."
                .into(),
        enabled: true,
        badge: Some("Built-in Sample".into()),
        actions: vec![PluginAction {
            id: "start-25".into(),
            label: "Pin Deep Work Activity (25m)".into(),
            kind: "activity".into(),
            target: "Deep Work Session — 25:00".into(),
        }],
    };
    let json = serde_json::to_string_pretty(&manifest)
        .map_err(|e| format!("Failed to serialize plugin manifest: {e}"))?;
    fs::write(&manifest_path, json).map_err(|e| format!("Failed to write plugin.json: {e}"))?;
    Ok(LoadedPlugin {
        manifest,
        path: manifest_path.to_string_lossy().to_string(),
    })
}

pub fn toggle_plugin_enabled(plugin_id: &str, enabled: bool) -> Result<PluginsSnapshot, String> {
    let dir = ensure_plugins_dir_and_sample();
    if let Ok(entries) = fs::read_dir(&dir) {
        for entry in entries.flatten() {
            let manifest_path = entry.path().join("plugin.json");
            if let Ok(raw) = fs::read_to_string(&manifest_path) {
                if let Ok(mut manifest) = serde_json::from_str::<PluginManifest>(&raw) {
                    if manifest.id == plugin_id {
                        manifest.enabled = enabled;
                        let json = serde_json::to_string_pretty(&manifest)
                            .map_err(|e| format!("Serialize error: {e}"))?;
                        fs::write(&manifest_path, json).map_err(|e| format!("Write error: {e}"))?;
                        return Ok(discover_plugins());
                    }
                }
            }
        }
    }
    Err(format!("Plugin '{plugin_id}' not found"))
}
