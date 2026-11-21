//! The Hub: all live state (settings, media, volume, weather, …) for one island. The real island has one fed
//! by the Windows services; the settings preview and `--demo` runs use one fed by demo data.

use crate::demo::{self, DemoPlayer};
use crate::model::*;
use crate::settings::{self, Settings};
use gpui::{App, AppContext, Context, EventEmitter, Task};
use serde_json::Value;
use std::time::{Duration, Instant};

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum LyrState {
    None,
    Loading,
    Synced,
    Plain,
    Instrumental,
    Missing,
}

pub struct Lyrics {
    pub key: String,
    pub req: u64,
    pub state: LyrState,
    pub lines: Vec<(f64, String)>,
    pub plain: String,
}

/// Show each line a hair early, like karaoke machines do.
pub const LEAD: f64 = 0.2;

impl Lyrics {
    pub fn index(&self, pos: f64) -> isize {
        let (mut lo, mut hi, mut i) = (0isize, self.lines.len() as isize - 1, -1isize);
        while lo <= hi {
            let mid = (lo + hi) / 2;
            if self.lines[mid as usize].0 <= pos {
                i = mid;
                lo = mid + 1;
            } else {
                hi = mid - 1;
            }
        }
        i
    }
    /// Fill fraction (0..1) of line `i` at `pos`, at roughly singing speed.
    pub fn fill(&self, i: isize, pos: f64) -> f32 {
        if i < 0 {
            return 0.0;
        }
        let i = i as usize;
        let Some(cur) = self.lines.get(i) else { return 0.0 };
        let gap = self.lines.get(i + 1).map(|n| n.0).unwrap_or(cur.0 + 5.0) - cur.0;
        let len = (gap * 0.92).min((cur.1.chars().count() as f64 * 0.085).max(1.4)).max(0.4);
        ((pos - cur.0) / len).clamp(0.0, 1.0) as f32
    }
}

#[derive(Clone, Copy, Default, Debug, PartialEq)]
pub struct Timer {
    pub end: i64,
    pub total: i64,
    pub paused: i64,
}

impl Timer {
    pub fn running(&self) -> bool {
        self.total > 0 && (self.paused > 0 || self.end > now_ms())
    }
    pub fn is_paused(&self) -> bool {
        self.paused > 0
    }
    pub fn remaining(&self) -> i64 {
        if self.paused > 0 {
            self.paused
        } else {
            (self.end - now_ms()).max(0)
        }
    }
    pub fn progress(&self) -> f32 {
        if self.total > 0 {
            1.0 - self.remaining() as f32 / self.total as f32
        } else {
            0.0
        }
    }
    pub fn label(&self) -> String {
        let s = (self.remaining() + 999) / 1000;
        format!("{}:{:02}", s / 60, s % 60)
    }
    pub fn finished_now(&self) -> bool {
        self.total > 0 && self.paused == 0 && self.end > 0 && self.end <= now_ms()
    }
}

