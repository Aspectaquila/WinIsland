//! Now-playing info + transport controls via Windows' Global System Media Transport Controls (GSMTC).
//! Event-driven: WinRT events only wake a worker thread, which reads the session and posts a snapshot to the UI.
//! Every session (Spotify, each browser, …) is tracked so the island can offer a source switcher; the one
//! being controlled is the user's pick, or Windows' "current" session when there is no pick.

use crate::model::{post, Event, MediaInfo, Source};
use std::collections::HashMap;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Arc, OnceLock};
use windows::core::HSTRING;
use windows::Foundation::TypedEventHandler;
use windows::Media::Control::{
    GlobalSystemMediaTransportControlsSession as Session, GlobalSystemMediaTransportControlsSessionManager as Manager,
    GlobalSystemMediaTransportControlsSessionPlaybackStatus as Status,
};
use windows::Storage::Streams::{DataReader, IRandomAccessStreamReference};

pub enum Cmd {
    Refresh,
    SessionsChanged,
    CurrentChanged,
    Select(String),
    Action(String),
    Seek(f64),
}

static TX: OnceLock<Sender<Cmd>> = OnceLock::new();

pub fn send(c: Cmd) {
    if let Some(tx) = TX.get() {
        let _ = tx.send(c);
    }
}

pub fn start() {
    let (tx, rx) = channel();
    let _ = TX.set(tx.clone());
    std::thread::Builder::new()
        .name("media".into())
        .spawn(move || {
            unsafe {
                let _ = windows::Win32::System::Com::CoInitializeEx(None, windows::Win32::System::Com::COINIT_MULTITHREADED);
            }
            if let Err(e) = run(tx, rx) {
                crate::model::log(&format!("media: {e:?}"));
            }
        })
        .ok();
}

struct Watched {
    s: Session,
    props: i64,
    playback: i64,
}

struct State {
    mgr: Manager,
    tx: Sender<Cmd>,
    watched: Vec<Watched>,
    session: Option<Session>,
    session_id: Option<String>,
    timeline_token: Option<i64>,
    pinned: Option<String>,
    last_art_key: String,
    last_art: Option<Arc<gpui::RenderImage>>,
    last_tint: Option<(f32, f32, f32)>,
    gecko_name: Option<String>,
}

fn run(tx: Sender<Cmd>, rx: Receiver<Cmd>) -> windows::core::Result<()> {
    let mgr = Manager::RequestAsync()?.get()?;
    {
        let t = tx.clone();
        mgr.CurrentSessionChanged(&TypedEventHandler::new(move |_, _| {
            let _ = t.send(Cmd::CurrentChanged);
            Ok(())
        }))?;
        let t = tx.clone();
        mgr.SessionsChanged(&TypedEventHandler::new(move |_, _| {
            let _ = t.send(Cmd::SessionsChanged);
            Ok(())
        }))?;
    }
    let mut st = State {
        mgr,
        tx,
        watched: vec![],
        session: None,
        session_id: None,
        timeline_token: None,
        pinned: None,
        last_art_key: String::from("\u{0}"),
        last_art: None,
        last_tint: None,
        gecko_name: None,
    };
    st.watch();
    st.attach();
    st.refresh();
    while let Ok(first) = rx.recv() {
        // coalesce bursts (Spotify fires several events per track change)
        let mut cmds = vec![first];
        std::thread::sleep(std::time::Duration::from_millis(40));
        while let Ok(c) = rx.try_recv() {
            cmds.push(c);
        }
        let mut refresh = false;
        for c in cmds {
            match c {
                Cmd::Refresh => refresh = true,
                Cmd::SessionsChanged => {
                    st.watch();
                    st.attach();
                    refresh = true;
                }
                Cmd::CurrentChanged => {
                    st.pinned = None;
                    st.attach();
                    refresh = true;
                }
                Cmd::Select(id) => {
                    if st.sessions().iter().any(|(i, _)| *i == id) {
                        st.pinned = Some(id);
                        st.attach();
                        refresh = true;
                    }
                }
                Cmd::Action(a) => st.action(&a),
                Cmd::Seek(s) => st.seek(s),
            }
        }
        if refresh {
            st.refresh();
        }
    }
    Ok(())
}

