//! Plain data shared between the services (background threads) and the UI.
#![allow(dead_code)] // a few fields mirror the C# messages (feels, wind, humidity, …) without being shown

use gpui::RenderImage;
use std::sync::{Arc, OnceLock};

#[derive(Clone, Default, Debug)]
pub struct Source {
    pub id: String,
    pub app: String,
    pub browser: bool,
    pub title: String,
    pub artist: String,
    pub playing: bool,
    pub selected: bool,
}

#[derive(Clone, Default)]
pub struct MediaInfo {
    pub active: bool,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub app: String,
    pub browser: bool,
    pub playing: bool,
    pub can_prev: bool,
    pub can_next: bool,
    /// seconds, as of `pos_stamp` (unix ms)
    pub position: f64,
    pub duration: f64,
    pub pos_stamp: i64,
    pub art: Option<Arc<RenderImage>>,
    /// vivid colour picked from the album art (h, s, l) in 0..1
    pub tint: Option<(f32, f32, f32)>,
    pub sources: Vec<Source>,
}

impl MediaInfo {
    /// Current playback position, extrapolated while playing.
    pub fn pos(&self) -> f64 {
        if !self.active || self.duration <= 0.0 {
            return 0.0;
        }
        let extra = if self.playing { ((now_ms() - self.pos_stamp) as f64 / 1000.0).max(0.0) } else { 0.0 };
        (self.position + extra).min(self.duration)
    }
    pub fn key(&self) -> String {
        if self.active { format!("{}|{}|{}", self.app, self.title, self.artist) } else { String::new() }
    }
}

#[derive(Clone, Default, Debug)]
pub struct WeatherInfo {
    pub needs_city: bool,
    pub city: String,
    pub units: String,
    pub temp: Option<f64>,
    pub feels: f64,
    pub code: i64,
    pub wind: f64,
    pub humidity: i64,
    pub is_day: bool,
    pub hi: Option<f64>,
    pub lo: Option<f64>,
}

#[derive(Clone, Default, Debug)]
pub struct StatsInfo {
    pub cpu: f64,
    pub ram: f64,
    pub ram_used: f64,
    pub ram_total: f64,
}

#[derive(Clone, Default, Debug, PartialEq)]
pub struct PrivacyInfo {
    pub camera: bool,
    pub mic: bool,
    pub camera_app: String,
    pub mic_app: String,
}

#[derive(Clone, Debug, Default)]
pub struct LyricsResult {
    pub found: bool,
    pub synced: Option<String>,
    pub plain: Option<String>,
    pub instrumental: bool,
}

#[derive(Clone, Debug, Default)]
pub struct Diagnostics {
    pub total_mb: f64,
    pub private_mb: f64,
    pub windhawk_island: bool,
    pub fluent_flyout: bool,
    pub notifications: String,
}

/// Everything that flows from services / the system into the UI thread.
pub enum Event {
    Media(MediaInfo),
    Volume { level: f32, muted: bool, ours: bool },
    Levels(f32),
    Weather(WeatherInfo),
    Stats(StatsInfo),
    Privacy(PrivacyInfo),
    Notification { app: String, title: String, body: String },
    NotifyStatus(String),
    Lyrics { req: u64, result: LyricsResult },
    Occlusion { primary_fullscreen: bool },
    Session { locked: bool },
    DisplayPower { on: bool },
    DisplayChanged,
    /// pointer entered / left the island window (whose region is exactly the island shapes)
    IslandHover(bool),
    OpenPanel,
    ToggleIsland,
    Quit,
    RestoreQuit,
    Toast(String),
    /// result of "Save city" / "Use my location": settings patch for the weather section
    WeatherLocation(Option<serde_json::Value>, &'static str),
}

static TX: OnceLock<futures::channel::mpsc::UnboundedSender<Event>> = OnceLock::new();

pub fn init_channel() -> futures::channel::mpsc::UnboundedReceiver<Event> {
    let (tx, rx) = futures::channel::mpsc::unbounded();
    let _ = TX.set(tx);
    rx
}

/// Post an event to the UI thread from anywhere.
pub fn post(e: Event) {
    if let Some(tx) = TX.get() {
        let _ = tx.unbounded_send(e);
    }
}

pub fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

pub fn log(s: &str) {
    use std::io::Write;
    let dir = crate::settings::cache_dir();
    let _ = std::fs::create_dir_all(&dir);
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(dir.join("winisland.log")) {
        let t = crate::util::local_time();
        let _ = writeln!(f, "{:04}-{:02}-{:02} {:02}:{:02}:{:02} {}", t.year, t.month, t.day, t.hour, t.minute, t.second, s);
    }
}
