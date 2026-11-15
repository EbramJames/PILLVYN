use crate::config::{app_data_dir, WidgetsConfig};
use crate::island::activities::now_ms;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimerEntry {
    pub id: String,
    pub label: String,
    pub total_seconds: u64,
    pub remaining_seconds: u64,
    pub running: bool,
    pub completed: bool,
    pub last_tick_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WeatherSnapshot {
    pub available: bool,
    pub city: String,
    pub temperature_c: Option<f64>,
    pub feels_like_c: Option<f64>,
    pub condition: String,
    pub weather_code: Option<u32>,
    pub wind_kmh: Option<f64>,
    pub humidity_percent: Option<u32>,
    pub updated_at_ms: Option<u64>,
    pub cached: bool,
    pub status_note: String,
}

impl Default for WeatherSnapshot {
    fn default() -> Self {
        Self {
            available: false,
            city: "Unconfigured".to_string(),
            temperature_c: None,
            feels_like_c: None,
            condition: "Offline".to_string(),
            weather_code: None,
            wind_kmh: None,
            humidity_percent: None,
            updated_at_ms: None,
            cached: false,
            status_note: "Enter a city in Settings or click Refresh Weather.".to_string(),
        }
    }
}

pub struct WidgetEngine {
    pub timers: Vec<TimerEntry>,
    pub stopwatch_running: bool,
    pub stopwatch_elapsed_ms: u64,
    stopwatch_last_ms: u64,
    pub weather: WeatherSnapshot,
    next_id: u64,
}

impl Default for WidgetEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl WidgetEngine {
    pub fn new() -> Self {
        let cached = load_cached_weather().unwrap_or_default();
        Self {
            timers: Vec::new(),
            stopwatch_running: false,
            stopwatch_elapsed_ms: 0,
            stopwatch_last_ms: now_ms(),
            weather: cached,
            next_id: 1,
        }
    }

    pub fn add_timer(&mut self, label: String, seconds: u64) -> TimerEntry {
        let clean_label = if label.trim().is_empty() {
            "Focus Timer".to_string()
        } else {
            label.trim().chars().take(36).collect()
        };
        let total = seconds.clamp(5, 86_400);
        let id = format!("tm-{}", self.next_id);
        self.next_id += 1;
        let entry = TimerEntry {
            id,
            label: clean_label,
            total_seconds: total,
            remaining_seconds: total,
            running: true,
            completed: false,
            last_tick_ms: now_ms(),
        };
        self.timers.push(entry.clone());
        if self.timers.len() > 6 {
            self.timers.remove(0);
        }
        entry
    }

    pub fn toggle_timer(&mut self, id: &str) -> bool {
        let now = now_ms();
        if let Some(t) = self.timers.iter_mut().find(|t| t.id == id) {
            if t.completed {
                t.remaining_seconds = t.total_seconds;
                t.completed = false;
                t.running = true;
            } else {
                t.running = !t.running;
            }
            t.last_tick_ms = now;
            return true;
        }
        false
    }

    pub fn remove_timer(&mut self, id: &str) -> bool {
        let before = self.timers.len();
        self.timers.retain(|t| t.id != id);
        self.timers.len() != before
    }

    pub fn control_stopwatch(&mut self, action: &str) {
        let now = now_ms();
        match action {
            "start" => {
                if !self.stopwatch_running {
                    self.stopwatch_running = true;
                    self.stopwatch_last_ms = now;
                }
            }
            "pause" => {
                if self.stopwatch_running {
                    self.stopwatch_elapsed_ms += now.saturating_sub(self.stopwatch_last_ms);
                    self.stopwatch_running = false;
                }
            }
            "reset" => {
                self.stopwatch_running = false;
                self.stopwatch_elapsed_ms = 0;
                self.stopwatch_last_ms = now;
            }
            _ => {}
        }
    }

    /// Advances active timers and returns any newly completed timer labels.
    pub fn tick(&mut self) -> Vec<TimerEntry> {
        let now = now_ms();
        let mut newly_finished = Vec::new();

        for t in &mut self.timers {
            if !t.running || t.completed {
                continue;
            }
            let elapsed_sec = now.saturating_sub(t.last_tick_ms) / 1000;
            if elapsed_sec >= 1 {
                t.last_tick_ms += elapsed_sec * 1000;
                if elapsed_sec >= t.remaining_seconds {
                    t.remaining_seconds = 0;
                    t.running = false;
                    t.completed = true;
                    newly_finished.push(t.clone());
                } else {
                    t.remaining_seconds -= elapsed_sec;
                }
            }
        }

        if self.stopwatch_running {
            self.stopwatch_elapsed_ms += now.saturating_sub(self.stopwatch_last_ms);
            self.stopwatch_last_ms = now;
        }

        newly_finished
    }
}

fn weather_cache_path() -> PathBuf {
    app_data_dir().join("weather-cache.json")
}

fn load_cached_weather() -> Option<WeatherSnapshot> {
    let raw = fs::read_to_string(weather_cache_path()).ok()?;
    let mut snap: WeatherSnapshot = serde_json::from_str(&raw).ok()?;
    snap.cached = true;
    Some(snap)
}

fn save_cached_weather(snap: &WeatherSnapshot) {
    let dir = app_data_dir();
    if fs::create_dir_all(&dir).is_ok() {
        if let Ok(json) = serde_json::to_string_pretty(snap) {
            let _ = fs::write(weather_cache_path(), json);
        }
    }
}

#[derive(Deserialize)]
struct GeoResponse {
    results: Option<Vec<GeoItem>>,
}

#[derive(Deserialize)]
struct GeoItem {
    name: String,
    country: Option<String>,
    latitude: f64,
    longitude: f64,
}

#[derive(Deserialize)]
struct OpenMeteoResponse {
    current: Option<OpenMeteoCurrent>,
}

#[derive(Deserialize)]
struct OpenMeteoCurrent {
    temperature_2m: f64,
    apparent_temperature: Option<f64>,
    relative_humidity_2m: Option<u32>,
    weather_code: Option<u32>,
    wind_speed_10m: Option<f64>,
}

pub async fn fetch_live_weather(cfg: &WidgetsConfig) -> Result<WeatherSnapshot, String> {
    if !cfg.weather_enabled {
        return Err("Weather widget is disabled in Settings".into());
    }

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(6))
        .user_agent("PILLVYN/1.0")
        .build()
        .map_err(|e| format!("HTTP client error: {e}"))?;

    let (lat, lon, resolved_city) = match (cfg.weather_lat, cfg.weather_lon) {
        (Some(la), Some(lo)) => (la, lo, cfg.weather_city.clone()),
        _ => {
            let query = cfg.weather_city.trim();
            if query.is_empty() || query == "Auto (Local)" {
                // Free keyless IP-geo fallback only if user selected Auto, else geocode explicit city
                let ip_resp: serde_json::Value = client
                    .get("https://get.geojs.io/v1/ip/geo.json")
                    .send()
                    .await
                    .map_err(|e| format!("Offline or network error: {e}"))?
                    .json()
                    .await
                    .map_err(|e| format!("Location parse error: {e}"))?;
                let la = ip_resp["latitude"]
                    .as_str()
                    .and_then(|s| s.parse::<f64>().ok())
                    .ok_or("Could not resolve local coordinates; set a city in Settings")?;
                let lo = ip_resp["longitude"]
                    .as_str()
                    .and_then(|s| s.parse::<f64>().ok())
                    .ok_or("Could not resolve local longitude; set a city in Settings")?;
                let city = ip_resp["city"].as_str().unwrap_or("Local").to_string();
                (la, lo, city)
            } else {
                let geo_url = format!(
                    "https://geocoding-api.open-meteo.com/v1/search?name={}&count=1&language=en&format=json",
                    url_encode(query)
                );
                let geo: GeoResponse = client
                    .get(geo_url)
                    .send()
                    .await
                    .map_err(|e| format!("Geocoding unavailable (offline?): {e}"))?
                    .json()
                    .await
                    .map_err(|e| format!("Geocoding parse error: {e}"))?;
                let first = geo
                    .results
                    .and_then(|mut r| r.drain(..).next())
                    .ok_or_else(|| format!("City '{query}' not found"))?;
                let label = match first.country {
                    Some(c) if !c.is_empty() => format!("{}, {c}", first.name),
                    _ => first.name,
                };
                (first.latitude, first.longitude, label)
            }
        }
    };

    let url = format!(
        "https://api.open-meteo.com/v1/forecast?latitude={lat}&longitude={lon}&current=temperature_2m,apparent_temperature,relative_humidity_2m,weather_code,wind_speed_10m"
    );
    let data: OpenMeteoResponse = client
        .get(url)
        .send()
        .await
        .map_err(|e| format!("Weather service unavailable (offline?): {e}"))?
        .json()
        .await
        .map_err(|e| format!("Weather parse error: {e}"))?;

    let cur = data
        .current
        .ok_or("Weather data missing current conditions")?;
    let code = cur.weather_code;
    let snap = WeatherSnapshot {
        available: true,
        city: resolved_city,
        temperature_c: Some(cur.temperature_2m),
        feels_like_c: cur.apparent_temperature,
        condition: wmo_code_to_label(code.unwrap_or(0)).to_string(),
        weather_code: code,
        wind_kmh: cur.wind_speed_10m,
        humidity_percent: cur.relative_humidity_2m,
        updated_at_ms: Some(now_ms()),
        cached: false,
        status_note: "Live Open-Meteo telemetry".to_string(),
    };
    save_cached_weather(&snap);
    Ok(snap)
}

fn wmo_code_to_label(code: u32) -> &'static str {
    match code {
        0 => "Clear Sky",
        1 | 2 => "Partly Cloudy",
        3 => "Overcast",
        45 | 48 => "Fog",
        51..=57 => "Drizzle",
        61..=67 => "Rain",
        71..=77 => "Snow",
        80..=82 => "Rain Showers",
        85 | 86 => "Snow Showers",
        95..=99 => "Thunderstorm",
        _ => "Variable",
    }
}

fn url_encode(input: &str) -> String {
    let mut out = String::new();
    for b in input.as_bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(*b as char);
            }
            b' ' => out.push_str("%20"),
            other => out.push_str(&format!("%{other:02X}")),
        }
    }
    out
}
