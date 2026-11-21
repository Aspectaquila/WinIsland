//! "Dynamic Notch": pure black, flush with the top screen edge, concave shoulders, shelf-style dashboard with
//! Home / Lyrics / Timer tabs. A faithful port of the macOS look of the WebView2 version.

use crate::util::PxF;
use super::parts::*;
use super::*;
use crate::hub::{Hub, LyrState, LEAD};
use crate::theme::*;
use std::cell::Cell;
use std::rc::Rc;

pub const EXP_W: f32 = 640.0;
const LINE_H: f32 = 38.0;
const LYR_VIEW_H: f32 = 196.0;
const LYR_PAD: f32 = 70.0;

fn exp_h(tab: Tab) -> f32 {
    match tab {
        Tab::Home => 252.0,
        Tab::Lyrics => 304.0,
        Tab::Timer => 220.0,
    }
}

fn compact_w(v: &IslandView, h: &Hub, mode: Mode) -> f32 {
    let n = v.privacy_dots(h) as f32;
    match mode {
        Mode::Timer => 290.0 + n * 12.0,
        Mode::Music => 320.0 + n * 12.0,
        Mode::MusicLyrics => 500.0 + n * 12.0,
        Mode::Idle | Mode::Privacy => 236.0 + n * 12.0,
        Mode::Empty => {
            if n > 0.0 {
                210.0 + n * 14.0
            } else {
                200.0
            }
        }
    }
}

pub fn geo(v: &IslandView, h: &Hub) -> Geo {
    match v.view {
        View::Compact => {
            let w = compact_w(v, h, v.mode(h));
            if v.hover {
                Geo { w: w + 12.0, h: 35.0, r: 15.0, y: 0.0 }
            } else {
                Geo { w, h: 32.0, r: 14.0, y: 0.0 }
            }
        }
        View::Hud => Geo { w: 380.0, h: 32.0, r: 14.0, y: 0.0 },
        View::Drop => Geo { w: v.drop.w, h: 100.0, r: 28.0, y: 0.0 },
        View::Exp => Geo { w: EXP_W, h: exp_h(v.tab), r: 32.0, y: 0.0 },
    }
}

/// Keep the lyrics page scrolled so the current line sits at ~42 % of the view.
pub fn prepare(v: &mut IslandView, h: &Hub) {
    if v.view == View::Exp && v.tab == Tab::Lyrics && h.lyrics.state == LyrState::Synced {
        let (i, _) = v.lyric_now(h);
        if i != v.lyr_idx {
            let first = v.lyr_idx == -2;
            v.lyr_idx = i;
            let y = LYR_PAD + i.max(0) as f32 * LINE_H + LINE_H / 2.0 - LYR_VIEW_H * 0.42;
            if first {
                v.lyr_scroll.snap(y.max(0.0));
            } else {
                v.lyr_scroll.set(y.max(0.0));
            }
        }
    }
}

fn notch_path(cx: f32, g: Geo) -> Option<Path<Pixels>> {
    let (w, h) = (g.w, g.h);
    let r = g.r.min(h / 2.0).min(w / 2.0).max(0.0);
    let sh = SH.min(h);
    let (x0, x1) = (cx - w / 2.0, cx + w / 2.0);
    let mut b = PathBuilder::fill();
    b.move_to(point(px(x0 - sh), px(-2.0)));
    b.line_to(point(px(x1 + sh), px(-2.0)));
    b.line_to(point(px(x1 + sh), px(0.0)));
    // right shoulder (concave): quarter circle centred at (x1 + sh, sh)
    b.arc_to(point(px(sh), px(sh)), px(0.0), false, false, point(px(x1), px(sh)));
    b.line_to(point(px(x1), px(h - r)));
    b.arc_to(point(px(r), px(r)), px(0.0), false, true, point(px(x1 - r), px(h)));
    b.line_to(point(px(x0 + r), px(h)));
    b.arc_to(point(px(r), px(r)), px(0.0), false, true, point(px(x0), px(h - r)));
    b.line_to(point(px(x0), px(sh)));
    b.arc_to(point(px(sh), px(sh)), px(0.0), false, false, point(px(x0 - sh), px(0.0)));
    b.line_to(point(px(x0 - sh), px(-2.0)));
    b.close();
    b.translate(point(px(0.0), px(g.y)));
    b.build().ok()
}