#[derive(Clone, Debug)]
pub enum HubEvent {
    /// volume changed outside of WinIsland (force = show even if the HUD setting is off)
    Volume { force: bool },
    Track,
    Notification { app: String, title: String, body: String },
    PrivacyNew { what: &'static str, app: String, camera: bool },
    TimerDone,
    Scene(String),
    SettingsChanged(Value),
    LyricsReady,
    Toast(String),
}

pub struct Hub {
    pub demo: Option<DemoPlayer>,
    pub preview: bool,
    pub settings: Settings,
    pub media: MediaInfo,
    pub volume: f32,
    pub muted: bool,
    pub weather: Option<WeatherInfo>,
    pub stats: Option<StatsInfo>,
    pub privacy: PrivacyInfo,
    pub level: f32,
    pub level_at: Instant,
    pub lyrics: Lyrics,
    pub timer: Timer,
    pub visible: bool,
    pub diag: Diagnostics,
    metering: bool,
    sampler: crate::stats::Sampler,
    stats_subs: u8,
    stats_task: Option<Task<()>>,
    tick_task: Option<Task<()>>,
    _minute_task: Option<Task<()>>,
    demo_level_task: Option<Task<()>>,
    last_title: Option<String>,
    media_seen: bool,
}

impl EventEmitter<HubEvent> for Hub {}

fn timer_path() -> std::path::PathBuf {
    settings::cache_dir().join("timer.json")
}

impl Hub {
    pub fn new(settings: Settings, demo: Option<DemoPlayer>, preview: bool, cx: &mut Context<Self>) -> Self {
        let timer = if demo.is_none() {
            std::fs::read_to_string(timer_path())
                .ok()
                .and_then(|t| serde_json::from_str::<Value>(&t).ok())
                .map(|v| Timer { end: v["end"].as_i64().unwrap_or(0), total: v["total"].as_i64().unwrap_or(0), paused: v["paused"].as_i64().unwrap_or(0) })
                .unwrap_or_default()
        } else {
            Timer::default()
        };
        let mut hub = Hub {
            media: demo.as_ref().map(|d| d.media()).unwrap_or_default(),
            weather: demo.as_ref().map(|_| demo::weather()),
            stats: demo.as_ref().map(|_| StatsInfo { cpu: 9.4, ram: 46.0, ram_used: 14.7, ram_total: 32.0 }),
            demo,
            preview,
            settings,
            volume: 0.62,
            muted: false,
            privacy: PrivacyInfo::default(),
            level: 0.0,
            level_at: Instant::now(),
            lyrics: Lyrics { key: String::new(), req: 0, state: LyrState::None, lines: vec![], plain: String::new() },
            timer,
            visible: true,
            diag: Diagnostics::default(),
            metering: false,
            sampler: Default::default(),
            stats_subs: 0,
            stats_task: None,
            tick_task: None,
            _minute_task: None,
            demo_level_task: None,
            last_title: None,
            media_seen: false,
        };
        hub.last_title = if hub.media.active { Some(hub.media.title.clone()) } else { None };
        hub.media_seen = hub.demo.is_some();
        // repaint the idle clock at every minute boundary (one timer per minute, no polling)
        hub._minute_task = Some(cx.spawn(async move |this, cx| loop {
            cx.background_executor().timer(Duration::from_millis(crate::util::ms_to_next_minute())).await;
            if this.update(cx, |_, cx| cx.notify()).is_err() {
                break;
            }
        }));
        hub.update_tick(cx);
        if hub.demo.is_some() {
            cx.spawn(async move |this, cx| {
                let _ = this.update(cx, |h, cx| {
                    if h.media.playing {
                        h.ensure_lyrics(cx);
                    }
                });
            })
            .detach();
        }
        hub
    }

    pub fn i(&self, k: &str) -> bool {
        self.settings.island(k)
    }

    pub fn ios(&self) -> bool {
        self.settings.is_island_style()
    }

    pub fn lyrics_on(&self) -> bool {
        self.settings.get("island.lyrics").and_then(Value::as_bool) != Some(false) && self.i("media")
    }

    /// The island sings along: synced lyrics for the current track and the option is on.
    pub fn notch_lyrics(&self) -> bool {
        self.lyrics_on()
            && self.settings.get("island.lyricsNotch").and_then(Value::as_bool) != Some(false)
            && self.lyrics.state == LyrState::Synced
            && self.lyrics.key == self.media.key()
    }

    pub fn media_pos(&self) -> f64 {
        self.media.pos()
    }

