//! "Dynamic Island": modelled on the iPhone's Dynamic Island and Live Activities. A floating capsule 10 px below
//! the top edge with a leading / trailing layout around an empty centre; two activities split into the capsule
//! plus a detached "minimal" bubble; expanded = one Live Activity card (lock-screen style), not a dashboard.

use crate::util::PxF;
use super::parts::*;
use super::*;
use crate::hub::{Hub, LyrState};
use crate::theme::*;
use std::cell::Cell;
use std::rc::Rc;

const TOP: f32 = 10.0;
const CAP_H: f32 = 37.0;
pub const CARD_W: f32 = 420.0;
const PAD: f32 = 20.0;
const ROW_H: f32 = 34.0;
const TILES_H: f32 = 102.0;
const CARD_BG: u32 = 0x1c1c1e;
const MENU_BG: u32 = 0x2c2c2e;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum CardKind {
    Music = 0,
    Timer = 1,
    Idle = 2,
}

pub fn card_kind(v: &IslandView, h: &Hub) -> CardKind {
    match v.primary_secondary(h).0 {
        Some(Activity::Timer) => CardKind::Timer,
        Some(Activity::Music) => CardKind::Music,
        _ => {
            if h.i("media") && h.media.active {
                CardKind::Music
            } else if h.timer.running() {
                CardKind::Timer
            } else {
                CardKind::Idle
            }
        }
    }
}

fn widgets_h(v: &IslandView) -> f32 {
    ROW_H + if v.widgets_open { 10.0 + TILES_H } else { 0.0 }
}

pub fn card_h(v: &IslandView, k: CardKind) -> f32 {
    match k {
        CardKind::Music => PAD + 64.0 + 14.0 + 30.0 + 10.0 + 27.0 + 8.0 + 50.0 + 12.0 + widgets_h(v) + 16.0,
        CardKind::Timer => PAD + 76.0 + 16.0 + widgets_h(v) + 16.0,
        CardKind::Idle => PAD + 64.0 + 14.0 + TILES_H + 12.0 + ROW_H + 16.0,
    }
}

fn compact_w(mode: Mode) -> f32 {
    match mode {
        Mode::Idle => 196.0,
        Mode::Empty => 126.0,
        Mode::Music => 236.0,
        Mode::MusicLyrics => 420.0,
        Mode::Timer => 208.0,
        Mode::Privacy => 184.0,
    }
}

pub fn geo(v: &IslandView, h: &Hub) -> Geo {
    match v.view {
        View::Compact => {
            let w = compact_w(v.mode(h));
            let (w, hh) = if v.hover { (w + 8.0, CAP_H + 3.0) } else { (w, CAP_H) };
            Geo { w, h: hh, r: hh / 2.0, y: TOP }
        }
        View::Hud => Geo { w: 262.0, h: CAP_H, r: CAP_H / 2.0, y: TOP },
        View::Drop => Geo { w: banner_w(v), h: 84.0, r: 42.0, y: TOP },
        View::Exp => Geo { w: CARD_W, h: card_h(v, card_kind(v, h)), r: 44.0, y: TOP },
    }
}

fn banner_w(v: &IslandView) -> f32 {
    match v.drop.kind {
        DropKind::Notification => 400.0,
        _ => 380.0,
    }
}

pub fn render(v: &IslandView, window: &mut Window, cx: &mut Context<IslandView>) -> AnyElement {
    let g = v.geo();
    let center = v.cx_center();
    let (a_in, a_out) = v.fades();
    let rr = g.r.min(g.h / 2.0);
    let mut cap = div()
        .id("island")
        .absolute()
        .left(px(center - g.w / 2.0))
        .top(px(g.y))
        .w(px(g.w))
        .h(px(g.h))
        .rounded(px(rr))
        .bg(gpui::black())
        .overflow_hidden()
        .font_family(FONT_DISPLAY)
        .text_color(hex(TEXT))
        .on_click(cx.listener(|this, _, _, cx| {
            if this.source_menu {
                this.source_menu = false;
                cx.notify();
            } else {
                this.on_click_island(cx)
            }
        }))
        .on_scroll_wheel(cx.listener(|this, e: &ScrollWheelEvent, _, cx| {
            let d = e.delta.pixel_delta(px(16.0)).y.f();
            if d.abs() > 0.01 {
                this.on_scroll(d > 0.0, cx);
            }
        }));
    if let Some((c, _)) = v.prev {
        if a_out > 0.01 {
            cap = cap.child(content(v, c, g, a_out, false, window, cx));
        }
    }
    cap = cap.child(content(v, v.content, g, a_in, true, window, cx));

    let mut root = div().size_full().relative().child(cap);
    if let Some((bx, by, bd)) = v.bubble_geo() {
        let h = v.hub.read(cx);
        let p = v.bub.pos.clamp(0.0, 1.0);
        let inner = minimal(v, h, v.bub_activity);
        root = root.child(
            div()
                .id("bubble")
                .absolute()
                .left(px(bx - bd / 2.0))
                .top(px(by - bd / 2.0))
                .size(px(bd))
                .rounded_full()
                .bg(gpui::black())
                .overflow_hidden()
                .flex()
                .items_center()
                .justify_center()
                .cursor_pointer()
                .child(div().opacity(ease_out((p - 0.35) / 0.65)).child(inner))
                .on_click(cx.listener(|this, _, _, cx| {
                    cx.stop_propagation();
                    this.swap_primary(cx)
                })),
        );
    }
    root.into_any_element()
}