impl State {
    /// Ids are "aumid#n" (n = occurrence), so two tabs of the same browser stay distinguishable.
    fn sessions(&self) -> Vec<(String, Session)> {
        let mut out = vec![];
        let mut seen: HashMap<String, usize> = HashMap::new();
        if let Ok(list) = self.mgr.GetSessions() {
            for s in list {
                let a = s.SourceAppUserModelId().map(|h| h.to_string()).unwrap_or_default();
                let n = seen.entry(a.clone()).or_insert(0);
                *n += 1;
                out.push((format!("{a}#{n}"), s));
            }
        }
        out
    }

    fn watch(&mut self) {
        for w in self.watched.drain(..) {
            let _ = w.s.RemoveMediaPropertiesChanged(w.props);
            let _ = w.s.RemovePlaybackInfoChanged(w.playback);
        }
        for (_, s) in self.sessions() {
            let t1 = self.tx.clone();
            let t2 = self.tx.clone();
            let props = s
                .MediaPropertiesChanged(&TypedEventHandler::new(move |_, _| {
                    let _ = t1.send(Cmd::Refresh);
                    Ok(())
                }))
                .unwrap_or(0);
            let playback = s
                .PlaybackInfoChanged(&TypedEventHandler::new(move |_, _| {
                    let _ = t2.send(Cmd::Refresh);
                    Ok(())
                }))
                .unwrap_or(0);
            self.watched.push(Watched { s, props, playback });
        }
    }

    fn attach(&mut self) {
        let all = self.sessions();
        let mut s: Option<Session> = None;
        if let Some(p) = &self.pinned {
            s = all.iter().find(|(id, _)| id == p).map(|(_, s)| s.clone());
        }
        if s.is_none() {
            self.pinned = None;
            s = self.mgr.GetCurrentSession().ok();
        }
        let id = s.as_ref().and_then(|s| id_of(s, &all));
        if s.is_some() && self.session.is_some() && id == self.session_id {
            return;
        }
        if let (Some(old), Some(tok)) = (&self.session, self.timeline_token.take()) {
            let _ = old.RemoveTimelinePropertiesChanged(tok);
        }
        self.session = s;
        self.session_id = id;
        if let Some(s) = &self.session {
            let t = self.tx.clone();
            self.timeline_token = s
                .TimelinePropertiesChanged(&TypedEventHandler::new(move |_, _| {
                    let _ = t.send(Cmd::Refresh);
                    Ok(())
                }))
                .ok();
        }
    }

    fn friendly_app(&mut self, aumid: &str) -> String {
        if aumid.is_empty() {
            return String::new();
        }
        if is_gecko_hash(aumid) {
            if self.gecko_name.is_none() {
                let found = crate::util::process_running(&["zen", "firefox", "librewolf", "floorp", "waterfox"]);
                self.gecko_name = Some(match found.as_deref() {
                    Some("zen") => "Zen",
                    Some("firefox") => "Firefox",
                    Some("librewolf") => "LibreWolf",
                    Some("floorp") => "Floorp",
                    Some("waterfox") => "Waterfox",
                    _ => "Browser",
                }
                .to_string());
            }
            return self.gecko_name.clone().unwrap_or_default();
        }
        friendly_app(aumid)
    }