    // ------------------------------------------------------------------ services → hub
    pub fn on_event(&mut self, e: Event, cx: &mut Context<Self>) {
        match e {
            Event::Media(m) => self.set_media(m, cx),
            Event::Volume { level, muted, ours } => {
                self.volume = level;
                self.muted = muted;
                if !ours {
                    cx.emit(HubEvent::Volume { force: false });
                }
                cx.notify();
            }
            Event::Levels(l) => {
                self.level = l;
                self.level_at = Instant::now();
            }
            Event::Weather(w) => {
                self.weather = Some(w);
                cx.notify();
            }
            Event::Privacy(p) => self.set_privacy(p, cx),
            Event::Notification { app, title, body } => {
                if self.i("notifications") {
                    cx.emit(HubEvent::Notification { app, title, body });
                }
            }
            Event::NotifyStatus(s) => {
                self.diag.notifications = s;
                cx.notify();
            }
            Event::Lyrics { req, result } => {
                if req != self.lyrics.req {
                    return; // a stale answer for a song that already ended
                }
                self.set_lyrics(result, cx);
            }
            Event::Toast(t) => cx.emit(HubEvent::Toast(t)),
            _ => {}
        }
    }

    pub fn set_media(&mut self, m: MediaInfo, cx: &mut Context<Self>) {
        let changed = m.active && !m.title.is_empty() && Some(&m.title) != self.last_title.as_ref();
        if m.active {
            self.last_title = Some(m.title.clone());
        }
        self.media = m;
        if changed && self.media_seen && self.i("media") && self.settings.get("island.trackPopup").and_then(Value::as_bool) != Some(false) {
            cx.emit(HubEvent::Track);
        }
        self.media_seen = true;
        // fetch lyrics ahead of time only when the island will sing along
        if self.media.playing && self.settings.get("island.lyricsNotch").and_then(Value::as_bool) != Some(false) {
            self.ensure_lyrics(cx);
        }
        self.update_metering_demo(cx);
        self.update_tick(cx);
        cx.notify();
    }

    fn set_privacy(&mut self, p: PrivacyInfo, cx: &mut Context<Self>) {
        let newly = if p.camera && !self.privacy.camera {
            Some(("Camera", p.camera_app.clone(), true))
        } else if p.mic && !self.privacy.mic {
            Some(("Microphone", p.mic_app.clone(), false))
        } else {
            None
        };
        self.privacy = p;
        if let Some((what, app, camera)) = newly {
            if self.i("privacy") && self.i("events") {
                cx.emit(HubEvent::PrivacyNew { what, app, camera });
            }
        }
        cx.notify();
    }

    fn set_lyrics(&mut self, r: LyricsResult, cx: &mut Context<Self>) {
        self.lyrics.lines = r.synced.as_deref().map(crate::lyrics::parse_lrc).unwrap_or_default();
        self.lyrics.plain = r.plain.unwrap_or_default();
        self.lyrics.state = if !self.lyrics.lines.is_empty() {
            LyrState::Synced
        } else if r.instrumental {
            LyrState::Instrumental
        } else if !self.lyrics.plain.is_empty() {
            LyrState::Plain
        } else {
            LyrState::Missing
        };
        cx.emit(HubEvent::LyricsReady);
        cx.notify();
    }

    /// Request lyrics for the current track if we don't have them yet. Returns true when a new lookup started.
    pub fn ensure_lyrics(&mut self, cx: &mut Context<Self>) -> bool {
        let k = self.media.key();
        if !self.lyrics_on() || k.is_empty() || k == self.lyrics.key {
            return false;
        }
        self.lyrics.req += 1;
        self.lyrics.key = k;
        self.lyrics.state = LyrState::Loading;
        self.lyrics.lines.clear();
        self.lyrics.plain.clear();
        let req = self.lyrics.req;
        if self.demo.is_some() {
            let lrc = demo::tracks().iter().find(|t| t.title == self.media.title).map(|t| t.lrc).unwrap_or("");
            let result = LyricsResult { found: !lrc.is_empty(), synced: if lrc.is_empty() { None } else { Some(lrc.into()) }, plain: None, instrumental: false };
            cx.spawn(async move |this, cx| {
                cx.background_executor().timer(Duration::from_millis(350)).await;
                let _ = this.update(cx, |h, cx| {
                    if h.lyrics.req == req {
                        h.set_lyrics(result, cx)
                    }
                });
            })
            .detach();
        } else {
            let (a, t, al, d, b) = (self.media.artist.clone(), self.media.title.clone(), self.media.album.clone(), self.media.duration, self.media.browser);
            std::thread::spawn(move || {
                let result = crate::lyrics::get(&a, &t, &al, d, b);
                post(Event::Lyrics { req, result });
            });
        }
        true
    }