fn content(v: &IslandView, c: Content, g: Geo, alpha: f32, entering: bool, window: &mut Window, cx: &Context<IslandView>) -> AnyElement {
    let h = v.hub.read(cx);
    let (w, hh) = match c {
        Content::Compact(_) | Content::Hud => (g.w, g.h),
        Content::Drop(_) => (banner_w(v), 84.0),
        Content::Exp(k) => {
            let k = kind_of(k);
            (CARD_W, card_h(v, k))
        }
    };
    let scale_y = if entering { (1.0 - alpha) * 5.0 } else { 0.0 };
    let inner = match c {
        Content::Compact(m) => compact(v, h, m, g.w, window),
        Content::Hud => hud(h),
        Content::Drop(_) => banner(v, h),
        Content::Exp(k) => match kind_of(k) {
            CardKind::Music => music_card(v, h, window, cx),
            CardKind::Timer => timer_card(v, h, cx),
            CardKind::Idle => idle_card(v, h, cx),
        },
    };
    div()
        .absolute()
        .left(px((g.w - w) / 2.0))
        .top(px(scale_y))
        .w(px(w))
        .h(px(hh))
        .opacity(alpha)
        .child(inner)
        .into_any_element()
}

fn kind_of(k: u8) -> CardKind {
    match k {
        1 => CardKind::Timer,
        2 => CardKind::Idle,
        _ => CardKind::Music,
    }
}

// ---------------------------------------------------------------- compact: leading / trailing around an empty centre
fn compact(v: &IslandView, h: &Hub, mode: Mode, w: f32, window: &mut Window) -> AnyElement {
    let tint = tint_color(h.media.tint);
    let row = div().size_full().flex().items_center().justify_between().text_size(px(14.0)).font_weight(FontWeight::SEMIBOLD);
    match mode {
        Mode::Empty => row.into_any_element(),
        Mode::Idle => {
            let mut trailing = div().flex().items_center().gap(px(5.0)).pr(px(15.0)).text_color(hex(TEXT));
            if let Some(wx) = h.weather.as_ref().filter(|w| w.temp.is_some() && h.i("weather")) {
                trailing = trailing.child(icon(crate::util::weather_icon(wx.code, wx.is_day), 16.0, hex(TEXT))).child(format!("{}°", wx.temp.unwrap_or(0.0)));
            } else {
                let c = crate::util::christmas();
                trailing = trailing.child(icon("tree", 15.0, hex(GREEN))).child(if c.is_eve || c.is_day { "God Jul".into() } else { format!("{}d", c.days) });
            }
            let leading = div().pl(px(16.0)).child(if h.settings.get("island.time").and_then(|x| x.as_bool()) != Some(false) { crate::util::time_str() } else { String::new() });
            row.child(leading).child(trailing).into_any_element()
        }
        Mode::Music | Mode::MusicLyrics => {
            let leading = div().pl(px(6.0)).child(art(h.media.art.clone(), 25.0, 7.0));
            let trailing = div().pr(px(13.0)).child(if h.i("visualizer") {
                waveform(&v.viz_values(h, 5), 3.0, 17.0, 2.5, tint).into_any_element()
            } else {
                icon("music", 15.0, tint).into_any_element()
            });
            let mut r = row.child(leading);
            if mode == Mode::MusicLyrics {
                let (i, p) = v.lyric_now(h);
                let text = if i < 0 { "♪".to_string() } else { h.lyrics.lines[i as usize].1.clone() };
                let k = Kar { size: 13.0, weight: FontWeight(650.0), family: FONT_DISPLAY, max_w: w - 31.0 - 40.0 - 36.0, dim: wa(0.32), fill: gpui::white().into() };
                r = r.child(div().flex_1().min_w_0().flex().justify_center().px(px(12.0)).child(karaoke(window, &text, if i < 0 { 0.0 } else { p }, &k)));
            }
            r.child(trailing).into_any_element()
        }
        Mode::Timer => {
            let t = h.timer;
            let leading = div().pl(px(8.0)).child(ring(23.0, 3.2, 1.0 - t.progress(), hex(ORANGE).into(), with_alpha(hex(ORANGE), 0.28)));
            let trailing = div().pr(px(15.0)).text_color(hex(ORANGE)).child(t.label());
            row.child(leading).child(trailing).into_any_element()
        }
        Mode::Privacy => {
            let cam = h.privacy.camera;
            let col = if cam { GREEN } else { ORANGE };
            let app = if cam { h.privacy.camera_app.clone() } else { h.privacy.mic_app.clone() };
            let leading = div().pl(px(13.0)).child(icon(if cam { "camFill" } else { "micFill" }, 17.0, hex(col)));
            let trailing = div().pr(px(15.0)).max_w(px(120.0)).truncate().text_color(hex(col)).text_size(px(13.0)).child(if app.is_empty() { if cam { "Camera".to_string() } else { "Mic".to_string() } } else { app });
            row.child(leading).child(trailing).into_any_element()
        }
    }
}