pub fn render(v: &IslandView, window: &mut Window, cx: &Context<IslandView>) -> AnyElement {
    let g = v.geo();
    let center = v.cx_center();
    let (a_in, a_out) = v.fades();

    let shape = canvas(
        move |_, _, _| (),
        move |bounds, _, window, _| {
            if let Some(p) = notch_path(bounds.origin.x.f() + center, Geo { y: bounds.origin.y.f(), ..g }) {
                window.paint_path(p, gpui::black());
            }
        },
    )
    .absolute()
    .top_0()
    .left_0()
    .size_full();

    let mut clip = div()
        .id("island")
        .absolute()
        .left(px(center - g.w / 2.0))
        .top(px(0.0))
        .w(px(g.w))
        .h(px(g.h))
        .overflow_hidden()
        .rounded_b(px(g.r.min(g.h / 2.0)))
        .on_click(cx.listener(|this, _, _, cx| this.on_click_island(cx)))
        .on_scroll_wheel(cx.listener(|this, e: &ScrollWheelEvent, _, cx| {
            let d = e.delta.pixel_delta(px(16.0)).y.f();
            if d.abs() > 0.01 {
                this.on_scroll(d > 0.0, cx);
            }
        }));
    if let Some((c, at)) = v.prev {
        let _ = at;
        if a_out > 0.01 {
            clip = clip.child(content(v, c, g, a_out, false, window, cx));
        }
    }
    clip = clip.child(content(v, v.content, g, a_in, true, window, cx));

    div().size_full().relative().child(shape).child(clip).into_any_element()
}

fn content(v: &IslandView, c: Content, g: Geo, alpha: f32, entering: bool, window: &mut Window, cx: &Context<IslandView>) -> AnyElement {
    let (w, h) = match c {
        Content::Compact(_) | Content::Hud => (g.w, g.h),
        Content::Drop(_) => (v.drop.w, 100.0),
        Content::Exp(t) => (EXP_W, exp_h(tab_of(t))),
    };
    let dy = if entering { (1.0 - alpha) * 4.0 } else { 0.0 };
    let inner = match c {
        Content::Compact(m) => compact(v, m, w, window, cx),
        Content::Hud => hud(v, cx),
        Content::Drop(_) => drop_view(v, cx),
        Content::Exp(t) => expanded(v, tab_of(t), window, cx),
    };
    div()
        .absolute()
        .left(px((g.w - w) / 2.0))
        .top(px(dy))
        .w(px(w))
        .h(px(h))
        .opacity(alpha)
        .text_color(hex(TEXT))
        .font_family(FONT_TEXT)
        .child(inner)
        .into_any_element()
}

fn tab_of(t: u8) -> Tab {
    match t {
        1 => Tab::Lyrics,
        2 => Tab::Timer,
        _ => Tab::Home,
    }
}

fn dots(v: &IslandView, h: &Hub) -> Vec<Div> {
    let mut out = vec![];
    if v.privacy_dots(h) > 0 {
        if h.privacy.camera {
            out.push(dot(GREEN, 6.0));
        }
        if h.privacy.mic {
            out.push(dot(ORANGE, 6.0));
        }
    }
    out
}

// ---------------------------------------------------------------- compact (idle + live activities)
fn compact(v: &IslandView, mode: Mode, w: f32, window: &mut Window, cx: &Context<IslandView>) -> AnyElement {
    let h = v.hub.read(cx);
    let tint = tint_color(h.media.tint);
    let side = || div().flex().flex_none().items_center().gap(px(7.0)).h_full().text_size(px(12.5)).font_weight(FontWeight::SEMIBOLD);
    let mut l = side().text_color(hex(TEXT));
    let mut r = side().text_color(hex(DIM));
    let mut m: Option<AnyElement> = None;
    match mode {
        Mode::Timer => {
            l = l.child(ring(18.0, 3.0, h.timer.progress(), hex(ORANGE).into(), with_alpha(hex(ORANGE), 0.25)));
            r = r.child(div().text_color(hex(ORANGE)).child(h.timer.label()));
        }
        Mode::Music | Mode::MusicLyrics => {
            l = l.child(art(h.media.art.clone(), 22.0, 6.0));
            if h.i("visualizer") {
                r = r.child(viz(&v.viz_values(h, 5), 3.0, 16.0, 2.5, tint));
            } else {
                r = r.child(icon("music", 14.0, hex(DIM)));
            }
            if mode == Mode::MusicLyrics {
                let (i, p) = v.lyric_now(h);
                let text = if i < 0 { "♪".to_string() } else { h.lyrics.lines[i as usize].1.clone() };
                let k = Kar { size: 12.5, weight: FontWeight(650.0), family: FONT_TEXT, max_w: w - 28.0 - 22.0 - 40.0 - 20.0 - 14.0, dim: wa(0.32), fill: gpui::white().into() };
                let line = karaoke(window, &text, if i < 0 { 0.0 } else { p }, &k);
                m = Some(div().flex_1().min_w_0().flex().items_center().justify_center().h_full().px(px(10.0)).child(line).into_any_element());
            }
        }
        Mode::Idle | Mode::Privacy => {
            if h.settings.get("island.time").and_then(|x| x.as_bool()) != Some(false) {
                l = l.child(div().child(crate::util::time_str()));
            }
            match &h.weather {
                Some(wx) if h.i("weather") && wx.temp.is_some() => {
                    r = r.child(icon(crate::util::weather_icon(wx.code, wx.is_day), 14.0, hex(DIM))).child(format!("{}°", wx.temp.unwrap_or(0.0)));
                }
                _ => {
                    let c = crate::util::christmas();
                    r = r.child(icon("tree", 14.0, hex(DIM))).child(if c.is_eve || c.is_day { "God Jul".to_string() } else { format!("{}d", c.days) });
                }
            }
        }
        Mode::Empty => {}
    }
    r = r.children(dots(v, h));
    let mut row = div().size_full().flex().items_center().justify_between().px(px(14.0)).child(l);
    if let Some(m) = m {
        row = row.child(m);
    }
    row.child(r).into_any_element()
}