    fn refresh(&mut self) {
        let mut m = MediaInfo::default();
        if let Some(s) = self.session.clone() {
            let props = s.TryGetMediaPropertiesAsync().and_then(|op| op.get()).ok();
            let pb = s.GetPlaybackInfo().ok();
            let tl = s.GetTimelineProperties().ok();
            let title = props.as_ref().and_then(|p| p.Title().ok()).map(|h| h.to_string()).unwrap_or_default();
            let artist = props.as_ref().and_then(|p| p.Artist().ok()).map(|h| h.to_string()).unwrap_or_default();
            let album = props.as_ref().and_then(|p| p.AlbumTitle().ok()).map(|h| h.to_string()).unwrap_or_default();
            let aumid = s.SourceAppUserModelId().map(|h| h.to_string()).unwrap_or_default();
            m.active = !title.is_empty();
            m.app = self.friendly_app(&aumid);
            m.browser = is_browser(&aumid);
            m.playing = pb.as_ref().and_then(|p| p.PlaybackStatus().ok()) == Some(Status::Playing);
            if let Some(c) = pb.as_ref().and_then(|p| p.Controls().ok()) {
                m.can_prev = c.IsPreviousEnabled().unwrap_or(false);
                m.can_next = c.IsNextEnabled().unwrap_or(false);
            }
            if let Some(tl) = &tl {
                let pos = tl.Position().map(|t| t.Duration).unwrap_or(0);
                let start = tl.StartTime().map(|t| t.Duration).unwrap_or(0);
                let end = tl.EndTime().map(|t| t.Duration).unwrap_or(0);
                m.position = pos as f64 / 1e7;
                m.duration = ((end - start) as f64 / 1e7).max(0.0);
                let upd = tl.LastUpdatedTime().map(|t| t.UniversalTime).unwrap_or(0);
                m.pos_stamp = if upd > 0 { (upd - 116_444_736_000_000_000) / 10_000 } else { crate::model::now_ms() };
            }
            let key = format!("{title}|{artist}|{album}");
            if key != self.last_art_key {
                self.last_art_key = key;
                let thumb = props.as_ref().and_then(|p| p.Thumbnail().ok());
                match thumb.and_then(|t| read_thumb(&t)) {
                    Some((img, tint)) => {
                        self.last_art = Some(img);
                        self.last_tint = Some(tint);
                    }
                    None => {
                        self.last_art = None;
                        self.last_tint = None;
                    }
                }
            }
            m.title = title;
            m.artist = artist;
            m.album = album;
            m.art = self.last_art.clone();
            m.tint = self.last_tint;
        }
        m.sources = self.sources();
        post(Event::Media(m));
    }

    fn sources(&mut self) -> Vec<Source> {
        let mut out = vec![];
        let sel = self.session_id.clone();
        for (id, s) in self.sessions() {
            let Ok(p) = s.TryGetMediaPropertiesAsync().and_then(|op| op.get()) else { continue };
            let title = p.Title().map(|h| h.to_string()).unwrap_or_default();
            if title.is_empty() {
                continue;
            }
            let aumid = s.SourceAppUserModelId().map(|h| h.to_string()).unwrap_or_default();
            let app = self.friendly_app(&aumid);
            out.push(Source {
                selected: Some(&id) == sel.as_ref(),
                id,
                app: if app.is_empty() { "Media".into() } else { app },
                browser: is_browser(&aumid),
                title,
                artist: p.Artist().map(|h| h.to_string()).unwrap_or_default(),
                playing: s.GetPlaybackInfo().and_then(|i| i.PlaybackStatus()).ok() == Some(Status::Playing),
            });
        }
        out
    }

    fn seek(&self, seconds: f64) {
        let Some(s) = &self.session else { return };
        if let Ok(tl) = s.GetTimelineProperties() {
            let start = tl.StartTime().map(|t| t.Duration).unwrap_or(0);
            let ticks = start + (seconds.max(0.0) * 1e7) as i64;
            if let Ok(op) = s.TryChangePlaybackPositionAsync(ticks) {
                let _ = op.get();
            }
        }
        let _ = self.tx.send(Cmd::Refresh);
    }

    fn action(&self, a: &str) {
        let Some(s) = &self.session else { return };
        let r = match a {
            "toggle" => s.TryTogglePlayPauseAsync(),
            "next" => s.TrySkipNextAsync(),
            "prev" => s.TrySkipPreviousAsync(),
            _ => return,
        };
        if let Ok(op) = r {
            let _ = op.get();
        }
    }
}

