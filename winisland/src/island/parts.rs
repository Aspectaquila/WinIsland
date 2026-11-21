//! Building blocks shared by both looks: icons, visualizer bars, progress rings, karaoke text, album art.

use crate::util::PxF;
use crate::assets;
use crate::theme::*;
use gpui::prelude::*;
use gpui::*;
use std::sync::Arc;

pub fn icon(name: &str, size: f32, color: impl Into<Hsla>) -> Svg {
    svg().path(assets::path(name)).size(px(size)).flex_none().text_color(color.into())
}

/// Visualizer bar heights (0.15..1), from the real output level when fresh, else gentle synthetic motion.
pub fn viz_values(n: usize, level: f32, fresh: bool, t: f32) -> Vec<f32> {
    let base = if fresh { (level.powf(0.6) * 1.25).min(1.0) } else { 0.35 + 0.25 * (t * 1.3).sin() };
    (0..n)
        .map(|i| {
            let i8 = i % 8;
            let (f, p) = (1.7 + i8 as f32 * 0.83, i8 as f32 * 1.3);
            (base * (0.45 + 0.55 * (t * f * if fresh { 2.2 } else { 1.4 } + p).sin().abs())).clamp(0.15, 1.0)
        })
        .collect()
}

/// Bars centred vertically, like the CSS `.viz`.
pub fn viz(values: &[f32], bar_w: f32, bar_h: f32, gap: f32, color: Hsla) -> Div {
    div().flex().flex_none().items_center().gap(px(gap)).h(px(bar_h)).children(values.iter().map(|v| {
        div().w(px(bar_w)).h(px((bar_h * v).max(bar_w))).rounded(px(bar_w / 2.0 + 0.5)).bg(color)
    }))
}

/// A waveform as on iPhone: mirrored bars that grow from the centre line.
pub fn waveform(values: &[f32], bar_w: f32, bar_h: f32, gap: f32, color: Hsla) -> Div {
    viz(values, bar_w, bar_h, gap, color)
}

fn arc_points(cx: f32, cy: f32, r: f32, from: f32, sweep: f32) -> Vec<Point<Pixels>> {
    let n = ((sweep.abs() / std::f32::consts::TAU) * 96.0).ceil().max(2.0) as usize;
    (0..=n)
        .map(|i| {
            let a = from + sweep * i as f32 / n as f32;
            point(px(cx + r * a.cos()), px(cy + r * a.sin()))
        })
        .collect()
}

pub fn stroke_arc(window: &mut Window, center: Point<Pixels>, r: f32, width: f32, from: f32, sweep: f32, color: Hsla) {
    if sweep.abs() < 0.001 {
        return;
    }
    let (cx, cy, hw) = (center.x.f(), center.y.f(), width / 2.0);
    let pi = std::f32::consts::PI;
    if sweep.abs() >= std::f32::consts::TAU * 0.999 {
        // full ring: outer + inner contour, even-odd fill
        let outer = arc_points(cx, cy, r + hw, 0.0, std::f32::consts::TAU);
        let mut inner = arc_points(cx, cy, r - hw, 0.0, std::f32::consts::TAU);
        inner.reverse();
        let mut b = PathBuilder::fill().with_style(PathStyle::Fill(FillOptions::default().with_fill_rule(FillRule::EvenOdd)));
        b.add_polygon(&outer, true);
        b.add_polygon(&inner, true);
        if let Ok(path) = b.build() {
            window.paint_path(path, color);
        }
        return;
    }
    let sg = sweep.signum();
    let mut pts = arc_points(cx, cy, r + hw, from, sweep);
    let a1 = from + sweep;
    pts.extend(arc_points(cx + r * a1.cos(), cy + r * a1.sin(), hw, a1, pi * sg)); // round cap
    let mut inner = arc_points(cx, cy, r - hw, from, sweep);
    inner.reverse();
    pts.extend(inner);
    pts.extend(arc_points(cx + r * from.cos(), cy + r * from.sin(), hw, from + pi, pi * sg)); // round cap
    let mut b = PathBuilder::fill();
    b.add_polygon(&pts, true);
    if let Ok(path) = b.build() {
        window.paint_path(path, color);
    }
}