// ---------------------------------------------------------------- HUD (volume): stays notch-height, just widens
fn hud(v: &IslandView, cx: &Context<IslandView>) -> AnyElement {
    let h = v.hub.read(cx);
    let (level, muted) = (h.volume, h.muted);
    div()
        .size_full()
        .flex()
        .items_center()
        .gap(px(10.0))
        .px(px(16.0))
        .child(icon(if muted || level == 0.0 { "mute" } else { "vol" }, 16.0, hex(TEXT)))
        .child(div().flex_1())
        .child(
            div()
                .w(px(110.0))
                .h(px(5.0))
                .rounded(px(3.0))
                .bg(wa(0.14))
                .overflow_hidden()
                .child(div().h_full().rounded(px(3.0)).bg(gpui::white()).w(px(110.0 * if muted { 0.0 } else { level }))),
        )
        .child(
            div()
                .w(px(38.0))
                .text_right()
                .text_size(px(12.0))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(hex(DIM))
                .child(if muted { "Muted".to_string() } else { format!("{}%", (level * 100.0).round()) }),
        )
        .into_any_element()
}

// ---------------------------------------------------------------- drop-down activities
fn drop_view(v: &IslandView, cx: &Context<IslandView>) -> AnyElement {
    let h = v.hub.read(cx);
    let d = &v.drop;
    let pic: AnyElement = match (&d.pic, d.dot, d.icon) {
        (Some(p), _, _) => img_el(p.clone(), 46.0, 11.0).into_any_element(),
        (None, Some(c), _) => div().size(px(46.0)).rounded(px(11.0)).bg(hex(0x1c1c1e)).flex().items_center().justify_center().child(dot(c, 12.0)).into_any_element(),
        (None, None, Some((name, col))) => div().size(px(46.0)).flex_none().rounded(px(11.0)).bg(hex(0x1c1c1e)).flex().items_center().justify_center().child(icon(name, 22.0, hex(col))).into_any_element(),
        _ => art(None, 46.0, 11.0),
    };
    let mut row = div()
        .size_full()
        .flex()
        .items_center()
        .gap(px(13.0))
        .pt(px(38.0))
        .pb(px(16.0))
        .px(px(18.0))
        .child(pic)
        .child(
            div()
                .flex_1()
                .min_w_0()
                .flex()
                .flex_col()
                .child(div().text_size(px(11.0)).text_color(hex(DIM)).font_weight(FontWeight::SEMIBOLD).truncate().child(d.k.clone()))
                .child(div().text_size(px(14.5)).font_weight(FontWeight::SEMIBOLD).truncate().child(d.t1.clone()))
                .child(div().text_size(px(12.5)).text_color(hex(DIM)).truncate().child(d.t2.clone())),
        );
    if d.viz && d.kind == DropKind::Track {
        row = row.child(viz(&v.viz_values(h, 5), 3.5, 22.0, 3.0, tint_color(h.media.tint)));
    }
    row.into_any_element()
}