    // ------------------------------------------------------------------ actions (UI → services)
    pub fn media_cmd(&mut self, action: &str, cx: &mut Context<Self>) {
        if let Some(d) = self.demo.as_mut() {
            d.action(action);
            let m = d.media();
            self.set_media(m, cx);
        } else {
            crate::media::send(crate::media::Cmd::Action(action.into()));
        }
    }

    pub fn seek(&mut self, pos: f64, cx: &mut Context<Self>) {
        self.media.position = pos;
        self.media.pos_stamp = now_ms();
        if let Some(d) = self.demo.as_mut() {
            d.seek(pos);
        } else {
            crate::media::send(crate::media::Cmd::Seek(pos));
        }
        cx.notify();
    }

    pub fn select_source(&mut self, id: &str, cx: &mut Context<Self>) {
        for s in self.media.sources.iter_mut() {
            s.selected = s.id == id;
        }
        if let Some(d) = self.demo.as_mut() {
            d.select(id);
            let m = d.media();
            self.set_media(m, cx);
        } else {
            crate::media::send(crate::media::Cmd::Select(id.into()));
        }
        cx.notify();
    }

    pub fn set_volume(&mut self, v: f32, cx: &mut Context<Self>) {
        self.volume = v.clamp(0.0, 1.0);
        if self.volume > 0.0 {
            self.muted = false;
        }
        if self.demo.is_none() {
            crate::audio::send(crate::audio::Cmd::SetLevel(self.volume));
        }
        cx.notify();
    }

    pub fn toggle_mute(&mut self, cx: &mut Context<Self>) {
        if self.demo.is_some() {
            self.muted = !self.muted;
        } else {
            crate::audio::send(crate::audio::Cmd::ToggleMute);
        }
        cx.notify();
    }

    pub fn set_metering(&mut self, on: bool, cx: &mut Context<Self>) {
        if on == self.metering {
            return;
        }
        self.metering = on;
        if self.demo.is_none() {
            crate::audio::send(crate::audio::Cmd::Metering(on));
        }
        self.update_metering_demo(cx);
    }

    fn update_metering_demo(&mut self, cx: &mut Context<Self>) {
        if self.demo.is_none() {
            return;
        }
        let want = self.metering && self.media.playing;
        if want && self.demo_level_task.is_none() {
            self.demo_level_task = Some(cx.spawn(async move |this, cx| {
                let mut ph = 0f32;
                loop {
                    cx.background_executor().timer(Duration::from_millis(66)).await;
                    ph += 0.33;
                    let v = 0.35 + 0.3 * ph.sin().abs() + 0.25 * (ph * 2.7 + 1.0).sin().abs();
                    if this.update(cx, |h, _| { h.level = v; h.level_at = Instant::now(); }).is_err() {
                        break;
                    }
                }
            }));
        } else if !want {
            self.demo_level_task = None;
        }
    }

    /// Stats are sampled only while someone looks at them (bit 1 = island dashboard).
    pub fn subscribe_stats(&mut self, who: u8, on: bool, cx: &mut Context<Self>) {
        let before = self.stats_subs;
        if on {
            self.stats_subs |= who;
        } else {
            self.stats_subs &= !who;
        }
        if before == 0 && self.stats_subs != 0 {
            self.sampler.prime();
            self.stats_task = Some(cx.spawn(async move |this, cx| {
                let mut k = 0u32;
                loop {
                    let first = k == 0;
                    cx.background_executor().timer(Duration::from_millis(if first { 300 } else { 2000 })).await;
                    k += 1;
                    let ok = this.update(cx, |h, cx| {
                        if h.demo.is_some() {
                            let kk = k as f64;
                            h.stats = Some(StatsInfo { cpu: 8.0 + 6.0 * (kk / 3.0).sin().abs(), ram: 46.0 + (k % 3) as f64, ram_used: 14.7, ram_total: 32.0 });
                        } else {
                            h.stats = Some(h.sampler.sample());
                        }
                        cx.notify();
                    });
                    if ok.is_err() {
                        break;
                    }
                }
            }));
        } else if self.stats_subs == 0 {
            self.stats_task = None;
        }
    }