/// The detached circle ("minimal" presentation) for the secondary activity.
fn minimal(v: &IslandView, h: &Hub, a: Option<Activity>) -> AnyElement {
    match a {
        Some(Activity::Music) => match &h.media.art {
            Some(img) => img_el(img.clone(), 25.0, 12.5).into_any_element(),
            None => waveform(&v.viz_values(h, 3), 3.0, 14.0, 2.0, tint_color(h.media.tint)).into_any_element(),
        },
        Some(Activity::Timer) => ring(23.0, 3.2, 1.0 - h.timer.progress(), hex(ORANGE).into(), with_alpha(hex(ORANGE), 0.28)).into_any_element(),
        Some(Activity::Privacy) => {
            let cam = h.privacy.camera;
            icon(if cam { "camFill" } else { "micFill" }, 16.0, hex(if cam { GREEN } else { ORANGE })).into_any_element()
        }
        None => div().into_any_element(),
    }
}

// ---------------------------------------------------------------- volume HUD inside the capsule
fn hud(h: &Hub) -> AnyElement {
    let muted = h.muted || h.volume == 0.0;
    let level = if h.muted { 0.0 } else { h.volume };
    div()
        .size_full()
        .flex()
        .items_center()
        .gap(px(12.0))
        .pl(px(15.0))
        .pr(px(17.0))
        .child(icon(if muted { "speakerMute" } else { "speaker" }, 18.0, hex(TEXT)))
        .child(div().flex_1().h(px(8.0)).rounded(px(4.0)).bg(wa(0.22)).overflow_hidden().child(div().h_full().rounded(px(4.0)).bg(gpui::white()).w(relative(level.clamp(0.0, 1.0)))))
        .into_any_element()
}

// ---------------------------------------------------------------- banner Live Activity (track change, notifications, …)
fn banner(v: &IslandView, h: &Hub) -> AnyElement {
    let d = &v.drop;
    let pic: AnyElement = match (&d.pic, d.dot, d.icon) {
        (Some(p), _, _) => img_el(p.clone(), 56.0, 14.0).into_any_element(),
        (None, Some(c), _) => div()
            .size(px(56.0))
            .flex_none()
            .rounded_full()
            .bg(with_alpha(hex(c), 0.2))
            .flex()
            .items_center()
            .justify_center()
            .child(icon(if c == GREEN { "camFill" } else { "micFill" }, 26.0, hex(c)))
            .into_any_element(),
        (None, None, Some((name, col))) => {
            let col = if d.kind == DropKind::Notification { BLUE } else { col };
            div().size(px(56.0)).flex_none().rounded_full().bg(with_alpha(hex(col), 0.2)).flex().items_center().justify_center().child(icon(name, 26.0, hex(col))).into_any_element()
        }
        _ => art(None, 56.0, 14.0),
    };
    let label = match d.kind {
        DropKind::Track => format!("Now Playing · {}", d.k),
        _ => d.k.clone(),
    };
    let mut row = div()
        .size_full()
        .flex()
        .items_center()
        .gap(px(13.0))
        .pl(px(14.0))
        .pr(px(24.0))
        .child(pic)
        .child(
            div()
                .flex_1()
                .min_w_0()
                .flex()
                .flex_col()
                .child(div().text_size(px(12.0)).text_color(hex(DIM_IOS)).font_weight(FontWeight::SEMIBOLD).truncate().child(label))
                .child(div().text_size(px(16.0)).font_weight(FontWeight(650.0)).truncate().child(d.t1.clone()))
                .child(div().text_size(px(14.0)).text_color(hex(DIM_IOS)).truncate().child(d.t2.clone())),
        );
    match d.kind {
        DropKind::Track if d.viz => row = row.child(waveform(&v.viz_values(h, 5), 3.5, 24.0, 3.0, tint_color(h.media.tint))),
        DropKind::TimerDone => row = row.child(div().size(px(30.0)).rounded_full().bg(hex(ORANGE)).flex().items_center().justify_center().child(icon("check", 18.0, gpui::black()))),
        _ => {}
    }
    row.into_any_element()
}