// ---------------------------------------------------------------- expanded dashboard
fn expanded(v: &IslandView, tab: Tab, window: &mut Window, cx: &Context<IslandView>) -> AnyElement {
    let h = v.hub.read(cx);
    let tab_btn = |id: &'static str, t: Tab, ic: &'static str, label: &'static str, cx: &Context<IslandView>| {
        let on = tab == t;
        div()
            .id(id)
            .h(px(24.0))
            .px(px(10.0))
            .rounded(px(12.0))
            .flex()
            .items_center()
            .gap(px(6.0))
            .text_size(px(12.0))
            .font_weight(FontWeight::SEMIBOLD)
            .cursor_pointer()
            .text_color(if on { hex(TEXT) } else { hex(DIM) })
            .when(on, |d| d.bg(wa(0.14)))
            .hover(|s| s.text_color(hex(TEXT)))
            .child(icon(ic, 13.0, if on { hex(TEXT) } else { hex(DIM) }))
            .child(label)
            .on_click(cx.listener(move |this, _, _, cx| {
                cx.stop_propagation();
                this.set_tab(t, cx)
            }))
    };
    let mut tabs = div().flex().gap(px(2.0)).child(tab_btn("tab-home", Tab::Home, "home", "Home", cx));
    if h.lyrics_on() {
        tabs = tabs.child(tab_btn("tab-lyrics", Tab::Lyrics, "mic", "Lyrics", cx));
    }
    tabs = tabs.child(tab_btn("tab-timer", Tab::Timer, "clock", "Timer", cx));

    let topbar = div()
        .h(px(30.0))
        .flex()
        .items_center()
        .gap(px(4.0))
        .child(tabs)
        .child(sources(v, h, cx))
        .child(div().flex_1())
        .child(div().flex().gap(px(5.0)).mr(px(6.0)).children(dots(v, h)))
        .child(
            div()
                .id("gear")
                .size(px(26.0))
                .rounded_full()
                .flex()
                .items_center()
                .justify_center()
                .cursor_pointer()
                .hover(|s| s.bg(wa(0.08)))
                .child(icon("gear", 15.0, hex(DIM)))
                .on_click(cx.listener(|this, _, _, cx| {
                    cx.stop_propagation();
                    this.open_settings(cx)
                })),
        );
    let page = match tab {
        Tab::Home => home(v, h, cx),
        Tab::Lyrics => lyrics_page(v, h, window, cx),
        Tab::Timer => timer_page(h, cx),
    };
    div().size_full().pt(px(6.0)).px(px(18.0)).pb(px(18.0)).flex().flex_col().child(topbar).child(div().pt(px(12.0)).flex_1().child(page)).into_any_element()
}

fn sources(v: &IslandView, h: &Hub, cx: &Context<IslandView>) -> AnyElement {
    let list = &h.media.sources;
    if !h.i("media") || list.len() < 2 {
        return div().into_any_element();
    }
    let tint = tint_color(h.media.tint);
    let _ = v;
    div()
        .flex()
        .gap(px(4.0))
        .min_w_0()
        .overflow_hidden()
        .ml(px(10.0))
        .children(list.iter().enumerate().map(|(i, s)| {
            let id = s.id.clone();
            let on = s.selected;
            div()
                .id(("src", i))
                .h(px(24.0))
                .max_w(px(168.0))
                .pl(px(8.0))
                .pr(px(10.0))
                .rounded(px(12.0))
                .flex()
                .items_center()
                .gap(px(6.0))
                .text_size(px(11.5))
                .cursor_pointer()
                .border_1()
                .border_color(if on { with_alpha(tint, 0.55) } else { wa(0.14) })
                .when(on, |d| d.bg(with_alpha(tint, 0.22)).text_color(hex(TEXT)))
                .when(!on, |d| d.text_color(hex(DIM)).hover(|s| s.bg(wa(0.08)).text_color(hex(TEXT))))
                .child(if s.playing { dot(GREEN, 6.0) } else { div().size(px(6.0)).flex_none().rounded_full().bg(hex(DIM2)) })
                .child(div().flex_none().font_weight(FontWeight(650.0)).child(s.app.clone()))
                .child(div().min_w_0().truncate().opacity(0.7).child(s.title.clone()))
                .on_click(cx.listener(move |this, _, _, cx| {
                    cx.stop_propagation();
                    if !on {
                        let id = id.clone();
                        this.hub.update(cx, |h, cx| h.select_source(&id, cx));
                    }
                }))
        }))
        .into_any_element()
}

/// Invisible element that records its bounds (for click → fraction maths).
pub fn probe(cell: Rc<Cell<Bounds<Pixels>>>) -> impl IntoElement {
    canvas(move |b, _, _| cell.set(b), |_, _, _, _| {}).absolute().top_0().left_0().size_full()
}

