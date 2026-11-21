//! The island: one state machine (compact → live activities → HUD / drop-down events → expanded) with two looks:
//! "Dynamic Notch" (notch.rs) and "Dynamic Island" (ios.rs). Geometry is driven by real springs that only
//! run while something moves; when idle nothing is redrawn.

pub mod ios;
pub mod notch;
pub mod parts;

use crate::hub::{Hub, HubEvent, LyrState};
use crate::spring::{ease_out, Spring};
use crate::win::{self, Shape};
use gpui::prelude::*;
use gpui::*;
use std::sync::Arc;
use std::time::{Duration, Instant};
use windows::Win32::Foundation::HWND;

/// Logical size of the island window (largest expanded state + margin).
pub const CANVAS_W: f32 = 760.0;
pub const CANVAS_H: f32 = 520.0;
/// Notch shoulder radius.
pub const SH: f32 = 12.0;
pub static RENDERS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum View {
    Compact,
    Hud,
    Drop,
    Exp,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Tab {
    Home,
    Lyrics,
    Timer,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Activity {
    Music,
    Timer,
    Privacy,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Mode {
    Empty,
    Idle,
    Music,
    MusicLyrics,
    Timer,
    Privacy,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Content {
    Compact(Mode),
    Hud,
    Drop(u64),
    Exp(u8),
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum DropKind {
    Track,
    Notification,
    Privacy,
    TimerDone,
    Greet,
}

#[derive(Clone)]
pub struct DropInfo {
    pub id: u64,
    pub kind: DropKind,
    pub pic: Option<Arc<RenderImage>>,
    pub icon: Option<(&'static str, u32)>,
    pub dot: Option<u32>,
    pub k: String,
    pub t1: String,
    pub t2: String,
    pub viz: bool,
    pub w: f32,
}

#[derive(Clone, Copy, PartialEq, Debug, Default)]
pub struct Geo {
    pub w: f32,
    pub h: f32,
    pub r: f32,
    pub y: f32,
}

pub struct IslandView {
    pub hub: Entity<Hub>,
    pub hwnd: Option<HWND>,
    pub preview: bool,
    pub canvas: Size<f32>,
    pub view: View,
    pub content: Content,
    pub content_at: Instant,
    pub prev: Option<(Content, Instant)>,
    pub hover: bool,
    pub w: Spring,
    pub h: Spring,
    pub r: Spring,
    pub y: Spring,
    /// iOS: presence of the detached "minimal" bubble (0 hidden … 1 out)
    pub bub: Spring,
    pub bub_activity: Option<Activity>,
    last_frame: Option<Instant>,
    transient: Option<Task<()>>,
    collapse: Option<Task<()>>,
    hover_task: Option<Task<()>>,
    pub tab: Tab,
    pub drop: DropInfo,
    drop_seq: u64,
    pub primary: Option<Activity>,
    pub widgets_open: bool,
    pub source_menu: bool,
    ambient: Option<Task<()>>,
    ambient_want: bool,
    metering: bool,
    stats_on: bool,
    pub lyr_scroll: Spring,
    pub lyr_idx: isize,
    last_region: Vec<Shape>,
    prev_shapes: Vec<Shape>,
    greeted: bool,
    pub t0: Instant,
    pub glow: Option<Entity<crate::glow::GlowView>>,
    glow_sent: Option<(Geo, Hsla, bool, bool)>,
    _subs: Vec<Subscription>,
}

impl IslandView {
    pub fn new(hub: Entity<Hub>, hwnd: Option<HWND>, preview: bool, canvas: Size<f32>, cx: &mut Context<Self>) -> Self {
        let subs = vec![
            cx.observe(&hub, |this, _, cx| {
                this.refresh_wants(cx);
                cx.notify()
            }),
            cx.subscribe(&hub, |this, _, ev: &HubEvent, cx| this.on_hub_event(ev.clone(), cx)),
        ];
        let mut v = IslandView {
            hub,
            hwnd,
            preview,
            canvas,
            view: View::Compact,
            content: Content::Compact(Mode::Idle),
            content_at: Instant::now() - Duration::from_secs(5),
            prev: None,
            hover: false,
            w: Spring::new(200.0),
            h: Spring::new(32.0),
            r: Spring::new(14.0),
            y: Spring::new(0.0),
            bub: Spring::new(0.0).with(260.0, 19.0),
            bub_activity: None,
            last_frame: None,
            transient: None,
            collapse: None,
            hover_task: None,
            tab: Tab::Home,
            drop: DropInfo { id: 0, kind: DropKind::Greet, pic: None, icon: None, dot: None, k: String::new(), t1: String::new(), t2: String::new(), viz: false, w: 430.0 },
            drop_seq: 0,
            primary: None,
            widgets_open: false,
            source_menu: false,
            ambient: None,
            ambient_want: false,
            metering: false,
            stats_on: false,
            lyr_scroll: Spring::new(0.0).with(190.0, 24.0),
            lyr_idx: -2,
            last_region: vec![],
            prev_shapes: vec![],
            greeted: false,
            t0: Instant::now(),
            glow: None,
            glow_sent: None,
            _subs: subs,
        };
        // start at the right size without a morph
        let g = v.target_geo(cx);
        v.w.snap(g.w);
        v.h.snap(g.h);
        v.r.snap(g.r);
        v.y.snap(g.y);
        if !preview {
            // greeting at start-up
            cx.spawn(async move |this, cx| {
                cx.background_executor().timer(Duration::from_millis(1200)).await;
                let _ = this.update(cx, |t, cx| t.greet(cx));
            })
            .detach();
        }
        v
    }

    // ------------------------------------------------------------------ hub events
    fn on_hub_event(&mut self, ev: HubEvent, cx: &mut Context<Self>) {
        match ev {
            HubEvent::Volume { force } => {
                let on = self.hub.read(cx).i("volume");
                if self.view == View::Exp {
                    cx.notify();
                } else if force || on {
                    self.transient(View::Hud, 1600, cx);
                }
            }
            HubEvent::Track => {
                if self.view == View::Exp {
                    return;
                }
                let h = self.hub.read(cx);
                let m = &h.media;
                let d = DropInfo {
                    id: 0,
                    kind: DropKind::Track,
                    pic: m.art.clone(),
                    icon: Some(("music", crate::theme::DIM)),
                    dot: None,
                    k: if m.app.is_empty() { "Now playing".into() } else { m.app.clone() },
                    t1: m.title.clone(),
                    t2: m.artist.clone(),
                    viz: h.i("visualizer"),
                    w: 430.0,
                };
                self.show_drop(d, 3600, cx);
            }
            HubEvent::Notification { app, title, body } => {
                let d = DropInfo {
                    id: 0,
                    kind: DropKind::Notification,
                    pic: None,
                    icon: Some(("bell", crate::theme::TEXT)),
                    dot: None,
                    k: if app.is_empty() { "Notification".into() } else { app },
                    t1: title,
                    t2: body,
                    viz: false,
                    w: 450.0,
                };
                self.show_drop(d, 5000, cx);
            }
            HubEvent::PrivacyNew { what, app, camera } => {
                let d = DropInfo {
                    id: 0,
                    kind: DropKind::Privacy,
                    pic: None,
                    icon: None,
                    dot: Some(if camera { crate::theme::GREEN } else { crate::theme::ORANGE }),
                    k: "Privacy".into(),
                    t1: format!("{what} in use"),
                    t2: if app.is_empty() { "An app is using it".into() } else { app },
                    viz: false,
                    w: 400.0,
                };
                self.show_drop(d, 2600, cx);
            }
            HubEvent::TimerDone => {
                let d = DropInfo {
                    id: 0,
                    kind: DropKind::TimerDone,
                    pic: None,
                    icon: Some(("clock", crate::theme::ORANGE)),
                    dot: None,
                    k: "Timer".into(),
                    t1: "Time's up".into(),
                    t2: "Your timer has finished".into(),
                    viz: false,
                    w: 400.0,
                };
                if self.view == View::Exp {
                    cx.notify();
                } else {
                    self.show_drop(d, 8000, cx);
                }
            }
            HubEvent::Scene(s) => self.scene(&s, cx),
            HubEvent::SettingsChanged(_) | HubEvent::LyricsReady => cx.notify(),
            HubEvent::Toast(_) => {}
        }
    }

    fn greet(&mut self, cx: &mut Context<Self>) {
        if self.greeted {
            return;
        }
        self.greeted = true;
        if !self.hub.read(cx).i("events") {
            return;
        }
        let (t1, t2) = crate::util::greeting();
        let d = DropInfo { id: 0, kind: DropKind::Greet, pic: None, icon: Some(("tree", crate::theme::GREEN)), dot: None, k: "WinIsland".into(), t1, t2, viz: false, w: 430.0 };
        self.show_drop(d, 4200, cx);
    }

    /// Demo / test hook: open a view without mouse input.
    pub fn scene(&mut self, s: &str, cx: &mut Context<Self>) {
        self.greeted = true;
        self.transient = None;
        let hub = self.hub.clone();
        // "<scene>-plain": without the sing-along line in the compact island
        let s = match s.strip_suffix("-plain") {
            Some(base) => {
                hub.update(cx, |h, _| h.settings.doc["island"]["lyricsNotch"] = serde_json::Value::Bool(false));
                base
            }
            None => s,
        };
        match s {
            "sequence" => {
                // for animated captures: idle → music → expanded → collapse (~6 s)
                hub.update(cx, |h, cx| {
                    if let Some(d) = h.demo.as_mut() {
                        d.playing = false;
                        let m = d.media();
                        h.set_media(m, cx);
                    }
                });
                self.back(cx);
                cx.spawn(async move |this, cx| {
                    let exec = cx.background_executor().clone();
                    let t = move |ms| exec.timer(Duration::from_millis(ms));
                    t(2000).await;
                    let _ = this.update(cx, |v, cx| {
                        v.hub.update(cx, |h, cx| {
                            if let Some(d) = h.demo.as_mut() {
                                d.playing = true;
                                d.stamp = crate::model::now_ms();
                                let m = d.media();
                                h.set_media(m, cx);
                            }
                        })
                    });
                    t(1600).await;
                    let _ = this.update(cx, |v, cx| v.expand(cx));
                    t(2200).await;
                    let _ = this.update(cx, |v, cx| v.back(cx));
                })
                .detach();
            }
            "idle" => {
                hub.update(cx, |h, cx| {
                    if let Some(d) = h.demo.as_mut() {
                        d.playing = false;
                        let m = d.media();
                        h.set_media(m, cx);
                    }
                });
                self.back(cx);
            }
            "music" | "compact" => self.back(cx),
            "expanded" | "exp" => {
                self.tab = Tab::Home;
                self.expand(cx);
            }
            "lyrics" => {
                self.tab = Tab::Lyrics;
                hub.update(cx, |h, cx| {
                    h.ensure_lyrics(cx);
                });
                self.expand(cx);
            }
            "volume" | "hud" => self.transient(View::Hud, 600_000, cx),
            "drop" | "banner" => {
                let h = hub.read(cx);
                let m = &h.media;
                let d = DropInfo { id: 0, kind: DropKind::Track, pic: m.art.clone(), icon: Some(("music", crate::theme::DIM)), dot: None, k: m.app.clone(), t1: m.title.clone(), t2: m.artist.clone(), viz: true, w: 430.0 };
                self.show_drop(d, 600_000, cx);
            }
            "timer" => {
                hub.update(cx, |h, cx| h.timer_start(25.0, cx));
                self.tab = Tab::Timer;
                self.primary = Some(Activity::Timer);
                self.expand(cx);
            }
            "compact-timer" => {
                hub.update(cx, |h, cx| h.timer_start(25.0, cx));
                self.back(cx);
            }
            "two-activities" | "split" => {
                hub.update(cx, |h, cx| h.timer_start(25.0, cx));
                self.primary = Some(Activity::Music);
                self.back(cx);
            }
            "camera" => {
                hub.update(cx, |h, cx| {
                    if let Some(d) = h.demo.as_mut() {
                        d.playing = false;
                        let m = d.media();
                        h.set_media(m, cx);
                    }
                    h.privacy = crate::model::PrivacyInfo { camera: true, mic: false, camera_app: "Teams".into(), mic_app: String::new() };
                    cx.notify();
                });
                self.back(cx);
            }
            "two-camera" => {
                hub.update(cx, |h, cx| {
                    h.privacy = crate::model::PrivacyInfo { camera: true, mic: false, camera_app: "Teams".into(), mic_app: String::new() };
                    cx.notify();
                });
                self.primary = Some(Activity::Music);
                self.back(cx);
            }
            "expanded-timer" => {
                hub.update(cx, |h, cx| h.timer_start(25.0, cx));
                self.primary = Some(Activity::Timer);
                self.tab = Tab::Timer;
                self.expand(cx);
            }
            "expanded-widgets" => {
                self.widgets_open = true;
                self.tab = Tab::Home;
                self.expand(cx);
            }
            "expanded-sources" => {
                self.source_menu = true;
                self.expand(cx);
            }
            "expanded-idle" => {
                hub.update(cx, |h, cx| {
                    if let Some(d) = h.demo.as_mut() {
                        d.playing = false;
                    }
                    h.media = crate::model::MediaInfo::default();
                    cx.notify();
                });
                self.expand(cx);
            }
            "notification" => {
                hub.update(cx, |_, cx| cx.emit(HubEvent::Notification { app: "Messages".into(), title: "Ingrid".into(), body: "Are we still on for skating tonight?".into() }));
            }
            _ => {}
        }
        cx.notify();
    }

    // ------------------------------------------------------------------ state changes
    pub fn set_view(&mut self, v: View, cx: &mut Context<Self>) {
        let was = self.view;
        self.view = v;
        self.refresh_wants(cx);
        if v != View::Exp {
            self.source_menu = false;
        }
        if v == View::Exp && was != View::Exp {
            let sys = self.hub.read(cx).i("system");
            if sys {
                self.stats_on = true;
                self.hub.update(cx, |h, cx| h.subscribe_stats(1, true, cx));
            }
        }
        if v != View::Exp && was == View::Exp && self.stats_on {
            self.stats_on = false;
            self.hub.update(cx, |h, cx| h.subscribe_stats(1, false, cx));
        }
        cx.notify();
    }

    pub fn transient(&mut self, v: View, ms: u64, cx: &mut Context<Self>) {
        if self.view == View::Exp {
            return;
        }
        self.set_view(v, cx);
        self.transient = Some(cx.spawn(async move |this, cx| {
            cx.background_executor().timer(Duration::from_millis(ms)).await;
            let _ = this.update(cx, |t, cx| t.back(cx));
        }));
    }

    pub fn show_drop(&mut self, mut d: DropInfo, ms: u64, cx: &mut Context<Self>) {
        if self.view == View::Exp {
            return;
        }
        self.drop_seq += 1;
        d.id = self.drop_seq;
        self.drop = d;
        self.transient(View::Drop, ms, cx);
    }

    pub fn back(&mut self, cx: &mut Context<Self>) {
        self.transient = None;
        self.collapse = None;
        self.set_view(View::Compact, cx);
    }

    pub fn expand(&mut self, cx: &mut Context<Self>) {
        self.transient = None;
        self.collapse = None;
        if self.tab == Tab::Lyrics && !self.hub.read(cx).lyrics_on() {
            self.tab = Tab::Home;
        }
        // the Lyrics tab (notch) and the Live Activity card (island) show the current line
        if self.tab == Tab::Lyrics || self.hub.read(cx).ios() {
            self.hub.update(cx, |h, cx| {
                h.ensure_lyrics(cx);
            });
        }
        self.lyr_idx = -2;
        self.set_view(View::Exp, cx);
    }

    pub fn set_tab(&mut self, t: Tab, cx: &mut Context<Self>) {
        self.tab = t;
        if t == Tab::Lyrics {
            self.hub.update(cx, |h, cx| {
                h.ensure_lyrics(cx);
            });
            self.lyr_idx = -2;
        }
        cx.notify();
    }

    pub fn on_hover_change(&mut self, hovered: bool, cx: &mut Context<Self>) {
        if hovered == self.hover {
            return;
        }
        self.hover = hovered;
        if hovered {
            self.collapse = None;
            if self.view == View::Compact && self.hub.read(cx).i("hoverExpand") {
                self.hover_task = Some(cx.spawn(async move |this, cx| {
                    cx.background_executor().timer(Duration::from_millis(380)).await;
                    let _ = this.update(cx, |t, cx| {
                        if t.hover && t.view == View::Compact {
                            t.expand(cx)
                        }
                    });
                }));
            }
        } else {
            self.hover_task = None;
            // Leaving always closes the dashboard, however it was opened. A short grace keeps a pointer that
            // just grazes the edge from slamming it shut; the user's collapse delay is respected.
            if self.view == View::Exp {
                let delay = self.hub.read(cx).settings.f("island.collapseDelay").unwrap_or(0.0).max(0.3);
                self.collapse = Some(cx.spawn(async move |this, cx| {
                    cx.background_executor().timer(Duration::from_millis((delay * 1000.0) as u64)).await;
                    let _ = this.update(cx, |t, cx| {
                        if !t.hover {
                            t.back(cx)
                        }
                    });
                }));
            }
        }
        cx.notify();
    }

    pub fn on_click_island(&mut self, cx: &mut Context<Self>) {
        if self.view != View::Exp {
            self.expand(cx);
        }
    }

    pub fn on_scroll(&mut self, up: bool, cx: &mut Context<Self>) {
        if self.view == View::Exp {
            return;
        }
        let hub = self.hub.clone();
        hub.update(cx, |h, cx| {
            let nv = (h.volume + if up { 0.02 } else { -0.02 }).clamp(0.0, 1.0);
            h.set_volume(nv, cx);
        });
        self.transient(View::Hud, 1600, cx);
    }

    pub fn open_settings(&mut self, cx: &mut Context<Self>) {
        if !self.preview {
            crate::model::post(crate::model::Event::OpenPanel);
        }
        let _ = cx;
    }

    // ------------------------------------------------------------------ activities
    pub fn privacy_dots(&self, h: &Hub) -> usize {
        if !h.i("privacy") {
            return 0;
        }
        h.privacy.camera as usize + h.privacy.mic as usize
    }

    /// Live activities in priority order (iOS look).
    pub fn activities(&self, h: &Hub) -> Vec<Activity> {
        let mut v = vec![];
        if h.i("media") && h.media.active && h.media.playing {
            v.push(Activity::Music);
        }
        if h.timer.running() {
            v.push(Activity::Timer);
        }
        if h.i("privacy") && (h.privacy.camera || h.privacy.mic) {
            v.push(Activity::Privacy);
        }
        v
    }

    /// (primary, secondary) for the iOS look; the user's pick (bubble click) wins while it is still live.
    pub fn primary_secondary(&self, h: &Hub) -> (Option<Activity>, Option<Activity>) {
        let acts = self.activities(h);
        let p = match self.primary {
            Some(p) if acts.contains(&p) => Some(p),
            _ => acts.first().copied(),
        };
        let s = acts.iter().copied().find(|a| Some(*a) != p);
        (p, s)
    }

    pub fn swap_primary(&mut self, cx: &mut Context<Self>) {
        let (_, s) = self.primary_secondary(self.hub.read(cx));
        if let Some(s) = s {
            self.primary = Some(s);
        }
        cx.notify();
    }

    pub fn mode(&self, h: &Hub) -> Mode {
        if h.ios() {
            return match self.primary_secondary(h).0 {
                Some(Activity::Music) => {
                    if h.notch_lyrics() {
                        Mode::MusicLyrics
                    } else {
                        Mode::Music
                    }
                }
                Some(Activity::Timer) => Mode::Timer,
                Some(Activity::Privacy) => Mode::Privacy,
                None => {
                    if h.i("idleInfo") {
                        Mode::Idle
                    } else {
                        Mode::Empty
                    }
                }
            };
        }
        if h.timer.running() {
            Mode::Timer
        } else if h.i("media") && h.media.active && h.media.playing {
            if h.notch_lyrics() {
                Mode::MusicLyrics
            } else {
                Mode::Music
            }
        } else if h.i("idleInfo") {
            Mode::Idle
        } else {
            Mode::Empty
        }
    }

    pub fn content_key(&self, h: &Hub) -> Content {
        match self.view {
            View::Compact => Content::Compact(self.mode(h)),
            View::Hud => Content::Hud,
            View::Drop => Content::Drop(self.drop.id),
            View::Exp => {
                if h.ios() {
                    Content::Exp(ios::card_kind(self, h) as u8)
                } else {
                    Content::Exp(self.tab as u8)
                }
            }
        }
    }

    pub fn target_geo(&self, cx: &App) -> Geo {
        let h = self.hub.read(cx);
        if h.ios() {
            ios::geo(self, h)
        } else {
            notch::geo(self, h)
        }
    }

    // ------------------------------------------------------------------ frame
    fn step(&mut self, target: Geo, bub_target: f32) -> bool {
        self.w.set(target.w);
        self.h.set(target.h);
        self.r.set(target.r);
        self.y.set(target.y);
        self.bub.set(bub_target);
        let now = Instant::now();
        let dt = match self.last_frame {
            Some(t) => (now - t).as_secs_f32().clamp(0.0, 1.0 / 30.0),
            None => 1.0 / 60.0,
        };
        let mut moving = false;
        moving |= self.w.step(dt);
        moving |= self.h.step(dt);
        moving |= self.r.step(dt);
        moving |= self.y.step(dt);
        moving |= self.bub.step(dt);
        moving |= self.lyr_scroll.step(dt);
        self.last_frame = if moving { Some(now) } else { None };
        moving
    }

    pub fn geo(&self) -> Geo {
        Geo { w: self.w.pos.max(1.0), h: self.h.pos.max(1.0), r: self.r.pos.max(0.0), y: self.y.pos }
    }

    pub fn cx_center(&self) -> f32 {
        self.canvas.width / 2.0
    }

    /// Fade-in alpha of the current content and fade-out alpha of the previous one.
    pub fn fades(&self) -> (f32, f32) {
        let t = self.content_at.elapsed().as_secs_f32();
        let a_in = ease_out((t - 0.06) / 0.26);
        let a_out = 1.0 - ease_out(t / 0.16);
        (a_in, a_out)
    }

    fn update_region(&mut self, window: &Window, shapes: Vec<Shape>) {
        let Some(hwnd) = self.hwnd else { return };
        let _ = window;
        // the union with the previous frame keeps a shrinking shape from being clipped while DWM catches up
        let mut all = shapes.clone();
        for s in std::mem::replace(&mut self.prev_shapes, shapes) {
            if !all.contains(&s) {
                all.push(s);
            }
        }
        if all != self.last_region {
            win::set_region(hwnd, &all);
            self.last_region = all;
        }
    }

    /// Device-pixel click shapes for the current geometry (+ a small margin so the anti-aliased edge isn't cut).
    fn shapes(&self, scale: f32, ios: bool) -> Vec<Shape> {
        let g = self.geo();
        let cx = self.cx_center();
        let m = 2.0;
        let d = |v: f32| (v * scale).round() as i32;
        let mut out = vec![];
        if ios {
            let rr = g.r.min(g.h / 2.0);
            out.push(Shape::Round { l: d(cx - g.w / 2.0 - m), t: d(g.y - m), r: d(cx + g.w / 2.0 + m), b: d(g.y + g.h + m), radius: d(rr + m) });
            if let Some((bx, by, bd)) = self.bubble_geo() {
                out.push(Shape::Round { l: d(bx - bd / 2.0 - m), t: d(by - bd / 2.0 - m), r: d(bx + bd / 2.0 + m), b: d(by + bd / 2.0 + m), radius: d(bd / 2.0 + m) });
            }
        } else {
            let rr = g.r.min(g.h / 2.0);
            // body (square top corners) + the shoulder band at the top edge
            out.push(Shape::Round { l: d(cx - g.w / 2.0 - m), t: d(-40.0), r: d(cx + g.w / 2.0 + m), b: d(g.h + m), radius: d(rr + m) });
            out.push(Shape::Rect { l: d(cx - g.w / 2.0 - SH - m), t: 0, r: d(cx + g.w / 2.0 + SH + m), b: d(SH.min(g.h) + 1.0) });
        }
        out
    }

    /// iOS bubble: (centre x, centre y, diameter) while visible.
    pub fn bubble_geo(&self) -> Option<(f32, f32, f32)> {
        let p = self.bub.pos;
        if p < 0.02 {
            return None;
        }
        let g = self.geo();
        let d = 37.0 * (0.35 + 0.65 * p.clamp(0.0, 1.3));
        let main_cx = self.cx_center();
        let x = main_cx + (g.w / 2.0 + 8.0 + 37.0 / 2.0) * p.clamp(0.0, 1.2) ;
        let x = x.max(main_cx);
        Some((x, g.y + g.h / 2.0, d))
    }

    fn ambient_wanted(&self, h: &Hub, mode: Mode) -> bool {
        if !h.visible || !h.media.playing || !h.media.active {
            return false;
        }
        match self.view {
            View::Compact => matches!(mode, Mode::Music | Mode::MusicLyrics) && (h.i("visualizer") || mode == Mode::MusicLyrics),
            View::Drop => self.drop.viz && self.drop.kind == DropKind::Track,
            View::Exp => {
                if h.ios() {
                    ios::card_kind(self, h) == ios::CardKind::Music
                } else {
                    self.tab != Tab::Timer
                }
            }
            View::Hud => false,
        }
    }

    /// Metering and the 30 fps ambient loop must also stop while the window is hidden (no renders then).
    /// Re-apply the click region (after the window was moved / restyled).
    pub fn reset_region(&mut self, cx: &mut Context<Self>) {
        self.last_region.clear();
        cx.notify();
    }

    pub fn refresh_wants(&mut self, cx: &mut Context<Self>) {
        let hub = self.hub.clone();
        let (a, m) = {
            let h = hub.read(cx);
            let mode = self.mode(h);
            (self.ambient_wanted(h, mode), self.metering_wanted(h, mode))
        };
        self.update_metering(m, cx);
        self.ensure_ambient(a, cx);
    }

    fn ensure_ambient(&mut self, want: bool, cx: &mut Context<Self>) {
        self.ambient_want = want;
        if want && self.ambient.is_none() {
            self.ambient = Some(cx.spawn(async move |this, cx| loop {
                cx.background_executor().timer(Duration::from_millis(33)).await;
                let keep = this
                    .update(cx, |t, cx| {
                        if t.ambient_want {
                            cx.notify();
                            true
                        } else {
                            t.ambient = None;
                            false
                        }
                    })
                    .unwrap_or(false);
                if !keep {
                    break;
                }
            }));
        }
    }

    fn metering_wanted(&self, h: &Hub, mode: Mode) -> bool {
        h.visible
            && h.i("visualizer")
            && h.media.active
            && h.media.playing
            && (self.view == View::Exp || (self.view == View::Compact && matches!(mode, Mode::Music | Mode::MusicLyrics)) || self.view == View::Drop)
    }

    fn update_metering(&mut self, want: bool, cx: &mut Context<Self>) {
        if want != self.metering {
            self.metering = want;
            let hub = self.hub.clone();
            cx.defer(move |cx| hub.update(cx, |h, cx| h.set_metering(want, cx)));
        }
    }

    pub fn viz_values(&self, h: &Hub, n: usize) -> Vec<f32> {
        let fresh = h.level_at.elapsed() < Duration::from_millis(250) && h.level > 0.004;
        parts::viz_values(n, h.level, fresh, self.t0.elapsed().as_secs_f32())
    }

    /// Current lyric line index and fill for the playing track.
    pub fn lyric_now(&self, h: &Hub) -> (isize, f32) {
        if h.lyrics.state != LyrState::Synced {
            return (-1, 0.0);
        }
        let pos = h.media_pos() + crate::hub::LEAD;
        let i = h.lyrics.index(pos);
        (i, h.lyrics.fill(i, pos))
    }
}

impl Render for IslandView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        crate::island::RENDERS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let hub = self.hub.clone();
        let (ios, want_ambient, want_meter, glow_state) = {
            let h = hub.read(cx);
            let ios = h.ios();
            let mode = self.mode(h);

            // content crossfade bookkeeping
            let key = self.content_key(h);
            if key != self.content {
                let old = self.content;
                self.content = key;
                let fade_old = self.content_at.elapsed().as_secs_f32() > 0.12;
                self.prev = if fade_old { Some((old, Instant::now())) } else { self.prev };
                self.content_at = Instant::now();
            }
            if let Some((_, at)) = self.prev {
                if at.elapsed().as_secs_f32() > 0.2 {
                    self.prev = None;
                }
            }

            let target = if ios { ios::geo(self, h) } else { notch::geo(self, h) };
            let (_, sec) = self.primary_secondary(h);
            let bub_target = if ios && self.view == View::Compact && sec.is_some() { 1.0 } else { 0.0 };
            if sec.is_some() {
                self.bub_activity = sec;
            }
            if !ios {
                notch::prepare(self, h);
            }
            let moving = self.step(target, bub_target);
            let fading = self.content_at.elapsed().as_secs_f32() < 0.34 || self.prev.is_some();
            if moving || fading {
                window.request_animation_frame();
            }
            let glow_on = !ios && h.visible && h.i("glow") && h.media.active && h.media.playing && h.i("media");
            let glow_state = (self.geo(), crate::theme::tint_color(h.media.tint), glow_on, !ios && h.visible);
            (ios, self.ambient_wanted(h, mode), self.metering_wanted(h, mode), glow_state)
        };
        if let Some(g) = self.glow.clone() {
            let changed = self.glow_sent.map(|s| s != glow_state).unwrap_or(true);
            let relevant = glow_state.3 || self.glow_sent.map(|s| s.3).unwrap_or(true);
            if changed && relevant {
                self.glow_sent = Some(glow_state);
                cx.defer(move |cx| g.update(cx, |gv, cx| gv.set(glow_state.0, glow_state.1, glow_state.2, glow_state.3, cx)));
            }
        }
        self.update_metering(want_meter, cx);
        self.ensure_ambient(want_ambient, cx);

        let shapes = self.shapes(window.scale_factor(), ios);
        self.update_region(window, shapes);

        let body = if ios { ios::render(self, window, cx) } else { notch::render(self, window, cx) };
        div().size_full().relative().child(body)
    }
}
