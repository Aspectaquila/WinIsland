//! Demo data for previews and the `--demo` test hook: fictional tracks with generated album art and lyrics.

use crate::model::{now_ms, MediaInfo, Source, WeatherInfo};
use gpui::RenderImage;
use std::sync::{Arc, OnceLock};

pub const LRC1: &str = "[00:00.00]
[00:09.60]Cold light over the water
[00:13.80]Silver lines across the sky
[00:18.10]Hold on, the night is getting brighter
[00:22.70]Every star is falling into time
[00:27.30]We were running through the snowfall
[00:31.60]Chasing colours we can't name
[00:36.00]Northern lights, northern lights
[00:40.20]Burning softly, calling out my name
[00:44.90]Stay with me until the morning
[00:49.20]Let the whole world fade to white
[00:53.60]Northern lights, northern lights
[00:57.90]You and me beneath the frozen sky
[01:02.40]
[01:10.80]Footprints in the quiet
[01:15.00]Breathing clouds into the cold
[01:19.40]Every heartbeat lighting up the silence
[01:23.90]Like a story that we've never told
[01:28.50]Northern lights, northern lights
[01:32.80]Burning softly, calling out my name";

pub const LRC2: &str = "[00:00.00]
[00:06.20]Glass harbor, glass harbor
[00:10.40]Boats asleep in frozen light
[00:14.80]I can hear the city humming
[00:19.00]Through the windows of the night
[00:23.60]Hold the lantern a little closer
[00:27.90]Let it melt the winter through
[00:32.30]Glass harbor, glass harbor
[00:36.50]Every road leads back to you";

pub struct Track {
    pub title: &'static str,
    pub artist: &'static str,
    pub album: &'static str,
    pub duration: f64,
    pub app: &'static str,
    pub browser: bool,
    pub lrc: &'static str,
    pub art: Arc<RenderImage>,
    pub tint: (f32, f32, f32),
}

pub fn tracks() -> &'static [Track] {
    static T: OnceLock<Vec<Track>> = OnceLock::new();
    T.get_or_init(|| {
        let mk = |title, artist, album, duration, app, browser, lrc, seed, pal: [u32; 4]| {
            let (art, tint) = crate::media::rgba_to_render(art(seed, pal));
            Track { title, artist, album, duration, app, browser, lrc, art, tint }
        };
        vec![
            mk("Northern Lights", "Aurora Fields", "Polar Hours", 214.0, "Spotify", false, LRC1, 7, [0x0b1b4a, 0x3a0f5c, 0x3ee6c1, 0x7f6bff]),
            mk("Glass Harbor", "Lumen", "Glass Harbor", 198.0, "Spotify", false, LRC2, 23, [0x3b0d2e, 0xf26b3a, 0xffb36b, 0xff4f8b]),
            mk("Snowfall at Night (Live Session)", "Winter Room", "", 263.0, "Chrome", true, "", 41, [0x0e2233, 0x2d6a8f, 0xbfe6ff, 0x6fb7ff]),
        ]
    })
}

#[derive(Clone)]
pub struct DemoPlayer {
    pub ti: usize,
    pub playing: bool,
    pub position: f64,
    pub stamp: i64,
}

impl DemoPlayer {
    pub fn new(playing: bool) -> Self {
        DemoPlayer { ti: 0, playing, position: 34.5, stamp: now_ms() }
    }
    pub fn pos(&self) -> f64 {
        self.position + if self.playing { (now_ms() - self.stamp) as f64 / 1000.0 } else { 0.0 }
    }
    pub fn media(&self) -> MediaInfo {
        let t = &tracks()[self.ti];
        let sp = &tracks()[if self.ti < 2 { self.ti } else { 0 }];
        let ch = &tracks()[2];
        MediaInfo {
            active: true,
            title: t.title.into(),
            artist: t.artist.into(),
            album: t.album.into(),
            app: t.app.into(),
            browser: t.browser,
            playing: self.playing,
            can_prev: true,
            can_next: true,
            position: self.pos(),
            duration: t.duration,
            pos_stamp: now_ms(),
            art: Some(t.art.clone()),
            tint: Some(t.tint),
            sources: vec![
                Source { id: "demo#0".into(), app: "Spotify".into(), browser: false, title: sp.title.into(), artist: sp.artist.into(), playing: self.playing && self.ti < 2, selected: self.ti < 2 },
                Source { id: "demo#2".into(), app: "Chrome".into(), browser: true, title: ch.title.into(), artist: ch.artist.into(), playing: self.playing && self.ti == 2, selected: self.ti == 2 },
            ],
        }
    }
    pub fn action(&mut self, a: &str) {
        match a {
            "toggle" => {
                self.position = self.pos();
                self.stamp = now_ms();
                self.playing = !self.playing;
            }
            _ => {
                self.ti = if self.ti == 2 { 2 } else { (self.ti + 1) % 2 };
                self.position = 0.0;
                self.stamp = now_ms();
                self.playing = true;
            }
        }
    }
    pub fn seek(&mut self, p: f64) {
        self.position = p;
        self.stamp = now_ms();
    }
    pub fn select(&mut self, id: &str) {
        self.ti = if id == "demo#2" { 2 } else { 0 };
        self.position = if self.ti == 2 { 72.0 } else { 34.5 };
        self.stamp = now_ms();
    }
}