fn ctrl_btn(id: &'static str, name: &'static str, size: f32, btn: f32, enabled: bool, cx: &Context<IslandView>, action: &'static str) -> Stateful<Div> {
    div()
        .id(id)
        .size(px(btn))
        .rounded_full()
        .flex()
        .items_center()
        .justify_center()
        .when(enabled, |d| d.cursor_pointer().hover(|s| s.bg(wa(0.08))))
        .when(!enabled, |d| d.opacity(0.3))
        .child(icon(name, size, hex(TEXT)))
        .on_click(cx.listener(move |this, _, _, cx| {
            cx.stop_propagation();
            if enabled {
                this.hub.update(cx, |h, cx| h.media_cmd(action, cx));
            }
        }))
}

fn home(v: &IslandView, h: &Hub, cx: &Context<IslandView>) -> AnyElement {
    let m = &h.media;
    let tint = tint_color(m.tint);
    let np: AnyElement = if h.i("media") && m.active {
        let pos = m.pos();
        let frac = if m.duration > 0.0 { (pos / m.duration) as f32 } else { 0.0 };
        let track_b = Rc::new(Cell::new(Bounds::default()));
        let tb = track_b.clone();
        let cover = div()
            .relative()
            .size(px(112.0))
            .flex_none()
            .child(div().absolute().top(px(18.0)).left(px(10.0)).size(px(92.0)).rounded(px(16.0)).shadow(vec![BoxShadow {
                color: with_alpha(tint, 0.55),
                offset: point(px(0.), px(8.)),
                blur_radius: px(24.),
                spread_radius: px(0.),
            }]))
            .child(div().absolute().top_0().left_0().child(art(m.art.clone(), 112.0, 16.0)));
        let mut titles = div()
            .flex_1()
            .min_w_0()
            .child(div().text_size(px(16.0)).font_weight(FontWeight(650.0)).truncate().child(m.title.clone()))
            .child(div().text_size(px(13.0)).text_color(hex(DIM)).truncate().mt(px(2.0)).child(m.artist.clone()));
        if !m.app.is_empty() {
            titles = titles.child(div().text_size(px(11.0)).text_color(tint).font_weight(FontWeight(650.0)).mt(px(3.0)).child(m.app.clone()));
        }
        let mut row1 = div().flex().items_start().gap(px(10.0)).child(titles);
        if h.i("visualizer") {
            row1 = row1.child(viz(&v.viz_values(h, 6), 3.5, 22.0, 3.0, tint));
        }
        let scrub = div()
            .mt_auto()
            .child(
                div()
                    .id("track")
                    .relative()
                    .h(px(16.0))
                    .flex()
                    .items_center()
                    .cursor_pointer()
                    .child(probe(track_b))
                    .child(div().w_full().h(px(5.0)).rounded(px(3.0)).bg(wa(0.14)).overflow_hidden().child(div().h_full().bg(gpui::white()).w(relative(frac.clamp(0.0, 1.0))))),
            )
            .child(
                div()
                    .flex()
                    .justify_between()
                    .text_size(px(11.0))
                    .text_color(hex(DIM))
                    .mt(px(1.0))
                    .child(crate::util::clock(pos))
                    .child(if m.duration > 0.0 { format!("-{}", crate::util::clock(m.duration - pos)) } else { String::new() }),
            )
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, e: &MouseDownEvent, _, cx| {
                    cx.stop_propagation();
                    let b = tb.get();
                    if b.size.width.f() <= 0.0 {
                        return;
                    }
                    let f = ((e.position.x - b.origin.x).f() / b.size.width.f()).clamp(0.0, 1.0) as f64;
                    this.hub.update(cx, |h, cx| {
                        let d = h.media.duration;
                        if d > 0.0 {
                            h.seek(f * d, cx)
                        }
                    });
                }),
            );
        let ctrls = div()
            .flex()
            .items_center()
            .justify_center()
            .gap(px(22.0))
            .mt(px(4.0))
            .child(ctrl_btn("prev", "prev", 22.0, 34.0, m.can_prev, cx, "prev"))
            .child(ctrl_btn("play", if m.playing { "pause" } else { "play" }, 28.0, 34.0, true, cx, "toggle"))
            .child(ctrl_btn("next", "next", 22.0, 34.0, m.can_next, cx, "next"));
        div()
            .flex()
            .gap(px(16.0))
            .flex_1()
            .min_w_0()
            .h_full()
            .child(cover)
            .child(div().flex_1().min_w_0().h_full().flex().flex_col().child(row1).child(scrub).child(ctrls))
            .into_any_element()
    } else {
        div()
            .flex_1()
            .flex()
            .items_center()
            .gap(px(14.0))
            .text_color(hex(DIM))
            .text_size(px(13.0))
            .child(div().size(px(112.0)).rounded(px(16.0)).bg(hex(0x1c1c1e)).flex().items_center().justify_center().child(icon("music", 36.0, hex(DIM2))))
            .child(div().child("Nothing playing").child(div().text_size(px(12.0)).child("Play something in Spotify or your browser")))
            .into_any_element()
    };

    let mini = || {
        div()
            .rounded(px(14.0))
            .bg(linear_gradient(180.0, linear_color_stop(wa(0.085), 0.0), linear_color_stop(wa(0.045), 1.0)))
            .border_1()
            .border_color(wa(0.06))
            .px(px(12.0))
            .py(px(10.0))
            .h(px(76.0))
            .flex()
            .flex_col()
    };
    let label = |s: String| div().text_size(px(10.5)).text_color(hex(DIM)).font_weight(FontWeight(650.0)).child(s.to_uppercase());
    let mut side = div().w(px(186.0)).flex_none().flex().flex_col().gap(px(8.0));
    if h.i("weather") {
        side = side.child(match &h.weather {
            Some(w) if w.temp.is_some() => mini()
                .child(label(w.city.split(',').next().unwrap_or("Weather").to_string()))
                .child(
                    div()
                        .flex()
                        .items_center()
                        .justify_between()
                        .mt(px(2.0))
                        .child(div().font_family(FONT_DISPLAY).text_size(px(26.0)).line_height(px(29.0)).child(format!("{}°", w.temp.unwrap_or(0.0))))
                        .child(icon(crate::util::weather_icon(w.code, w.is_day), 26.0, hex(0xc9ecff))),
                )
                .child(div().text_size(px(11.5)).text_color(hex(DIM)).truncate().child(format!(
                    "{} · H{}° L{}°",
                    crate::util::wmo(w.code).0,
                    w.hi.unwrap_or(0.0),
                    w.lo.unwrap_or(0.0)
                ))),
            _ => mini().child(label("Weather".into())).child(div().text_size(px(11.5)).text_color(hex(DIM)).mt(px(4.0)).child("Set a city in Settings")),
        });
    }
    if h.i("system") {
        let s = h.stats.clone().unwrap_or_default();
        let has = h.stats.is_some();
        let bar = |name: &'static str, val: f64| {
            div()
                .flex()
                .items_center()
                .gap(px(6.0))
                .text_size(px(11.0))
                .text_color(hex(DIM))
                .child(div().w(px(34.0)).child(name))
                .child(div().flex_1().h(px(4.0)).rounded(px(2.0)).bg(wa(0.14)).overflow_hidden().child(div().h_full().bg(gpui::white()).w(relative((val / 100.0).clamp(0.0, 1.0) as f32))))
                .child(div().w(px(34.0)).text_right().text_color(hex(TEXT)).font_weight(FontWeight::SEMIBOLD).child(if has { format!("{}%", val.round()) } else { "–".into() }))
        };
        side = side.child(mini().child(label("System".into())).child(div().flex().flex_col().gap(px(6.0)).mt(px(6.0)).child(bar("CPU", s.cpu)).child(bar("RAM", s.ram))));
    } else {
        let c = crate::util::christmas();
        side = side.child(mini().child(label("Christmas Eve".into())).child(div().text_size(px(13.0)).mt(px(2.0)).child(if c.is_eve || c.is_day { "God Jul! 🎄".to_string() } else { format!("{} days to go", c.days) })));
    }

    let vol_b = Rc::new(Cell::new(Bounds::default()));
    let (vb1, vb2) = (vol_b.clone(), vol_b.clone());
    let level = if h.muted { 0.0 } else { h.volume };
    let set_from = move |this: &mut IslandView, x: Pixels, b: Bounds<Pixels>, cx: &mut Context<IslandView>| {
        if b.size.width.f() > 0.0 {
            let f = ((x - b.origin.x).f() / b.size.width.f()).clamp(0.0, 1.0);
            this.hub.update(cx, |h, cx| h.set_volume(f, cx));
        }
    };
    let volrow = div()
        .flex()
        .items_center()
        .gap(px(10.0))
        .mt(px(10.0))
        .child(
            div()
                .id("mute")
                .cursor_pointer()
                .child(icon(if h.muted { "mute" } else { "vol" }, 15.0, hex(DIM)))
                .on_click(cx.listener(|this, _, _, cx| {
                    cx.stop_propagation();
                    this.hub.update(cx, |h, cx| h.toggle_mute(cx))
                })),
        )
        .child(
            div()
                .id("vol")
                .relative()
                .flex_1()
                .h(px(16.0))
                .flex()
                .items_center()
                .cursor_pointer()
                .child(probe(vol_b))
                .child(div().w_full().h(px(5.0)).rounded(px(3.0)).bg(wa(0.14)).overflow_hidden().child(div().h_full().bg(gpui::white()).w(relative(level))))
                .child(div().absolute().top(px(1.5)).left(relative(level)).ml(px(-6.5)).size(px(13.0)).rounded_full().bg(gpui::white()).shadow(vec![BoxShadow {
                    color: ba(0.5),
                    offset: point(px(0.), px(1.)),
                    blur_radius: px(4.),
                    spread_radius: px(0.),
                }]))
                .on_mouse_down(MouseButton::Left, cx.listener(move |this, e: &MouseDownEvent, _, cx| {
                    cx.stop_propagation();
                    set_from(this, e.position.x, vb1.get(), cx)
                }))
                .on_mouse_move(cx.listener(move |this, e: &MouseMoveEvent, _, cx| {
                    if e.pressed_button == Some(MouseButton::Left) {
                        set_from(this, e.position.x, vb2.get(), cx)
                    }
                })),
        );

    div()
        .flex()
        .flex_col()
        .child(div().flex().gap(px(14.0)).h(px(160.0)).child(np).child(side))
        .child(volrow)
        .into_any_element()
}