    // ------------------------------------------------------------------ timer (live activity)
    pub fn timer_start(&mut self, min: f64, cx: &mut Context<Self>) {
        let ms = (min * 60000.0) as i64;
        self.timer = Timer { total: ms, end: now_ms() + ms, paused: 0 };
        self.save_timer();
        self.update_tick(cx);
        cx.notify();
    }
    pub fn timer_toggle(&mut self, cx: &mut Context<Self>) {
        let t = &mut self.timer;
        if t.paused > 0 {
            t.end = now_ms() + t.paused;
            t.paused = 0;
        } else if t.end > now_ms() {
            t.paused = t.end - now_ms();
        }
        self.save_timer();
        self.update_tick(cx);
        cx.notify();
    }
    pub fn timer_reset(&mut self, cx: &mut Context<Self>) {
        self.timer = Timer::default();
        self.save_timer();
        self.update_tick(cx);
        cx.notify();
    }
    fn save_timer(&self) {
        if self.demo.is_some() {
            return;
        }
        let t = self.timer;
        let _ = std::fs::create_dir_all(settings::cache_dir());
        let _ = std::fs::write(timer_path(), serde_json::json!({ "end": t.end, "total": t.total, "paused": t.paused }).to_string());
    }

    /// A 1 s tick only while a timer runs (countdown + finish detection).
    fn update_tick(&mut self, cx: &mut Context<Self>) {
        let needs = self.timer.running() && !self.timer.is_paused();
        if needs && self.tick_task.is_none() {
            self.tick_task = Some(cx.spawn(async move |this, cx| loop {
                let ms = 1000 - (now_ms() % 1000) + 5;
                cx.background_executor().timer(Duration::from_millis(ms as u64)).await;
                let keep = this
                    .update(cx, |h, cx| {
                        if h.timer.finished_now() {
                            h.timer = Timer::default();
                            h.save_timer();
                            if !h.preview {
                                crate::chime::play();
                            }
                            cx.emit(HubEvent::TimerDone);
                            cx.notify();
                            h.tick_task = None;
                            return false;
                        }
                        cx.notify();
                        h.timer.running() && !h.timer.is_paused()
                    })
                    .unwrap_or(false);
                if !keep {
                    break;
                }
            }));
        } else if !needs {
            self.tick_task = None;
        }
    }

    // ------------------------------------------------------------------ settings
    pub fn apply_settings(&mut self, patch: Value, cx: &mut Context<Self>) {
        self.settings.apply(&patch);
        cx.emit(HubEvent::SettingsChanged(patch));
        cx.notify();
    }

    pub fn set_setting(&mut self, path: &str, v: Value, cx: &mut Context<Self>) {
        self.apply_settings(settings::patch(path, v), cx);
    }

    pub fn reset_settings(&mut self, cx: &mut Context<Self>) {
        self.settings.reset();
        let doc = self.settings.doc.clone();
        cx.emit(HubEvent::SettingsChanged(doc));
        cx.notify();
    }
}

pub fn demo_hub(cx: &mut App, style: &str, playing: bool, preview: bool) -> gpui::Entity<Hub> {
    let mut s = Settings { doc: settings::defaults(), read_only: true };
    s.doc["island"]["style"] = Value::String(style.into());
    s.doc["island"]["events"] = Value::Bool(false);
    s.doc["weather"] = serde_json::json!({ "city": "Oslo, Norway", "lat": 59.91, "lon": 10.75, "units": "c" });
    cx.new(|cx| Hub::new(s, Some(DemoPlayer::new(playing)), preview, cx))
}