pub fn weather() -> WeatherInfo {
    WeatherInfo { needs_city: false, city: "Oslo, Norway".into(), units: "C".into(), temp: Some(-4.0), feels: -9.0, code: 73, wind: 4.0, humidity: 80, is_day: true, hi: Some(-2.0), lo: Some(-9.0) }
}

// ---------------------------------------------------------------- generated album art (seeded, deterministic)

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> f32 {
        self.0 = (self.0 * 16807) % 2147483647;
        (self.0 as f32 - 1.0) / 2147483646.0
    }
}

fn col(c: u32) -> [f32; 3] {
    [((c >> 16) & 255) as f32, ((c >> 8) & 255) as f32, (c & 255) as f32]
}

fn mix(a: [f32; 3], b: [f32; 3], t: f32) -> [f32; 3] {
    [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t, a[2] + (b[2] - a[2]) * t]
}

fn screen(a: [f32; 3], b: [f32; 3], t: f32) -> [f32; 3] {
    let s = |x: f32, y: f32| 255.0 - (255.0 - x) * (255.0 - y) / 255.0;
    mix(a, [s(a[0], b[0]), s(a[1], b[1]), s(a[2], b[2])], t)
}

pub fn art(seed: u64, pal: [u32; 4]) -> image::RgbaImage {
    const N: u32 = 240;
    let sz = N as f32;
    let (a, b, c, d) = (col(pal[0]), col(pal[1]), col(pal[2]), col(pal[3]));
    let mut r = Rng(seed);
    let blobs: Vec<(f32, f32, f32, [f32; 3])> = (0..6)
        .map(|i| (r.next() * sz, r.next() * sz * 0.61, sz * (0.25 + r.next() * 0.44), if i % 2 == 1 { c } else { d }))
        .collect();
    let ribbons: Vec<(f32, f32, f32, f32, [f32; 3])> = (0..3)
        .map(|k| {
            let y0 = sz * (0.33 + k as f32 * 0.083);
            let amp = sz * (0.12 + r.next() * 0.1);
            let ph = r.next() * 6.28;
            let w = sz * (0.05 - k as f32 * 0.011);
            (y0, amp, ph, w, if k == 1 { c } else { d })
        })
        .collect();
    let mut img = image::RgbaImage::new(N, N);
    for y in 0..N {
        for x in 0..N {
            let (fx, fy) = (x as f32, y as f32);
            let mut p = mix(a, b, ((fx + fy) / (2.0 * sz)).clamp(0.0, 1.0));
            for (bx, by, rad, bc) in &blobs {
                let dist = ((fx - bx).powi(2) + (fy - by).powi(2)).sqrt() / rad;
                if dist < 1.0 {
                    p = mix(p, *bc, 0.55 * (1.0 - dist));
                }
            }
            for (y0, amp, ph, w, rc) in &ribbons {
                let cy = y0 + (fx / sz * 3.4 + ph).sin() * amp * 0.6 - (fx / sz) * sz * 0.08;
                let dd = (fy - cy).abs();
                let glow = (-(dd / (w * 2.6)).powi(2)).exp() * 0.5 + (-(dd / *w).powi(2)).exp() * 0.5;
                p = screen(p, *rc, glow.min(1.0) * 0.8);
            }
            // mountains
            let ridge = sz * 0.69 + (fx * 0.083 + seed as f32).sin() * sz * 0.06
                - if fx > sz / 3.0 && fx < sz * 2.0 / 3.0 { sz * 0.11 * ((fx - sz / 3.0) / (sz / 3.0) * std::f32::consts::PI).sin() } else { 0.0 };
            if fy > ridge {
                p = mix(p, [5.0, 8.0, 20.0], 0.82);
            }
            let n = (r.next() - 0.5) * 14.0;
            img.put_pixel(x, y, image::Rgba([(p[0] + n).clamp(0.0, 255.0) as u8, (p[1] + n).clamp(0.0, 255.0) as u8, (p[2] + n).clamp(0.0, 255.0) as u8, 255]));
        }
    }
    img
}