// ---------------------------------------------------------------- Live Activity cards
fn round_btn(id: &'static str, name: &'static str, icon_size: f32, d: f32, fg: Hsla, bg: Option<Hsla>) -> Stateful<Div> {
    div()
        .id(id)
        .size(px(d))
        .flex_none()
        .rounded_full()
        .flex()
        .items_center()
        .justify_center()
        .cursor_pointer()
        .when_some(bg, |el, b| el.bg(b))
        .when(bg.is_none(), |el| el.hover(|s| s.bg(wa(0.1))))
        .child(icon(name, icon_size, fg))
}

fn music_card(v: &IslandView, h: &Hub, window: &mut Window, cx: &Context<IslandView>) -> AnyElement {
    let m = &h.media;
    let tint = tint_color(m.tint);
    let pos = m.pos();
    let frac = if m.duration > 0.0 { (pos / m.duration) as f32 } else { 0.0 };

    // header: art · titles + source · waveform
    let multi = m.sources.len() >= 2 && h.i("media");
    let app_name = if m.app.is_empty() { "Media".to_string() } else { m.app.clone() };
    let source = div()
        .id("source")
        .flex()
        .items_center()
        .gap(px(3.0))
        .mt(px(3.0))
        .text_size(px(12.5))
        .font_weight(FontWeight(650.0))
        .text_color(tint)
        .when(multi, |d| d.cursor_pointer().px(px(8.0)).py(px(2.0)).ml(px(-2.0)).rounded(px(9.0)).bg(with_alpha(tint, 0.16)))
        .child(app_name)
        .when(multi, |d| d.child(icon(if v.source_menu { "chevUp" } else { "chevDown" }, 12.0, tint)))
        .on_click(cx.listener(move |this, _, _, cx| {
            cx.stop_propagation();
            if multi {
                this.source_menu = !this.source_menu;
                cx.notify();
            }
        }));
    let header = div()
        .flex()
        .items_start()
        .gap(px(14.0))
        .h(px(64.0))
        .child(
            div()
                .relative()
                .size(px(64.0))
                .flex_none()
                .child(div().absolute().top(px(10.0)).left(px(6.0)).size(px(52.0)).rounded(px(14.0)).shadow(vec![BoxShadow {
                    color: with_alpha(tint, 0.45),
                    offset: point(px(0.), px(6.)),
                    blur_radius: px(18.),
                    spread_radius: px(0.),
                }]))
                .child(div().absolute().top_0().left_0().child(art(m.art.clone(), 64.0, 14.0))),
        )
        .child(
            div()
                .flex_1()
                .min_w_0()
                .flex()
                .flex_col()
                .child(div().text_size(px(17.0)).font_weight(FontWeight(650.0)).truncate().child(m.title.clone()))
                .child(div().text_size(px(15.0)).text_color(hex(DIM_IOS)).truncate().child(m.artist.clone()))
                .child(div().flex().child(source)),
        )
        .child(div().pt(px(4.0)).child(if m.playing && h.i("visualizer") {
            waveform(&v.viz_values(h, 6), 3.5, 26.0, 3.0, tint).into_any_element()
        } else {
            waveform(&[0.15; 6], 3.5, 26.0, 3.0, with_alpha(tint, 0.6)).into_any_element()
        }));

    // karaoke line area
    let lyric: AnyElement = if h.lyrics_on() && h.lyrics.key == m.key() {
        match h.lyrics.state {
            LyrState::Synced => {
                let (i, p) = v.lyric_now(h);
                let text = if i < 0 { "♪".to_string() } else { h.lyrics.lines[i as usize].1.clone() };
                let k = Kar { size: 18.0, weight: FontWeight::BOLD, family: FONT_DISPLAY, max_w: CARD_W - 2.0 * PAD, dim: wa(0.3), fill: gpui::white().into() };
                karaoke(window, &text, if i < 0 { 0.0 } else { p }, &k).into_any_element()
            }
            LyrState::Loading => div().text_size(px(14.0)).text_color(hex(DIM_IOS)).child("Looking up lyrics…").into_any_element(),
            LyrState::Instrumental => div().text_size(px(14.0)).text_color(hex(DIM_IOS)).child("♪  Instrumental").into_any_element(),
            LyrState::Plain => div().text_size(px(14.0)).text_color(hex(DIM2)).truncate().child("Lyrics aren't time-synced for this song").into_any_element(),
            _ => div().text_size(px(14.0)).text_color(hex(DIM2)).truncate().child(if m.album.is_empty() { "No synced lyrics for this one".to_string() } else { m.album.clone() }).into_any_element(),
        }
    } else {
        div().text_size(px(14.0)).text_color(hex(DIM2)).truncate().child(m.album.clone()).into_any_element()
    };

    // progress
    let track_b = Rc::new(Cell::new(Bounds::default()));
    let tb = track_b.clone();
    let progress = div()
        .flex()
        .flex_col()
        .child(
            div()
                .id("ios-track")
                .relative()
                .h(px(7.0))
                .cursor_pointer()
                .child(super::notch::probe(track_b))
                .child(div().size_full().rounded(px(3.5)).bg(wa(0.2)).overflow_hidden().child(div().h_full().rounded(px(3.5)).bg(wa(0.94)).w(relative(frac.clamp(0.0, 1.0)))))
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(move |this, e: &MouseDownEvent, _, cx| {
                        cx.stop_propagation();
                        let b = tb.get();
                        if b.size.width.f() > 0.0 {
                            let f = ((e.position.x - b.origin.x).f() / b.size.width.f()).clamp(0.0, 1.0) as f64;
                            this.hub.update(cx, |h, cx| {
                                let d = h.media.duration;
                                if d > 0.0 {
                                    h.seek(f * d, cx)
                                }
                            });
                        }
                    }),
                ),
        )
        .child(
            div()
                .flex()
                .justify_between()
                .mt(px(6.0))
                .h(px(14.0))
                .text_size(px(12.0))
                .font_weight(FontWeight::MEDIUM)
                .text_color(hex(DIM_IOS))
                .child(crate::util::clock(pos))
                .child(if m.duration > 0.0 { format!("-{}", crate::util::clock(m.duration - pos)) } else { String::new() }),
        );

    let action = |id: &'static str, name: &'static str, isz: f32, enabled: bool, act: &'static str| {
        round_btn(id, name, isz, 50.0, if enabled { hex(TEXT).into() } else { wa(0.3) }, None).on_click(cx.listener(move |this, _, _, cx| {
            cx.stop_propagation();
            if enabled {
                this.hub.update(cx, |h, cx| h.media_cmd(act, cx));
            }
        }))
    };
    let controls = div()
        .h(px(50.0))
        .flex()
        .items_center()
        .justify_center()
        .gap(px(40.0))
        .child(action("ios-prev", "back", 30.0, m.can_prev, "prev"))
        .child(action("ios-play", if m.playing { "pause" } else { "play" }, 40.0, true, "toggle"))
        .child(action("ios-next", "fwd", 30.0, m.can_next, "next"));

    let mut card = div()
        .relative()
        .size_full()
        .px(px(PAD))
        .pt(px(PAD))
        .flex()
        .flex_col()
        .child(header)
        .child(div().h(px(30.0)).mt(px(14.0)).flex().items_center().child(lyric))
        .child(div().mt(px(10.0)).child(progress))
        .child(div().mt(px(8.0)).child(controls))
        .child(div().mt(px(12.0)).child(widgets(v, h, cx)));
    if v.source_menu && multi {
        card = card.child(source_menu(h, cx));
    }
    card.into_any_element()
}

