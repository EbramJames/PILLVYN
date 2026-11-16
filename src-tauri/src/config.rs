use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::sync::Mutex;

static CONFIG_IO_LOCK: Mutex<()> = Mutex::new(());

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct PillvynConfig {
    pub version: u32,
    pub appearance: AppearanceConfig,
    pub modules: ModulesConfig,
    pub hotkeys: HotkeysConfig,
    pub privacy: PrivacyConfig,
    pub gaming: GamingConfig,
    pub widgets: WidgetsConfig,
    pub performance: PerformanceConfig,
    pub ai: AiConfig,
    pub quick_actions: Vec<QuickActionItem>,
    pub profiles: Vec<AppProfile>,
    pub autostart: bool,
    #[serde(default = "default_false")]
    pub autostart_initialized: bool,
}

fn default_false() -> bool {
    false
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AppearanceConfig {
    pub theme: String,
    pub accent_color: String,
    pub opacity: f64,
    pub scale: f64,
    pub compact_width: u32,
    pub top_offset: i32,
    pub x_offset: i32,
    pub monitor_name: Option<String>,
    pub reduced_motion: bool,
    pub hover_expand: bool,
    pub hover_peek: bool,
    pub minimal_idle: bool,
    pub click_outside_dismiss: bool,
    pub show_volume_hud: bool,
    pub show_emojis: bool,
    pub sms_ding_enabled: bool,
    pub sms_ding_sound: bool,
    pub auto_collapse_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ModulesConfig {
    pub music: bool,
    pub notifications: bool,
    pub system: bool,
    pub hardware: bool,
    pub gaming: bool,
    pub downloads: bool,
    pub clipboard: bool,
    pub network: bool,
    pub actions: bool,
    pub widgets: bool,
    pub plugins: bool,
    pub ai: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct HotkeysConfig {
    pub toggle_expand: String,
    pub toggle_visibility: String,
    pub toggle_dnd: String,
    pub clipboard_drawer: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct PrivacyConfig {
    pub clipboard_enabled: bool,
    pub clipboard_max_items: usize,
    pub clipboard_skip_sensitive: bool,
    pub dnd_mode: bool,
    pub focus_mode: bool,
    pub update_check_consent: bool,
    pub update_repo: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct GamingConfig {
    pub auto_detect: bool,
    pub suppress_notifications: bool,
    pub click_through_hud: bool,
    pub custom_processes: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct WidgetsConfig {
    pub weather_enabled: bool,
    pub weather_city: String,
    pub weather_lat: Option<f64>,
    pub weather_lon: Option<f64>,
    pub temperature_unit: String,
    pub clock_24h: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct PerformanceConfig {
    pub hardware_interval_ms: u64,
    pub network_interval_ms: u64,
    pub low_power_on_battery: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AiConfig {
    pub enabled: bool,
    pub provider: String,
    pub endpoint: String,
    pub model: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuickActionItem {
    pub id: String,
    pub label: String,
    pub icon: String,
    pub kind: String,
    pub target: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppProfile {
    pub id: String,
    pub name: String,
    pub process_name: String,
    pub dnd: bool,
    pub theme_override: Option<String>,
    pub active_tab: Option<String>,
}

impl Default for AppearanceConfig {
    fn default() -> Self {
        Self {
            theme: "obsidian".to_string(),
            accent_color: "#5b8cff".to_string(),
            opacity: 0.88,
            scale: 1.0,
            compact_width: 268,
            top_offset: 10,
            x_offset: 0,
            monitor_name: None,
            reduced_motion: false,
            hover_expand: false,
            hover_peek: true,
            minimal_idle: true,
            click_outside_dismiss: true,
            show_volume_hud: true,
            show_emojis: true,
            sms_ding_enabled: true,
            sms_ding_sound: true,
            auto_collapse_ms: 0,
        }
    }
}

impl Default for ModulesConfig {
    fn default() -> Self {
        Self {
            music: true,
            notifications: true,
            system: true,
            hardware: true,
            gaming: true,
            downloads: true,
            clipboard: true,
            network: true,
            actions: true,
            widgets: true,
            plugins: true,
            ai: false,
        }
    }
}

impl Default for HotkeysConfig {
    fn default() -> Self {
        Self {
            toggle_expand: "Alt+Shift+Space".to_string(),
            toggle_visibility: "Ctrl+Alt+H".to_string(),
            toggle_dnd: "Ctrl+Alt+D".to_string(),
            clipboard_drawer: "Alt+Shift+V".to_string(),
        }
    }
}

impl Default for PrivacyConfig {
    fn default() -> Self {
        Self {
            clipboard_enabled: true,
            clipboard_max_items: 25,
            clipboard_skip_sensitive: true,
            dnd_mode: false,
            focus_mode: false,
            update_check_consent: false,
            update_repo: "EbramJames/PILLVYN".to_string(),
        }
    }
}

impl Default for GamingConfig {
    fn default() -> Self {
        Self {
            auto_detect: true,
            suppress_notifications: true,
            click_through_hud: false,
            custom_processes: Vec::new(),
        }
    }
}

impl Default for WidgetsConfig {
    fn default() -> Self {
        Self {
            weather_enabled: true,
            weather_city: "Auto (Local)".to_string(),
            weather_lat: None,
            weather_lon: None,
            temperature_unit: "C".to_string(),
            clock_24h: true,
        }
    }
}

impl Default for PerformanceConfig {
    fn default() -> Self {
        Self {
            hardware_interval_ms: 2000,
            network_interval_ms: 1500,
            low_power_on_battery: true,
        }
    }
}

impl Default for AiConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            provider: "ollama".to_string(),
            endpoint: "http://127.0.0.1:11434".to_string(),
            model: "llama3.2".to_string(),
        }
    }
}

impl Default for PillvynConfig {
    fn default() -> Self {
        Self {
            version: 2,
            appearance: AppearanceConfig::default(),
            modules: ModulesConfig::default(),
            hotkeys: HotkeysConfig::default(),
            privacy: PrivacyConfig::default(),
            gaming: GamingConfig::default(),
            widgets: WidgetsConfig::default(),
            performance: PerformanceConfig::default(),
            ai: AiConfig::default(),
            quick_actions: default_quick_actions(),
            profiles: default_profiles(),
            autostart: true,
            autostart_initialized: true,
        }
    }
}

pub fn default_quick_actions() -> Vec<QuickActionItem> {
    vec![
        QuickActionItem {
            id: "snip".into(),
            label: "Screenshot".into(),
            icon: "camera".into(),
            kind: "uri".into(),
            target: "ms-screenclip:".into(),
        },
        QuickActionItem {
            id: "terminal".into(),
            label: "Terminal".into(),
            icon: "terminal".into(),
            kind: "system_tool".into(),
            target: "wt".into(),
        },
        QuickActionItem {
            id: "taskmgr".into(),
            label: "Task Manager".into(),
            icon: "activity".into(),
            kind: "system_tool".into(),
            target: "taskmgr".into(),
        },
        QuickActionItem {
            id: "settings".into(),
            label: "Win Settings".into(),
            icon: "sliders".into(),
            kind: "uri".into(),
            target: "ms-settings:".into(),
        },
        QuickActionItem {
            id: "sound".into(),
            label: "Sound Mixer".into(),
            icon: "volume".into(),
            kind: "uri".into(),
            target: "ms-settings:apps-volume".into(),
        },
        QuickActionItem {
            id: "calc".into(),
            label: "Calculator".into(),
            icon: "hash".into(),
            kind: "system_tool".into(),
            target: "calc".into(),
        },
    ]
}

pub fn default_profiles() -> Vec<AppProfile> {
    vec![
        AppProfile {
            id: "gaming-focus".into(),
            name: "Gaming Focus".into(),
            process_name: "steam.exe".into(),
            dnd: true,
            theme_override: Some("crimson".into()),
            active_tab: Some("gaming".into()),
        },
        AppProfile {
            id: "studio-obs".into(),
            name: "Broadcast Studio".into(),
            process_name: "obs64.exe".into(),
            dnd: true,
            theme_override: Some("obsidian".into()),
            active_tab: Some("system".into()),
        },
        AppProfile {
            id: "dev-code".into(),
            name: "Deep Coding".into(),
            process_name: "code.exe".into(),
            dnd: false,
            theme_override: None,
            active_tab: Some("music".into()),
        },
    ]
}

impl PillvynConfig {
    pub fn sanitize(&mut self) {
        const THEMES: [&str; 5] = ["obsidian", "aurora", "midnight", "crimson", "frosted"];
        if !THEMES.contains(&self.appearance.theme.as_str()) {
            self.appearance.theme = "obsidian".to_string();
        }
        if !is_valid_hex_color(&self.appearance.accent_color) {
            self.appearance.accent_color = "#5b8cff".to_string();
        }
        self.appearance.opacity = self.appearance.opacity.clamp(0.45, 0.98);
        self.appearance.scale = self.appearance.scale.clamp(0.8, 1.35);
        self.appearance.compact_width = self.appearance.compact_width.clamp(210, 380);
        self.appearance.top_offset = self.appearance.top_offset.clamp(0, 2400);
        self.appearance.x_offset = self.appearance.x_offset.clamp(-3000, 3000);
        if self.appearance.auto_collapse_ms > 0 {
            self.appearance.auto_collapse_ms = self.appearance.auto_collapse_ms.clamp(2000, 30000);
        }

        if self.version < 2 {
            self.version = 2;
        }
        self.privacy.clipboard_max_items = self.privacy.clipboard_max_items.clamp(5, 100);
        if self.privacy.update_repo.trim().is_empty()
            || self
                .privacy
                .update_repo
                .trim()
                .eq_ignore_ascii_case("pillvyn/pillvyn")
        {
            self.privacy.update_repo = "EbramJames/PILLVYN".to_string();
        }

        if self.widgets.temperature_unit != "C" && self.widgets.temperature_unit != "F" {
            self.widgets.temperature_unit = "C".to_string();
        }

        self.performance.hardware_interval_ms =
            self.performance.hardware_interval_ms.clamp(1000, 10000);
        self.performance.network_interval_ms =
            self.performance.network_interval_ms.clamp(1000, 10000);

        if !["ollama", "openai_compatible"].contains(&self.ai.provider.as_str()) {
            self.ai.provider = "ollama".to_string();
        }
        if self.ai.endpoint.trim().is_empty() {
            self.ai.endpoint = "http://127.0.0.1:11434".to_string();
        }
        if self.ai.model.trim().is_empty() {
            self.ai.model = "llama3.2".to_string();
        }
        self.quick_actions.truncate(16);
        self.profiles.truncate(24);
    }
}

pub fn is_valid_hex_color(value: &str) -> bool {
    let bytes = value.as_bytes();
    bytes.len() == 7 && bytes[0] == b'#' && bytes[1..].iter().all(|b| b.is_ascii_hexdigit())
}

pub fn app_data_dir() -> PathBuf {
    if let Some(proj) = ProjectDirs::from("com", "pillvyn", "PILLVYN") {
        return proj.config_dir().to_path_buf();
    }
    PathBuf::from(".")
}

pub fn plugins_dir() -> PathBuf {
    app_data_dir().join("plugins")
}

pub fn config_file_path() -> PathBuf {
    app_data_dir().join("config.json")
}

pub fn load_config() -> PillvynConfig {
    let path = config_file_path();
    let (mut cfg, loaded_from_disk) = match fs::read_to_string(&path) {
        Ok(raw) => match serde_json::from_str::<PillvynConfig>(&raw) {
            Ok(parsed) => (parsed, true),
            Err(_) => (PillvynConfig::default(), false),
        },
        Err(_) => (PillvynConfig::default(), false),
    };
    let mut needs_migration = loaded_from_disk
        && (cfg.version < 2
            || cfg
                .privacy
                .update_repo
                .trim()
                .eq_ignore_ascii_case("pillvyn/pillvyn"));
    if loaded_from_disk && !cfg.autostart_initialized {
        cfg.autostart = true;
        cfg.autostart_initialized = true;
        needs_migration = true;
    }
    cfg.sanitize();
    if needs_migration {
        let _ = save_config(&cfg);
    }
    cfg
}

pub fn save_config(cfg: &PillvynConfig) -> Result<(), String> {
    let _guard = CONFIG_IO_LOCK
        .lock()
        .map_err(|_| "Config save lock poisoned".to_string())?;
    let mut clean = cfg.clone();
    clean.sanitize();
    let dir = app_data_dir();
    fs::create_dir_all(&dir).map_err(|e| format!("Failed to create config dir: {e}"))?;
    let path = config_file_path();
    let tmp = dir.join(format!("config.{}.tmp", std::process::id()));
    let json = serde_json::to_string_pretty(&clean)
        .map_err(|e| format!("Failed to serialize config: {e}"))?;
    fs::write(&tmp, json).map_err(|e| format!("Failed to write temp config: {e}"))?;
    fs::rename(&tmp, &path).map_err(|e| format!("Failed to commit config: {e}"))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_sanitize_clamps_invalid_values() {
        let mut cfg = PillvynConfig::default();
        cfg.appearance.theme = "unknown_theme".into();
        cfg.appearance.accent_color = "javascript:alert(1)".into();
        cfg.appearance.opacity = 4.5;
        cfg.appearance.scale = 0.1;
        cfg.performance.hardware_interval_ms = 50;
        cfg.sanitize();

        assert_eq!(cfg.appearance.theme, "obsidian");
        assert_eq!(cfg.appearance.accent_color, "#5b8cff");
        assert!((cfg.appearance.opacity - 0.98).abs() < f64::EPSILON);
        assert!((cfg.appearance.scale - 0.8).abs() < f64::EPSILON);
        assert_eq!(cfg.performance.hardware_interval_ms, 1000);
    }

    #[test]
    fn hex_color_validation_works() {
        assert!(is_valid_hex_color("#5b8cff"));
        assert!(is_valid_hex_color("#00FFAA"));
        assert!(!is_valid_hex_color("5b8cff"));
        assert!(!is_valid_hex_color("#zzz"));
        assert!(!is_valid_hex_color("#1234567"));
    }

    #[test]
    fn migrates_v1_repo_and_defaults_new_appearance_flags() {
        let legacy = r#"{
            "version": 1,
            "appearance": { "theme": "obsidian" },
            "privacy": { "update_repo": "pillvyn/pillvyn" }
        }"#;
        let mut cfg: PillvynConfig = serde_json::from_str(legacy).expect("valid legacy json");
        cfg.sanitize();
        assert_eq!(cfg.version, 2);
        assert_eq!(cfg.privacy.update_repo, "EbramJames/PILLVYN");
        assert!(cfg.appearance.minimal_idle);
        assert!(cfg.appearance.hover_peek);
        assert!(cfg.appearance.click_outside_dismiss);
        assert!(cfg.appearance.show_volume_hud);
        assert!(cfg.appearance.show_emojis);
        assert!(cfg.appearance.sms_ding_enabled);
        assert!(cfg.appearance.sms_ding_sound);
        assert!(!cfg.autostart_initialized);
    }

    #[test]
    fn autostart_defaults_on_and_preserves_explicit_opt_out() {
        let fresh = PillvynConfig::default();
        assert!(fresh.autostart);
        assert!(fresh.autostart_initialized);

        // Simulated upgrade from <= v1.4.0 config missing `autostart_initialized`
        let old_json = r#"{"version":2,"autostart":false}"#;
        let parsed_old: PillvynConfig = serde_json::from_str(old_json).expect("valid v1.4 json");
        assert!(!parsed_old.autostart_initialized);

        // User who explicitly toggled startup OFF retains their preference across reloads
        let opt_out_json = r#"{"version":2,"autostart":false,"autostart_initialized":true}"#;
        let parsed_opt_out: PillvynConfig =
            serde_json::from_str(opt_out_json).expect("valid opt-out json");
        assert!(!parsed_opt_out.autostart);
        assert!(parsed_opt_out.autostart_initialized);
    }
}