fn lyrics_page(v: &IslandView, h: &Hub, window: &mut Window, cx: &Context<IslandView>) -> AnyElement {
    let m = &h.media;
    let mut col = div().h(px(238.0)).flex().flex_col();
    if m.active {
        let mut sub = m.artist.clone();
        if !m.app.is_empty() {
            sub.push_str(&format!(" · {}", m.app));
        }
        col = col.child(
            div()
                .flex()
                .items_center()
                .gap(px(10.0))
                .h(px(34.0))
                .child(art(m.art.clone(), 34.0, 8.0))
                .child(
                    div()
                        .min_w_0()
                        .flex_1()
                        .child(div().text_size(px(13.0)).font_weight(FontWeight(650.0)).truncate().child(m.title.clone()))
                        .child(div().text_size(px(11.5)).text_color(hex(DIM)).truncate().child(sub)),
                ),
        );
    }
    let tint = tint_color(m.tint);
    let msg = |ic: &'static str, t1: &'static str, t2: &'static str| {
        div()
            .flex_1()
            .flex()
            .items_center()
            .justify_center()
            .gap(px(14.0))
            .text_size(px(15.0))
            .font_weight(FontWeight::SEMIBOLD)
            .child(icon(ic, 34.0, tint))
            .child(div().child(t1).when(!t2.is_empty(), |d| d.child(div().text_size(px(12.5)).text_color(hex(DIM)).font_weight(FontWeight::NORMAL).child(t2))))
    };
    let body: AnyElement = if !m.active {
        msg("mic", "Nothing playing", "Play something in Spotify to sing along").into_any_element()
    } else {
        match h.lyrics.state {
            LyrState::Loading => msg("mic", "Looking up lyrics…", "").into_any_element(),
            LyrState::Instrumental => msg("music", "Instrumental", "Nothing to sing on this one").into_any_element(),
            LyrState::Missing | LyrState::None => msg("mic", "No lyrics found", "LRCLIB doesn't have this song yet").into_any_element(),
            LyrState::Plain => div()
                .id("plain")
                .flex_1()
                .mt(px(8.0))
                .overflow_y_scroll()
                .text_size(px(15.0))
                .font_weight(FontWeight::SEMIBOLD)
                .line_height(px(22.5))
                .text_color(wa(0.85))
                .children(h.lyrics.plain.lines().map(|l| div().min_h(px(22.5)).child(l.to_string())))
                .child(div().text_size(px(11.0)).text_color(hex(DIM)).my(px(10.0)).child("Not time-synced for this song"))
                .into_any_element(),
            LyrState::Synced => {
                let (cur, fill) = v.lyric_now(h);
                let scroll = v.lyr_scroll.pos;
                let mut lines = div().absolute().left_0().right_0().top(px(LYR_PAD - scroll));
                for (i, (t, text)) in h.lyrics.lines.iter().enumerate() {
                    let i = i as isize;
                    let y = LYR_PAD - scroll + i as f32 * LINE_H;
                    if y < -LINE_H || y > LYR_VIEW_H + LINE_H {
                        lines = lines.child(div().h(px(LINE_H)));
                        continue;
                    }
                    // the CSS mask: fade out towards the top and bottom edges
                    let mid = (y + LINE_H / 2.0) / LYR_VIEW_H;
                    let edge = (mid / 0.18).min((1.0 - mid) / 0.22).clamp(0.0, 1.0);
                    let is_cur = i == cur;
                    let (op, size) = if is_cur { (1.0, 23.0) } else if i < cur { (0.4, 21.6) } else { (0.55, 21.6) };
                    let p = if is_cur { fill } else if i < cur { 1.0 } else { 0.0 };
                    let k = Kar { size, weight: FontWeight::BOLD, family: FONT_DISPLAY, max_w: EXP_W - 36.0 - 8.0, dim: wa(0.32), fill: gpui::white().into() };
                    let shown = if text.is_empty() { "♪" } else { text.as_str() };
                    let t = *t;
                    lines = lines.child(
                        div()
                            .id(("ln", i as usize))
                            .h(px(LINE_H))
                            .px(px(4.0))
                            .flex()
                            .items_center()
                            .cursor_pointer()
                            .opacity(op * edge)
                            .child(karaoke(window, shown, p, &k))
                            .on_click(cx.listener(move |this, _, _, cx| {
                                cx.stop_propagation();
                                this.hub.update(cx, |h, cx| h.seek((t - LEAD + 0.05).max(0.0), cx));
                                this.lyr_idx = -3;
                            })),
                    );
                }
                div().relative().flex_1().mt(px(8.0)).overflow_hidden().child(lines).into_any_element()
            }
        }
    };
    col.child(body).into_any_element()
}