/// Progress ring(s): each entry (radius, progress 0..1, colour); a faint track under each.
pub fn rings(size: f32, width: f32, entries: Vec<(f32, f32, Hsla, Hsla)>) -> impl IntoElement {
    canvas(
        |_, _, _| (),
        move |bounds, _, window, _| {
            let c = bounds.center();
            for (r, p, col, track) in &entries {
                stroke_arc(window, c, *r, width, 0.0, std::f32::consts::TAU * 0.9999, *track);
                let p = p.clamp(0.0, 1.0);
                if p > 0.002 {
                    stroke_arc(window, c, *r, width, -std::f32::consts::FRAC_PI_2, std::f32::consts::TAU * p, *col);
                }
            }
        },
    )
    .size(px(size))
    .flex_none()
}

pub fn ring(size: f32, width: f32, p: f32, color: Hsla, track: Hsla) -> impl IntoElement {
    let r = (size - width) / 2.0;
    rings(size, width, vec![(r, p, color, track)])
}

pub fn text_width(window: &mut Window, text: &str, size: f32, family: &'static str, weight: FontWeight) -> f32 {
    if text.is_empty() {
        return 0.0;
    }
    let mut f = font(family);
    f.weight = weight;
    let run = TextRun { len: text.len(), font: f, color: gpui::white(), background_color: None, underline: None, strikethrough: None };
    window.text_system().layout_line(text, px(size), &[run], None).width.f()
}

pub struct Kar {
    pub size: f32,
    pub weight: FontWeight,
    pub family: &'static str,
    pub max_w: f32,
    pub dim: Hsla,
    pub fill: Hsla,
}

/// The karaoke line: a dim copy with a bright copy revealed left → right by `p`.
pub fn karaoke(window: &mut Window, text: &str, p: f32, k: &Kar) -> Div {
    let text = if text.is_empty() { "♪".to_string() } else { text.to_string() };
    let w = (text_width(window, &text, k.size, k.family, k.weight) + 2.0).min(k.max_w).max(4.0);
    let line = |c: Hsla| div().w(px(w)).truncate().text_color(c).child(text.clone());
    div()
        .relative()
        .flex_none()
        .w(px(w))
        .text_size(px(k.size))
        .font_weight(k.weight)
        .font_family(k.family)
        .line_height(px(k.size * 1.25))
        .child(line(k.dim))
        .child(div().absolute().top_0().left_0().h_full().w(px(w * p.clamp(0.0, 1.0))).overflow_hidden().child(line(k.fill)))
}

pub fn art(img: Option<Arc<RenderImage>>, size: f32, radius: f32) -> AnyElement {
    match img {
        Some(i) => img_el(i, size, radius).into_any_element(),
        None => div()
            .size(px(size))
            .flex_none()
            .rounded(px(radius))
            .bg(hex(0x1c1c1e))
            .flex()
            .items_center()
            .justify_center()
            .child(icon("music", (size * 0.36).max(10.0), hex(DIM2)))
            .into_any_element(),
    }
}

pub fn img_el(i: Arc<RenderImage>, size: f32, radius: f32) -> Img {
    img(ImageSource::Render(i)).size(px(size)).flex_none().rounded(px(radius)).object_fit(ObjectFit::Cover)
}

pub fn dot(color: u32, d: f32) -> Div {
    div().size(px(d)).flex_none().rounded_full().bg(hex(color)).shadow(vec![BoxShadow {
        color: with_alpha(hex(color), 0.8),
        offset: point(px(0.), px(0.)),
        blur_radius: px(6.),
        spread_radius: px(0.),
    }])
}