/// iOS-style context menu listing every player, anchored under the source button.
fn source_menu(h: &Hub, cx: &Context<IslandView>) -> AnyElement {
    let list = &h.media.sources;
    let mut menu = div()
        .id("source-menu")
        .absolute()
        .left(px(PAD + 64.0 + 14.0 - 4.0))
        .top(px(PAD + 64.0 + 2.0))
        .w(px(250.0))
        .rounded(px(16.0))
        .bg(hex(MENU_BG))
        .border_1()
        .border_color(wa(0.08))
        .shadow(vec![BoxShadow { color: ba(0.55), offset: point(px(0.), px(12.)), blur_radius: px(30.), spread_radius: px(0.) }])
        .overflow_hidden()
        .flex()
        .flex_col()
        .on_click(|_, _, cx| cx.stop_propagation())
        .child(div().px(px(14.0)).pt(px(10.0)).pb(px(6.0)).text_size(px(12.0)).text_color(hex(DIM_IOS)).font_weight(FontWeight::SEMIBOLD).child("Control playback on"));
    for (i, s) in list.iter().enumerate() {
        let id = s.id.clone();
        let sel = s.selected;
        menu = menu.child(
            div()
                .id(("src-row", i))
                .h(px(46.0))
                .px(px(14.0))
                .flex()
                .items_center()
                .gap(px(10.0))
                .border_t_1()
                .border_color(wa(0.07))
                .cursor_pointer()
                .hover(|st| st.bg(wa(0.06)))
                .child(div().w(px(16.0)).flex_none().child(if sel { icon("check", 15.0, hex(BLUE)).into_any_element() } else { div().into_any_element() }))
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .flex()
                        .flex_col()
                        .child(div().flex().items_center().gap(px(6.0)).child(div().text_size(px(14.0)).font_weight(FontWeight::SEMIBOLD).child(s.app.clone())).when(s.playing, |d| d.child(dot(GREEN, 6.0))))
                        .child(div().text_size(px(12.0)).text_color(hex(DIM_IOS)).truncate().child(s.title.clone())),
                )
                .on_click(cx.listener(move |this, _, _, cx| {
                    cx.stop_propagation();
                    this.source_menu = false;
                    if !sel {
                        let id = id.clone();
                        this.hub.update(cx, |h, cx| h.select_source(&id, cx));
                    }
                    cx.notify();
                })),
        );
    }
    menu.into_any_element()
}

