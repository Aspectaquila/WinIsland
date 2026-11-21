//! The soft shadow and album-tint glow under the Dynamic Notch. They live in their own fully click-through window
//! (layered + transparent) right behind the island, so the island window's click region can stay exactly the
//! island shape. The Dynamic Island capsule stays crisp (nothing is drawn for it).

use crate::island::Geo;
use crate::spring::Spring;
use crate::theme::{ba, with_alpha};
use gpui::prelude::*;
use gpui::*;
use std::time::Instant;

pub struct GlowView {
    pub geo: Geo,
    pub center: f32,
    pub tint: Hsla,
    pub glow: bool,
    pub shadow: bool,
    alpha: Spring,
    shade: Spring,
    last: Option<Instant>,
}

impl GlowView {
    pub fn new(center: f32) -> Self {
        GlowView {
            geo: Geo::default(),
            center,
            tint: gpui::white().into(),
            glow: false,
            shadow: false,
            alpha: Spring::new(0.0).with(60.0, 15.0),
            shade: Spring::new(0.0).with(90.0, 19.0),
            last: None,
        }
    }

    /// Called by the island whenever its geometry or glow state changes.
    pub fn set(&mut self, geo: Geo, tint: Hsla, glow: bool, shadow: bool, cx: &mut Context<Self>) {
        if self.geo != geo || self.tint != tint || self.glow != glow || self.shadow != shadow {
            self.geo = geo;
            self.tint = tint;
            self.glow = glow;
            self.shadow = shadow;
            cx.notify();
        }
    }
}

impl Render for GlowView {
    fn render(&mut self, window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        self.alpha.set(if self.glow { 1.0 } else { 0.0 });
        self.shade.set(if self.shadow { 1.0 } else { 0.0 });
        let now = Instant::now();
        let dt = self.last.map(|t| (now - t).as_secs_f32().min(1.0 / 30.0)).unwrap_or(1.0 / 60.0);
        let moving = self.alpha.step(dt) | self.shade.step(dt);
        self.last = if moving { Some(now) } else { None };
        if moving {
            window.request_animation_frame();
        }
        let a = self.alpha.pos.clamp(0.0, 1.0);
        let s = self.shade.pos.clamp(0.0, 1.0);
        let g = self.geo;
        let mut root = div().size_full().relative();
        if (a > 0.01 || s > 0.01) && g.w > 1.0 {
            let r = g.r.min(g.h / 2.0);
            let mut shadows = vec![BoxShadow { color: ba(0.35 * s), offset: point(px(0.), px(6.)), blur_radius: px(9.), spread_radius: px(0.) }];
            if a > 0.01 {
                shadows.push(BoxShadow { color: with_alpha(self.tint, 0.45 * a), offset: point(px(0.), px(10.)), blur_radius: px(18.), spread_radius: px(-6.) });
                shadows.push(BoxShadow { color: with_alpha(self.tint, 0.25 * a), offset: point(px(0.), px(4.)), blur_radius: px(8.), spread_radius: px(-2.) });
            }
            root = root.child(
                div()
                    .absolute()
                    .left(px(self.center - g.w / 2.0 + 4.0))
                    .top(px(g.y))
                    .w(px(g.w - 8.0))
                    .h(px(g.h))
                    .rounded_b(px(r))
                    .shadow(shadows),
            );
        }
        root
    }
}