fn timer_page(h: &Hub, cx: &Context<IslandView>) -> AnyElement {
    let t = h.timer;
    let running = t.running();
    let remain = if running { 1.0 - t.progress() } else { 0.0 };
    let dial = div()
        .relative()
        .size(px(150.0))
        .flex_none()
        .child(ring(150.0, 8.0, remain, hex(ORANGE).into(), wa(0.14)))
        .child(
            div()
                .absolute()
                .top_0()
                .left_0()
                .size_full()
                .flex()
                .items_center()
                .justify_center()
                .font_family(FONT_DISPLAY)
                .text_size(px(34.0))
                .font_weight(FontWeight::LIGHT)
                .child(if running { t.label() } else { "0:00".into() }),
        );
    let presets = div().flex().flex_wrap().gap(px(8.0)).children([1.0, 5.0, 10.0, 15.0, 25.0, 45.0, 60.0].into_iter().map(|m| {
        div()
            .id(("preset", m as usize))
            .h(px(32.0))
            .px(px(14.0))
            .rounded(px(16.0))
            .bg(wa(0.08))
            .hover(|s| s.bg(wa(0.14)))
            .cursor_pointer()
            .flex()
            .items_center()
            .text_size(px(12.5))
            .font_weight(FontWeight::SEMIBOLD)
            .child(format!("{} min", m))
            .on_click(cx.listener(move |this, _, _, cx| {
                cx.stop_propagation();
                this.hub.update(cx, |h, cx| h.timer_start(m, cx))
            }))
    }));
    let go_label = if running { if t.is_paused() { "Resume" } else { "Pause" } } else { "Start 25 min" };
    let actions = div()
        .flex()
        .gap(px(10.0))
        .mt(px(14.0))
        .child(
            div()
                .id("t-go")
                .h(px(36.0))
                .px(px(20.0))
                .rounded(px(18.0))
                .bg(hex(ORANGE))
                .text_color(gpui::black())
                .font_weight(FontWeight(650.0))
                .text_size(px(13.0))
                .flex()
                .items_center()
                .cursor_pointer()
                .child(go_label)
                .on_click(cx.listener(move |this, _, _, cx| {
                    cx.stop_propagation();
                    this.hub.update(cx, |h, cx| if h.timer.running() { h.timer_toggle(cx) } else { h.timer_start(25.0, cx) })
                })),
        )
        .child(
            div()
                .id("t-reset")
                .h(px(36.0))
                .px(px(20.0))
                .rounded(px(18.0))
                .bg(wa(0.14))
                .font_weight(FontWeight(650.0))
                .text_size(px(13.0))
                .flex()
                .items_center()
                .cursor_pointer()
                .child("Reset")
                .on_click(cx.listener(|this, _, _, cx| {
                    cx.stop_propagation();
                    this.hub.update(cx, |h, cx| h.timer_reset(cx))
                })),
        );
    div()
        .flex()
        .items_center()
        .gap(px(22.0))
        .pt(px(4.0))
        .px(px(4.0))
        .child(dial)
        .child(
            div()
                .flex_1()
                .child(presets)
                .child(actions)
                .child(div().text_size(px(12.0)).text_color(hex(DIM)).mt(px(10.0)).child("The timer keeps running in the notch as a live activity, and chimes when it's done.")),
        )
        .into_any_element()
}