fn timer_card(v: &IslandView, h: &Hub, cx: &Context<IslandView>) -> AnyElement {
    let t = h.timer;
    let running = t.running();
    let buttons = div()
        .flex()
        .gap(px(12.0))
        .child(
            round_btn("t-pause", if t.is_paused() { "play" } else { "pause" }, 24.0, 56.0, hex(ORANGE).into(), Some(with_alpha(hex(ORANGE), 0.24))).on_click(cx.listener(|this, _, _, cx| {
                cx.stop_propagation();
                this.hub.update(cx, |h, cx| h.timer_toggle(cx))
            })),
        )
        .child(
            round_btn("t-cancel", "x", 22.0, 56.0, hex(TEXT).into(), Some(hex(0x3a3a3c).into())).on_click(cx.listener(|this, _, _, cx| {
                cx.stop_propagation();
                this.hub.update(cx, |h, cx| h.timer_reset(cx))
            })),
        );
    let right = div()
        .flex()
        .flex_col()
        .items_end()
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(6.0))
                .text_size(px(14.0))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(with_alpha(hex(ORANGE), 0.9))
                .child(ring(14.0, 2.4, 1.0 - t.progress(), hex(ORANGE).into(), with_alpha(hex(ORANGE), 0.3)))
                .child(if t.is_paused() { "Paused" } else { "Timer" }),
        )
        .child(div().text_size(px(50.0)).line_height(px(56.0)).font_weight(FontWeight::LIGHT).text_color(hex(ORANGE)).child(if running { t.label() } else { "0:00".into() }));
    div()
        .size_full()
        .px(px(PAD))
        .pt(px(PAD))
        .flex()
        .flex_col()
        .child(div().h(px(76.0)).flex().items_center().justify_between().child(buttons).child(right))
        .child(div().mt(px(16.0)).child(widgets(v, h, cx)))
        .into_any_element()
}

fn idle_card(v: &IslandView, h: &Hub, cx: &Context<IslandView>) -> AnyElement {
    let mut top = div()
        .h(px(64.0))
        .flex()
        .items_center()
        .justify_between()
        .child(
            div()
                .flex()
                .flex_col()
                .child(div().text_size(px(46.0)).line_height(px(48.0)).font_weight(FontWeight::LIGHT).child(crate::util::time_str()))
                .child(div().text_size(px(13.0)).text_color(hex(DIM_IOS)).font_weight(FontWeight::MEDIUM).child(crate::util::date_str())),
        );
    if h.i("privacy") && (h.privacy.camera || h.privacy.mic) {
        let cam = h.privacy.camera;
        let c = if cam { GREEN } else { ORANGE };
        top = top.child(
            div()
                .flex()
                .items_center()
                .gap(px(6.0))
                .px(px(10.0))
                .h(px(28.0))
                .rounded(px(14.0))
                .bg(with_alpha(hex(c), 0.16))
                .text_color(hex(c))
                .text_size(px(12.5))
                .font_weight(FontWeight::SEMIBOLD)
                .child(icon(if cam { "camFill" } else { "micFill" }, 14.0, hex(c)))
                .child(if cam { "Camera in use" } else { "Mic in use" }),
        );
    }
    let row = div()
        .id("np-row")
        .h(px(ROW_H))
        .rounded(px(ROW_H / 2.0))
        .bg(hex(CARD_BG))
        .px(px(12.0))
        .flex()
        .items_center()
        .gap(px(8.0))
        .text_size(px(13.0))
        .text_color(hex(DIM_IOS))
        .child(icon("music", 15.0, hex(DIM_IOS)))
        .child(div().flex_1().child("Nothing playing"))
        .child(gear(cx));
    div()
        .size_full()
        .px(px(PAD))
        .pt(px(PAD))
        .flex()
        .flex_col()
        .child(top)
        .child(div().mt(px(14.0)).child(tiles(v, h, cx)))
        .child(div().mt(px(12.0)).child(row))
        .into_any_element()
}