fn id_of(s: &Session, all: &[(String, Session)]) -> Option<String> {
    for (id, x) in all {
        if x == s {
            return Some(id.clone());
        }
    }
    let a = s.SourceAppUserModelId().ok()?;
    all.iter().find(|(_, x)| x.SourceAppUserModelId().ok().as_ref() == Some(&a)).map(|(id, _)| id.clone())
}

/// Firefox-family browsers register a 16-hex-digit hash of their install folder.
fn is_gecko_hash(a: &str) -> bool {
    a.len() == 16 && a.chars().all(|c| c.is_ascii_hexdigit())
}

fn is_browser(aumid: &str) -> bool {
    let a = aumid.to_lowercase();
    is_gecko_hash(aumid)
        || ["chrome", "msedge", "firefox", "zen", "brave", "opera", "vivaldi"].iter().any(|k| a.contains(k))
}

fn friendly_app(aumid: &str) -> String {
    let a = aumid.to_lowercase();
    let known = [
        ("spotify", "Spotify"),
        ("chrome", "Chrome"),
        ("msedge", "Edge"),
        ("firefox", "Firefox"),
        ("brave", "Brave"),
        ("opera", "Opera"),
        ("zen", "Browser"),
        ("zunemusic", "Media Player"),
        ("media", "Media Player"),
        ("discord", "Discord"),
        ("vlc", "VLC"),
        ("foobar", "foobar2000"),
        ("tidal", "TIDAL"),
        ("applemusic", "Apple Music"),
        ("itunes", "Apple Music"),
    ];
    for (k, v) in known {
        if a.contains(k) {
            return v.into();
        }
    }
    let mut n = aumid.rsplit(['!', '\\', '/']).next().unwrap_or("").to_string();
    if n.to_lowercase().ends_with(".exe") {
        n.truncate(n.len() - 4);
    }
    // Browsers/PWAs often register opaque hashes as their media id; don't show those.
    if (n.len() >= 10 && n.chars().all(|c| c.is_ascii_hexdigit())) || n.chars().count() > 28 || n.contains('{') {
        return String::new();
    }
    n
}

fn read_thumb(r: &IRandomAccessStreamReference) -> Option<(Arc<gpui::RenderImage>, (f32, f32, f32))> {
    let s = r.OpenReadAsync().ok()?.get().ok()?;
    let size = s.Size().ok()?;
    if size == 0 || size > 4_000_000 {
        return None;
    }
    let rd = DataReader::CreateDataReader(&s).ok()?;
    rd.LoadAsync(size as u32).ok()?.get().ok()?;
    let mut buf = vec![0u8; size as usize];
    rd.ReadBytes(&mut buf).ok()?;
    let _ = HSTRING::new();
    decode_art(&buf)
}

/// Decode an encoded image into a gpui image (BGRA) plus its tint colour.
pub fn decode_art(bytes: &[u8]) -> Option<(Arc<gpui::RenderImage>, (f32, f32, f32))> {
    let img = image::load_from_memory(bytes).ok()?;
    // album art is shown square: centre-crop video thumbnails (16:9) and the like
    let (w, h) = (img.width(), img.height());
    let s = w.min(h);
    let img = if w != h { img.crop_imm((w - s) / 2, (h - s) / 2, s, s) } else { img };
    let img = if s > 400 { img.thumbnail(360, 360) } else { img };
    Some(rgba_to_render(img.to_rgba8()))
}

pub fn rgba_to_render(mut rgba: image::RgbaImage) -> (Arc<gpui::RenderImage>, (f32, f32, f32)) {
    let tint = crate::util::tint_from_rgba(rgba.width(), rgba.height(), rgba.as_raw());
    for p in rgba.pixels_mut() {
        p.0.swap(0, 2); // RGBA → BGRA
    }
    let frame = image::Frame::new(rgba);
    (Arc::new(gpui::RenderImage::new(smallvec::smallvec![frame])), tint)
}