fn gear(cx: &Context<IslandView>) -> Stateful<Div> {
    div()
        .id("ios-gear")
        .size(px(26.0))
        .flex_none()
        .rounded_full()
        .flex()
        .items_center()
        .justify_center()
        .cursor_pointer()
        .hover(|s| s.bg(wa(0.1)))
        .child(icon("gear", 15.0, hex(DIM_IOS)))
        .on_click(cx.listener(|this, _, _, cx| {
            cx.stop_propagation();
            this.open_settings(cx)
        }))
}

/// Secondary content: a summary row that expands into widget tiles.
fn widgets(v: &IslandView, h: &Hub, cx: &Context<IslandView>) -> AnyElement {
    let open = v.widgets_open;
    let mut summary = div().flex().items_center().gap(px(12.0)).flex_1().min_w_0().text_size(px(13.0)).font_weight(FontWeight::SEMIBOLD);
    if h.i("weather") {
        if let Some(w) = h.weather.as_ref().filter(|w| w.temp.is_some()) {
            summary = summary.child(div().flex().items_center().gap(px(5.0)).child(icon(crate::util::weather_icon(w.code, w.is_day), 15.0, hex(0x64D2FF))).child(format!("{}°", w.temp.unwrap_or(0.0))));
        }
    }
    if h.i("system") {
        let s = h.stats.clone().unwrap_or_default();
        let mini = |val: f64, col: u32, label: &'static str| {
            div()
                .flex()
                .items_center()
                .gap(px(5.0))
                .child(ring(15.0, 2.6, (val / 100.0) as f32, hex(col).into(), with_alpha(hex(col), 0.25)))
                .child(div().text_color(hex(DIM_IOS)).child(label))
                .child(format!("{}%", val.round()))
        };
        summary = summary.child(mini(s.cpu, PINK, "CPU")).child(mini(s.ram, GREEN, "RAM"));
    }
    if h.timer.running() && card_kind(v, h) != CardKind::Timer {
        summary = summary.child(div().flex().items_center().gap(px(5.0)).text_color(hex(ORANGE)).child(icon("clock", 14.0, hex(ORANGE))).child(h.timer.label()));
    }
    let row = div()
        .id("widgets-row")
        .h(px(ROW_H))
        .rounded(px(ROW_H / 2.0))
        .bg(hex(CARD_BG))
        .pl(px(14.0))
        .pr(px(4.0))
        .flex()
        .items_center()
        .gap(px(6.0))
        .cursor_pointer()
        .hover(|s| s.bg(hex(0x242426)))
        .child(summary)
        .child(icon(if open { "chevUp" } else { "chevDown" }, 14.0, hex(DIM_IOS)))
        .child(gear(cx))
        .on_click(cx.listener(|this, _, _, cx| {
            cx.stop_propagation();
            this.widgets_open = !this.widgets_open;
            cx.notify();
        }));
    let mut col = div().flex().flex_col().child(row);
    if open {
        col = col.child(div().mt(px(10.0)).child(tiles(v, h, cx)));
    }
    col.into_any_element()
}

fn sky(code: i64, day: bool) -> (u32, u32) {
    let kind = crate::util::wmo(code).1;
    match (kind, day) {
        ("partly" | "cloud", true) => (0x4b6a8c, 0x8ba7c2),
        ("partly" | "cloud", false) => (0x1d2633, 0x44536a),
        ("rain" | "storm", _) => (0x26364c, 0x4e6280),
        ("snow" | "sleet", _) => (0x5c7b9c, 0xa9c0d8),
        ("fog", _) => (0x545f69, 0x8c98a1),
        (_, false) => (0x0b1734, 0x2b3f74),
        _ => (0x1d6fd0, 0x58abf0),
    }
}

fn tiles(v: &IslandView, h: &Hub, cx: &Context<IslandView>) -> AnyElement {
    let tile = || div().flex_1().min_w_0().h(px(TILES_H)).rounded(px(22.0)).p(px(12.0)).flex().flex_col();
    let mut row = div().flex().gap(px(10.0));
    // weather: a sky that matches the conditions
    if h.i("weather") {
        row = row.child(match h.weather.as_ref().filter(|w| w.temp.is_some()) {
            Some(w) => {
                let (a, b) = sky(w.code, w.is_day);
                tile()
                    .bg(linear_gradient(170.0, linear_color_stop(hex(a), 0.0), linear_color_stop(hex(b), 1.0)))
                    .text_color(gpui::white())
                    .child(
                        div()
                            .flex()
                            .justify_between()
                            .items_center()
                            .child(div().text_size(px(13.0)).font_weight(FontWeight(650.0)).truncate().child(w.city.split(',').next().unwrap_or("Weather").to_string()))
                            .child(icon(crate::util::weather_icon(w.code, w.is_day), 17.0, gpui::white())),
                    )
                    .child(div().text_size(px(34.0)).line_height(px(40.0)).font_weight(FontWeight::LIGHT).child(format!("{}°", w.temp.unwrap_or(0.0))))
                    .child(div().mt_auto().text_size(px(11.0)).font_weight(FontWeight::MEDIUM).text_color(wa(0.85)).truncate().child(format!(
                        "{} · H{}° L{}°",
                        crate::util::wmo(w.code).0,
                        w.hi.unwrap_or(0.0),
                        w.lo.unwrap_or(0.0)
                    )))
            }
            None => tile()
                .bg(linear_gradient(170.0, linear_color_stop(hex(0x1d6fd0), 0.0), linear_color_stop(hex(0x58abf0), 1.0)))
                .child(div().text_size(px(13.0)).font_weight(FontWeight(650.0)).child("Weather"))
                .child(div().mt_auto().text_size(px(11.5)).text_color(wa(0.85)).child("Set a city in Settings")),
        });
    }
    // system: activity-style rings
    if h.i("system") {
        let s = h.stats.clone().unwrap_or_default();
        let has = h.stats.is_some();
        let legend = |col: u32, name: &'static str, val: f64| {
            div()
                .flex()
                .items_center()
                .gap(px(4.0))
                .text_size(px(11.0))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(hex(DIM_IOS))
                .child(div().size(px(7.0)).rounded_full().bg(hex(col)))
                .when(false, |d| d.child(name))
                .child(div().text_color(gpui::white()).child(if has { format!("{}%", val.round()) } else { "–".into() }))
        };
        row = row.child(
            tile()
                .bg(hex(CARD_BG))
                .items_center()
                .justify_center()
                .gap(px(6.0))
                .child(rings(
                    54.0,
                    7.0,
                    vec![
                        (23.5, (s.cpu / 100.0) as f32, hex(PINK).into(), with_alpha(hex(PINK), 0.22)),
                        (15.0, (s.ram / 100.0) as f32, hex(GREEN).into(), with_alpha(hex(GREEN), 0.22)),
                    ],
                ))
                .child(div().flex().gap(px(10.0)).child(legend(PINK, "CPU", s.cpu)).child(legend(GREEN, "RAM", s.ram))),
        );
    }
    // timer: quick start, or the running countdown (tap → make it the main activity)
    let t = h.timer;
    let mut timer = tile().bg(hex(CARD_BG)).child(
        div().flex().items_center().gap(px(6.0)).text_size(px(13.0)).font_weight(FontWeight(650.0)).child(icon("clock", 15.0, hex(ORANGE))).child("Timer"),
    );
    if t.running() {
        timer = timer
            .child(div().mt_auto().text_size(px(28.0)).font_weight(FontWeight::LIGHT).text_color(hex(ORANGE)).child(t.label()))
            .child(div().text_size(px(11.0)).text_color(hex(DIM_IOS)).child(if t.is_paused() { "Paused" } else { "Running" }));
        let timer = timer.id("timer-tile").cursor_pointer().on_click(cx.listener(|this, _, _, cx| {
            cx.stop_propagation();
            this.primary = Some(Activity::Timer);
            cx.notify();
        }));
        row = row.child(timer);
    } else {
        let chip = |m: f64| {
            div()
                .id(("chip", m as usize))
                .flex_1()
                .h(px(28.0))
                .rounded(px(14.0))
                .bg(with_alpha(hex(ORANGE), 0.2))
                .hover(|s| s.bg(with_alpha(hex(ORANGE), 0.32)))
                .flex()
                .items_center()
                .justify_center()
                .text_size(px(12.0))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(hex(ORANGE))
                .cursor_pointer()
                .child(format!("{}", m))
                .on_click(cx.listener(move |this, _, _, cx| {
                    cx.stop_propagation();
                    this.primary = Some(Activity::Timer);
                    this.hub.update(cx, |h, cx| h.timer_start(m, cx));
                }))
        };
        timer = timer
            .child(div().mt(px(4.0)).text_size(px(11.0)).text_color(hex(DIM_IOS)).child("Minutes"))
            .child(div().mt_auto().flex().gap(px(5.0)).child(chip(1.0)).child(chip(5.0)).child(chip(25.0)));
        row = row.child(timer);
    }
    let _ = v;
    row.into_any_element()
}
