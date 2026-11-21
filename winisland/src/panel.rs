//! The settings window: one design regardless of look. Top bar (brand, "Dynamic Notch | Dynamic Island"
//! segmented control, status pill), sidebar, and the Overview / Island / Music & Lyrics / Weather / General pages.
//! Every option is bound to settings.json through the Hub.

use crate::util::PxF;
use crate::hub::{Hub, HubEvent};
use crate::input::{SubmitEvent, TextInput};
use crate::island::{IslandView, Tab};
use crate::island::parts::icon;
use crate::model::{post, Event};
use crate::theme::*;
use gpui::prelude::*;
use gpui::*;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Page {
    Overview,
    Island,
    Music,
    Weather,
    General,
}

struct Dialog {
    title: &'static str,
    text: &'static str,
    yes: &'static str,
    action: fn(&mut PanelView, &mut Context<PanelView>),
}

pub struct PanelView {
    hub: Entity<Hub>,
    preview_hub: Entity<Hub>,
    preview: Entity<IslandView>,
    page: Page,
    page_at: Instant,
    city: Entity<TextInput>,
    dialog: Option<Dialog>,
    toast: Option<(String, Instant)>,
    anims: HashMap<String, Instant>,
    look_at: Instant,
    city_pending: bool,
    mem_mb: f64,
    icon: Option<Arc<RenderImage>>,
    tour: Option<Task<()>>,
    _tasks: Vec<Task<()>>,
    _subs: Vec<Subscription>,
}

const DUR: f32 = 0.5;

fn back_out(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    let c1 = 1.70158;
    let c3 = c1 + 1.0;
    1.0 + c3 * (t - 1.0).powi(3) + c1 * (t - 1.0).powi(2)
}

impl PanelView {
    pub fn new(hub: Entity<Hub>, start: Page, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let style = hub.read(cx).settings.s("island.style");
        let preview_hub = crate::hub::demo_hub(cx, &style, false, true);
        // mirror the user's island options into the preview (without events / greetings)
        let island = hub.read(cx).settings.doc["island"].clone();
        preview_hub.update(cx, |p, _| {
            p.settings.doc["island"] = island;
            p.settings.doc["island"]["events"] = Value::Bool(false);
        });
        let preview = cx.new(|cx| IslandView::new(preview_hub.clone(), None, true, size(780.0, 330.0), cx));
        let city = cx.new(|cx| TextInput::new("e.g. Oslo", cx));
        let subs = vec![
            cx.observe(&hub, |_, _, cx| cx.notify()),
            cx.subscribe(&hub, |this, _, ev: &HubEvent, cx| this.on_hub_event(ev.clone(), cx)),
            cx.subscribe(&city, |this, _, _: &SubmitEvent, cx| this.save_city(cx)),
            cx.observe(&city, |_, _, cx| cx.notify()),
        ];
        let icon = image::load_from_memory(include_bytes!("../../branding/icon-64.png")).ok().map(|i| crate::media::rgba_to_render(i.to_rgba8()).0);
        let diag = cx.spawn(async move |this, cx| loop {
            let _ = this.update(cx, |p, cx| {
                p.mem_mb = crate::util::process_memory_mb().0;
                p.hub.update(cx, |h, _| {
                    // demo runs (previews, website captures) don't show notices about this PC's other apps
                    if h.demo.is_none() {
                        h.diag.windhawk_island = crate::system::windhawk_island_active();
                        h.diag.fluent_flyout = crate::system::fluent_flyout_running();
                    }
                });
                cx.notify();
            });
            cx.background_executor().timer(Duration::from_secs(5)).await;
        });
        let _ = window;
        let mut p = PanelView {
            hub,
            preview_hub,
            preview,
            page: start,
            page_at: Instant::now(),
            city,
            dialog: None,
            toast: None,
            anims: HashMap::new(),
            look_at: Instant::now() - Duration::from_secs(5),
            city_pending: false,
            mem_mb: crate::util::process_memory_mb().0,
            icon,
            tour: None,
            _tasks: vec![diag],
            _subs: subs,
        };
        p.update_tour(cx);
        p
    }

    fn on_hub_event(&mut self, ev: HubEvent, cx: &mut Context<Self>) {
        match ev {
            HubEvent::SettingsChanged(_) => {
                let island = self.hub.read(cx).settings.doc["island"].clone();
                self.preview_hub.update(cx, |p, cx| {
                    p.settings.doc["island"] = island;
                    p.settings.doc["island"]["events"] = Value::Bool(false);
                    cx.notify();
                });
            }
            HubEvent::Toast(t) => self.show_toast(t, cx),
            _ => {}
        }
        cx.notify();
    }

    fn show_toast(&mut self, t: String, cx: &mut Context<Self>) {
        self.toast = Some((t, Instant::now()));
        cx.notify();
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(Duration::from_millis(2700)).await;
            let _ = this.update(cx, |p, cx| {
                if p.toast.as_ref().map(|t| t.1.elapsed() > Duration::from_millis(2500)).unwrap_or(false) {
                    p.toast = None;
                    cx.notify();
                }
            });
        })
        .detach();
    }

    pub fn weather_arrived(&mut self, cx: &mut Context<Self>) {
        if !self.city_pending {
            return;
        }
        let w = self.hub.read(cx).weather.clone();
        if let Some(w) = w {
            if !w.city.is_empty() && !w.needs_city {
                self.city_pending = false;
                self.show_toast(format!("Weather: {}°{} in {}", w.temp.unwrap_or(0.0), w.units, w.city.split(',').next().unwrap_or("")), cx);
            }
        }
    }

    fn save_city(&mut self, cx: &mut Context<Self>) {
        let v = self.city.read(cx).text().trim().to_string();
        if v.is_empty() {
            return;
        }
        self.city_pending = true;
        self.show_toast(format!("Looking up {v}…"), cx);
        std::thread::spawn(move || match crate::weather::geocode(&v) {
            Some(p) => post(Event::WeatherLocation(Some(p), "")),
            None => post(Event::WeatherLocation(None, "City not found")),
        });
    }

    fn locate(&mut self, cx: &mut Context<Self>) {
        self.city_pending = true;
        self.show_toast("Asking Windows for your location…".into(), cx);
        std::thread::spawn(move || match crate::weather::locate() {
            Some(p) => post(Event::WeatherLocation(Some(p), "")),
            None => post(Event::WeatherLocation(None, "Location unavailable. Enter a city instead.")),
        });
    }

    fn set(&mut self, key: &str, v: Value, cx: &mut Context<Self>) {
        self.anims.insert(key.to_string(), Instant::now());
        let k = key.to_string();
        self.hub.update(cx, |h, cx| h.set_setting(&k, v, cx));
    }

    fn get_b(&self, key: &str, cx: &App) -> bool {
        self.hub.read(cx).settings.b(key)
    }

    fn anim(&self, key: &str) -> f32 {
        match self.anims.get(key) {
            Some(t) => back_out(t.elapsed().as_secs_f32() / DUR),
            None => 1.0,
        }
    }

    fn animating(&self) -> bool {
        self.anims.values().any(|t| t.elapsed().as_secs_f32() < DUR)
            || self.page_at.elapsed().as_secs_f32() < 0.45
            || self.look_at.elapsed().as_secs_f32() < 0.6
            || self.toast.as_ref().map(|t| t.1.elapsed().as_secs_f32() < 0.5).unwrap_or(false)
    }

    fn show_page(&mut self, p: Page, cx: &mut Context<Self>) {
        if p != self.page {
            self.page = p;
            self.page_at = Instant::now();
            self.update_tour(cx);
            cx.notify();
        }
    }

    fn set_look(&mut self, style: &'static str, cx: &mut Context<Self>) {
        if self.hub.read(cx).settings.s("island.style") == style {
            return;
        }
        self.look_at = Instant::now();
        self.set("island.style", Value::String(style.into()), cx);
        self.restart_tour(cx);
    }

    fn restart_tour(&mut self, cx: &mut Context<Self>) {
        self.tour = None;
        self.update_tour(cx);
    }

    /// The live preview runs the real island component on demo data, only while the Overview is visible.
    fn update_tour(&mut self, cx: &mut Context<Self>) {
        if self.page != Page::Overview {
            self.tour = None;
            return;
        }
        if self.tour.is_some() {
            return;
        }
        let ph = self.preview_hub.clone();
        let pv = self.preview.clone();
        self.tour = Some(cx.spawn(async move |_, cx| {
            let exec = cx.background_executor().clone();
            let wait = move |ms: u64| exec.timer(Duration::from_millis(ms));
            let mut flip = 0usize;
            loop {
                let ios = ph.read_with(cx, |h, _| h.ios()).unwrap_or(false);
                // idle
                let _ = ph.update(cx, |h, cx| {
                    h.timer_reset(cx);
                    if let Some(d) = h.demo.as_mut() {
                        d.playing = false;
                        let m = d.media();
                        h.set_media(m, cx);
                    }
                });
                let _ = pv.update(cx, |v, cx| {
                    v.widgets_open = false;
                    v.primary = None;
                    v.back(cx)
                });
                wait(2600).await;
                // a new song: drop-down / banner, then the music live activity
                let _ = ph.update(cx, |h, cx| {
                    if let Some(d) = h.demo.as_mut() {
                        d.ti = flip % 2;
                        d.playing = true;
                        d.position = 30.5;
                        d.stamp = crate::model::now_ms();
                        let m = d.media();
                        h.set_media(m, cx);
                    }
                });
                flip += 1;
                wait(4200).await;
                wait(if ios { 2600 } else { 5200 }).await;
                if ios {
                    // two activities: the bubble springs out of the capsule; tap swaps which one is primary
                    let _ = ph.update(cx, |h, cx| h.timer_start(12.0, cx));
                    wait(2800).await;
                    let _ = pv.update(cx, |v, cx| v.swap_primary(cx));
                    wait(2200).await;
                    let _ = pv.update(cx, |v, cx| v.swap_primary(cx));
                    wait(1400).await;
                }
                let _ = pv.update(cx, |v, cx| v.transient(crate::island::View::Hud, 1600, cx));
                wait(1900).await;
                let _ = pv.update(cx, |v, cx| {
                    v.tab = Tab::Home;
                    v.expand(cx)
                });
                wait(5200).await;
                let _ = pv.update(cx, |v, cx| {
                    if ios {
                        v.widgets_open = true;
                        cx.notify();
                    } else {
                        v.set_tab(Tab::Lyrics, cx)
                    }
                });
                wait(6000).await;
                let _ = pv.update(cx, |v, cx| v.back(cx));
                wait(1500).await;
            }
        }));
    }
}

// ---------------------------------------------------------------- small widgets
fn icon_sq(name: &'static str, color: u32, size: f32, radius: f32) -> Div {
    div()
        .size(px(size))
        .flex_none()
        .rounded(px(radius))
        .bg(linear_gradient(180.0, linear_color_stop(lighten(color, 0.12), 0.0), linear_color_stop(hex(color), 1.0)))
        .flex()
        .items_center()
        .justify_center()
        .child(icon(name, size * 0.62, gpui::white()))
}

fn lighten(c: u32, by: f32) -> Hsla {
    let mut h: Hsla = hex(c).into();
    h.l = (h.l + by).min(1.0);
    h
}

impl PanelView {
    fn toggle(&self, id: &'static str, key: &'static str, cx: &mut Context<Self>) -> impl IntoElement {
        let on = self.get_b(key, cx);
        let t = self.anim(key);
        let pos = if on { 16.0 * t } else { 16.0 * (1.0 - t) };
        let bg_on = if on { t.clamp(0.0, 1.0) } else { 1.0 - t.clamp(0.0, 1.0) };
        let off: Hsla = hexa(0x78788060).into();
        let blue: Hsla = hex(BLUE).into();
        let bg = Hsla { h: blue.h, s: blue.s * bg_on, l: off.l + (blue.l - off.l) * bg_on, a: off.a + (1.0 - off.a) * bg_on };
        div()
            .id(id)
            .w(px(38.0))
            .h(px(22.0))
            .flex_none()
            .rounded_full()
            .bg(bg)
            .cursor_pointer()
            .relative()
            .child(div().absolute().top(px(2.0)).left(px(2.0 + pos)).size(px(18.0)).rounded_full().bg(gpui::white()).shadow(vec![BoxShadow {
                color: ba(0.3),
                offset: point(px(0.), px(2.)),
                blur_radius: px(5.),
                spread_radius: px(0.),
            }]))
            .on_click(cx.listener(move |this, _, _, cx| {
                let v = !this.get_b(key, cx);
                this.set(key, Value::Bool(v), cx);
            }))
    }

    fn row(&self, icon_name: &'static str, color: u32, title: &'static str, desc: Option<SharedString>, control: AnyElement, req: Option<&'static str>, first: bool, cx: &App) -> Div {
        let disabled = req.map(|r| !self.get_b(r, cx)).unwrap_or(false);
        let mut txt = div().flex_1().min_w_0().child(div().text_size(px(13.5)).font_weight(FontWeight::MEDIUM).child(title));
        if let Some(d) = desc {
            txt = txt.child(div().text_size(px(12.0)).text_color(hex(0x98989d)).mt(px(2.0)).line_height(px(16.8)).child(d));
        }
        div()
            .relative()
            .flex()
            .items_center()
            .gap(px(13.0))
            .min_h(px(46.0))
            .py(px(9.0))
            .when(!first, |d| d.child(div().absolute().top_0().left(px(37.0)).right_0().h(px(1.0)).bg(wa(0.075))))
            .when(disabled, |d| d.opacity(0.38))
            .child(icon_sq(icon_name, color, 24.0, 6.5))
            .child(txt)
            .child(if disabled { div().child(control).occlude().into_any_element() } else { control })
    }

    fn tg_row(&self, id: &'static str, icon_name: &'static str, color: u32, title: &'static str, desc: Option<&'static str>, key: &'static str, req: Option<&'static str>, first: bool, cx: &mut Context<Self>) -> Div {
        let t = self.toggle(id, key, cx).into_any_element();
        self.row(icon_name, color, title, desc.map(SharedString::from), t, req, first, cx)
    }
}

fn card() -> Div {
    div().bg(wa(0.045)).rounded(px(11.0)).px(px(14.0)).border_1().border_color(wa(0.075)).flex().flex_col()
}

fn sect(t: &'static str) -> Div {
    div().text_size(px(12.0)).text_color(hex(0x98989d)).font_weight(FontWeight::SEMIBOLD).mt(px(26.0)).mb(px(8.0)).ml(px(4.0)).child(t)
}

fn title(t: &'static str, lead: &'static str) -> Div {
    div()
        .child(div().font_family(FONT_DISPLAY).text_size(px(24.0)).font_weight(FontWeight::BOLD).child(t))
        .child(div().text_color(hex(0x98989d)).mt(px(6.0)).text_size(px(13.5)).line_height(px(21.0)).max_w(px(640.0)).child(lead))
}

fn foot(el: impl IntoElement) -> Div {
    div().text_size(px(12.0)).text_color(hex(DIM2)).mt(px(10.0)).mx(px(4.0)).line_height(px(18.0)).max_w(px(640.0)).child(el)
}

fn btn(id: &'static str, label: impl Into<SharedString>, kind: u8) -> Stateful<Div> {
    let (bg, fg, hover): (Hsla, Hsla, Hsla) = match kind {
        1 => (hex(BLUE).into(), gpui::white(), hex(0x3a9bff).into()),
        2 => (with_alpha(hex(RED), 0.16), hex(0xff6961).into(), with_alpha(hex(RED), 0.26)),
        _ => (hexa(0x78788042).into(), hex(TEXT).into(), hexa(0x78788061).into()),
    };
    div()
        .id(id)
        .h(px(30.0))
        .px(px(15.0))
        .rounded(px(8.0))
        .flex()
        .flex_none()
        .items_center()
        .justify_center()
        .text_size(px(13.0))
        .font_weight(FontWeight(550.0))
        .bg(bg)
        .text_color(fg)
        .cursor_pointer()
        .hover(move |s| s.bg(hover))
        .child(label.into())
}

impl Render for PanelView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if self.animating() {
            window.request_animation_frame();
        }
        // keep the preview island centred in the stage
        let vw = window.viewport_size().width.f();
        let stage_w = (vw - 252.0 - 80.0).clamp(300.0, 820.0);
        self.preview.update(cx, |p, _| p.canvas.width = stage_w);
        self.weather_arrived(cx);

        let main = div()
            .id("main")
            .flex_1()
            .h_full()
            .overflow_y_scroll()
            .px(px(40.0))
            .pt(px(30.0))
            .pb(px(48.0))
            .child(self.page_view(stage_w, window, cx));

        let mut root = div()
            .size_full()
            .relative()
            .flex()
            .flex_col()
            .bg(hexa(0x18181bc8))
            .text_color(hex(TEXT))
            .font_family(FONT_TEXT)
            .text_size(px(14.0))
            .child(self.topbar(cx))
            .child(div().flex().flex_1().min_h_0().child(self.sidebar(cx)).child(main));
        if let Some(d) = &self.dialog {
            root = root.child(self.dialog_view(d, cx));
        }
        if let Some((t, at)) = &self.toast {
            let k = back_out(at.elapsed().as_secs_f32() / 0.45);
            root = root.child(
                div().absolute().bottom(px(26.0 - 24.0 * (1.0 - k))).left_0().right_0().flex().justify_center().child(
                    div()
                        .px(px(18.0))
                        .py(px(11.0))
                        .rounded(px(14.0))
                        .bg(hexa(0x2c2c2eeb))
                        .border_1()
                        .border_color(wa(0.08))
                        .shadow(vec![BoxShadow { color: ba(0.45), offset: point(px(0.), px(14.)), blur_radius: px(34.), spread_radius: px(0.) }])
                        .text_size(px(13.0))
                        .opacity(k.clamp(0.0, 1.0))
                        .child(t.clone()),
                ),
            );
        }
        root
    }
}

impl PanelView {
    fn topbar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let ios = self.hub.read(cx).ios();
        let t = back_out(self.look_at.elapsed().as_secs_f32() / 0.55);
        let x = if ios { t } else { 1.0 - t };
        let seg_w = 158.0;
        let brand = div()
            .flex()
            .items_center()
            .gap(px(11.0))
            .child(match &self.icon {
                Some(i) => img(ImageSource::Render(i.clone())).size(px(32.0)).rounded(px(8.0)).into_any_element(),
                None => div().size(px(32.0)).into_any_element(),
            })
            .child(div().child(div().font_family(FONT_DISPLAY).text_size(px(15.0)).font_weight(FontWeight(650.0)).child("WinIsland")).child(div().text_size(px(11.5)).text_color(hex(0x98989d)).child("Settings")));
        let thumb_bg = if ios {
            linear_gradient(180.0, linear_color_stop(hex(0x3a8bff), 0.0), linear_color_stop(hex(0x0a6cff), 1.0))
        } else {
            linear_gradient(180.0, linear_color_stop(hex(0xe9e9ee), 0.0), linear_color_stop(hex(0xc9c9d0), 1.0))
        };
        let seg_btn = |id: &'static str, ic: &'static str, label: &'static str, on: bool, style: &'static str, cx: &mut Context<Self>| {
            let fg: Hsla = if on { if ios { gpui::white() } else { hex(0x111111).into() } } else { hex(0x98989d).into() };
            div()
                .id(id)
                .relative()
                .w(px(seg_w))
                .h(px(32.0))
                .flex()
                .items_center()
                .justify_center()
                .gap(px(7.0))
                .text_size(px(13.0))
                .font_weight(FontWeight(650.0))
                .text_color(fg)
                .cursor_pointer()
                .child(icon(ic, 16.0, fg))
                .child(label)
                .on_click(cx.listener(move |this, _, _, cx| this.set_look(style, cx)))
        };
        let switch = div()
            .relative()
            .flex()
            .p(px(3.0))
            .rounded(px(12.0))
            .bg(hexa(0x76768038))
            .border_1()
            .border_color(wa(0.04))
            .child(
                div()
                    .absolute()
                    .top(px(3.0))
                    .left(px(3.0 + seg_w * x))
                    .w(px(seg_w))
                    .h(px(32.0))
                    .rounded(px(9.0))
                    .bg(thumb_bg)
                    .shadow(vec![BoxShadow {
                        color: if ios { with_alpha(hex(BLUE), 0.45) } else { ba(0.35) },
                        offset: point(px(0.), px(3.)),
                        blur_radius: px(10.),
                        spread_radius: px(0.),
                    }]),
            )
            .child(seg_btn("look-notch", "laptop", "Dynamic Notch", !ios, "notch", cx))
            .child(seg_btn("look-island", "phone", "Dynamic Island", ios, "island", cx));
        let state = div()
            .flex()
            .items_center()
            .gap(px(8.0))
            .h(px(30.0))
            .px(px(13.0))
            .rounded(px(15.0))
            .bg(wa(0.045))
            .border_1()
            .border_color(wa(0.075))
            .text_size(px(12.0))
            .text_color(hex(0x98989d))
            .child(crate::island::parts::dot(GREEN, 7.0))
            .child(format!("Running · {} MB", self.mem_mb.round()));
        div()
            .h(px(60.0))
            .flex_none()
            .flex()
            .items_center()
            .px(px(20.0))
            .bg(hexa(0x1e1e228c))
            .border_b_1()
            .border_color(wa(0.075))
            .child(div().flex_1().child(brand))
            .child(switch)
            .child(div().flex_1().flex().justify_end().child(state))
    }

    fn sidebar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let item = |id: &'static str, p: Page, ic: &'static str, color: u32, label: &'static str, cx: &mut Context<Self>| {
            let on = self.page == p;
            div()
                .id(id)
                .h(px(34.0))
                .px(px(9.0))
                .rounded(px(8.0))
                .flex()
                .items_center()
                .gap(px(11.0))
                .cursor_pointer()
                .when(on, |d| d.bg(hexa(0x0a84ffeb)).shadow(vec![BoxShadow { color: with_alpha(hex(BLUE), 0.3), offset: point(px(0.), px(2.)), blur_radius: px(10.), spread_radius: px(0.) }]))
                .when(!on, |d| d.hover(|s| s.bg(wa(0.055))))
                .child(icon_sq(ic, color, 22.0, 6.0))
                .child(div().text_size(px(13.5)).font_weight(FontWeight::MEDIUM).child(label))
                .on_click(cx.listener(move |this, _, _, cx| this.show_page(p, cx)))
        };
        let kbd = |k: &'static str| div().px(px(6.0)).py(px(2.0)).rounded(px(5.0)).bg(wa(0.045)).border_1().border_color(wa(0.075)).text_size(px(10.5)).font_weight(FontWeight::SEMIBOLD).text_color(hex(0x98989d)).child(k);
        div()
            .w(px(252.0))
            .flex_none()
            .h_full()
            .flex()
            .flex_col()
            .gap(px(2.0))
            .pt(px(18.0))
            .px(px(12.0))
            .pb(px(14.0))
            .bg(hexa(0x2c2c306b))
            .border_r_1()
            .border_color(wa(0.075))
            .child(item("nav-overview", Page::Overview, "sparkle", INDIGO, "Overview", cx))
            .child(item("nav-island", Page::Island, "island", GRAPHITE, "Island", cx))
            .child(item("nav-music", Page::Music, "music", PINK, "Music & Lyrics", cx))
            .child(item("nav-weather", Page::Weather, "cloud", BLUE, "Weather", cx))
            .child(item("nav-about", Page::General, "info", GREY, "General", cx))
            .child(div().flex_1())
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .items_center()
                    .gap(px(4.0))
                    .p(px(10.0))
                    .text_size(px(11.5))
                    .text_color(hex(DIM2))
                    .child(kbd("Ctrl"))
                    .child(kbd("Alt"))
                    .child(kbd("F"))
                    .child("opens these settings"),
            )
    }

    fn page_view(&mut self, stage_w: f32, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let k = crate::spring::ease_out(self.page_at.elapsed().as_secs_f32() / 0.45);
        let body = match self.page {
            Page::Overview => self.overview(stage_w, cx),
            Page::Island => self.island_page(cx),
            Page::Music => self.music_page(cx),
            Page::Weather => self.weather_page(window, cx),
            Page::General => self.general_page(cx),
        };
        div().w_full().max_w(px(820.0)).mx_auto().opacity(k).mt(px(10.0 * (1.0 - k))).child(body).into_any_element()
    }

    fn overview(&mut self, stage_w: f32, cx: &mut Context<Self>) -> AnyElement {
        let h = self.hub.read(cx);
        let ios = h.ios();
        let (eyebrow, t1, lead) = if ios {
            (
                "DYNAMIC ISLAND",
                "The Dynamic Island, floating free.",
                "A capsule that hovers just below the top edge and morphs into Live Activities, like on iPhone. Two activities split into a second bubble, and a click opens one as a lock-screen style card.",
            )
        } else {
            (
                "DYNAMIC NOTCH",
                "A notch that lives on your desktop.",
                "Flush with the top edge of your screen, with soft shoulders, like the MacBook notch. Music, lyrics, timers and alerts unfold right out of it.",
            )
        };
        let swap = crate::spring::ease_out((self.look_at.elapsed().as_secs_f32() - 0.1) / 0.35);
        let glow = |x: f32, y: f32, c: u32, s: f32| {
            div().absolute().left(px(x)).top(px(y)).size(px(s)).rounded_full().shadow(vec![BoxShadow {
                color: with_alpha(hex(c), 0.55),
                offset: point(px(0.), px(0.)),
                blur_radius: px(120.),
                spread_radius: px(40.),
            }])
        };
        let (g1, g2) = if ios { (0x3ec1e6, 0x8b7bff) } else { (0x3ee6c1, 0xc08bff) };
        let stage = div()
            .relative()
            .h(px(320.0))
            .w(px(stage_w))
            .rounded(px(20.0))
            .overflow_hidden()
            .border_1()
            .border_color(wa(0.08))
            .bg(linear_gradient(180.0, linear_color_stop(hex(0x1a2246), 0.0), linear_color_stop(hex(0x0b0f22), 1.0)))
            .child(glow(-60.0, 260.0, g1, 220.0))
            .child(glow(stage_w - 200.0, 250.0, g2, 240.0))
            .child(div().absolute().left(px(stage_w / 2.0 - 260.0)).top(px(-260.0)).size(px(520.0)).rounded_full().shadow(vec![BoxShadow {
                color: with_alpha(hex(0x33417a), 0.6),
                offset: point(px(0.), px(0.)),
                blur_radius: px(90.),
                spread_radius: px(10.),
            }]))
            .child(div().absolute().top_0().left_0().size_full().child(self.preview.clone()))
            .child(div().id("stage-shield").absolute().top_0().left_0().size_full().occlude())
            .child(
                div()
                    .absolute()
                    .left(px(16.0))
                    .bottom(px(14.0))
                    .flex()
                    .items_center()
                    .gap(px(7.0))
                    .h(px(26.0))
                    .px(px(11.0))
                    .rounded(px(13.0))
                    .bg(ba(0.42))
                    .text_size(px(11.5))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(wa(0.85))
                    .child(crate::island::parts::dot(RED, 7.0))
                    .child("Live preview"),
            );
        let hero = div()
            .mt(px(22.0))
            .opacity(swap)
            .child(div().text_size(px(12.0)).font_weight(FontWeight::BOLD).text_color(if ios { hex(0x64d2ff) } else { hex(0x7ee8d0) }).child(eyebrow))
            .child(div().font_family(FONT_DISPLAY).text_size(px(30.0)).font_weight(FontWeight(750.0)).mt(px(6.0)).line_height(px(34.5)).child(t1))
            .child(div().text_color(hex(0x98989d)).mt(px(8.0)).text_size(px(14.0)).line_height(px(22.4)).max_w(px(660.0)).child(lead));

        let mut notices = div().flex().flex_col();
        if h.diag.windhawk_island && h.i("enabled") {
            notices = notices.child(notice(true, "Two islands are running. Your Windhawk mod \"Dynamic Island for Windows\" sits in the same spot. Open Windhawk → My Mods and switch it off."));
        }
        if h.diag.fluent_flyout && h.i("volume") {
            notices = notices.child(notice(false, "FluentFlyout also shows volume pop-ups. To avoid doubles, turn off Volume changes on the Island page or in FluentFlyout."));
        }

        let tile = |id: &'static str, key: &'static str, ic: &'static str, color: u32, label: &'static str, this: &PanelView, cx: &mut Context<Self>| {
            let on = this.get_b(key, cx);
            let t = this.anim(key).clamp(0.0, 1.0);
            let k = if on { t } else { 1.0 - t };
            let c: Hsla = hex(color).into();
            let grey: Hsla = hexa(0x7878804d).into();
            let bg = Hsla { h: c.h, s: c.s * k, l: grey.l + (c.l - grey.l) * k, a: grey.a + (1.0 - grey.a) * k };
            div()
                .id(id)
                .flex_1()
                .flex()
                .flex_col()
                .gap(px(10.0))
                .p(px(14.0))
                .rounded(px(16.0))
                .bg(wa(0.045))
                .border_1()
                .border_color(wa(0.075))
                .cursor_pointer()
                .hover(|s| s.bg(wa(0.07)))
                .child(
                    div()
                        .size(px(34.0))
                        .rounded_full()
                        .bg(bg)
                        .flex()
                        .items_center()
                        .justify_center()
                        .when(on, |d| d.shadow(vec![BoxShadow { color: with_alpha(c, 0.45 * k), offset: point(px(0.), px(6.)), blur_radius: px(18.), spread_radius: px(0.) }]))
                        .child(icon(ic, 21.0, gpui::white())),
                )
                .child(div().text_size(px(13.5)).font_weight(FontWeight::SEMIBOLD).child(label))
                .child(div().text_size(px(12.0)).text_color(hex(0x98989d)).mt(px(-8.0)).child(if on { "On" } else { "Off" }))
                .on_click(cx.listener(move |this, _, _, cx| {
                    let v = !this.get_b(key, cx);
                    this.set(key, Value::Bool(v), cx);
                }))
        };
        let tiles = div()
            .flex()
            .flex_col()
            .gap(px(10.0))
            .child(
                div()
                    .flex()
                    .gap(px(10.0))
                    .child(tile("q-island", "island.enabled", "island", INDIGO, "Island", self, cx))
                    .child(tile("q-pop", "island.trackPopup", "music", PINK, "Music pop-up", self, cx))
                    .child(tile("q-sing", "island.lyricsNotch", "mic", PURPLE, "Sing along", self, cx)),
            )
            .child(
                div()
                    .flex()
                    .gap(px(10.0))
                    .child(tile("q-viz", "island.visualizer", "wave", TEAL, "Visualizer", self, cx))
                    .child(tile("q-hover", "island.hoverExpand", "pointer", ORANGE, "Open on hover", self, cx))
                    .child(tile("q-glow", "island.glow", "sparkle", BLUE, "Album glow", self, cx)),
            );
        let stat = |label: &'static str, value: String, sub: &'static str| {
            div()
                .flex_1()
                .px(px(16.0))
                .py(px(14.0))
                .rounded(px(16.0))
                .bg(wa(0.045))
                .border_1()
                .border_color(wa(0.075))
                .child(div().text_size(px(12.0)).text_color(hex(0x98989d)).font_weight(FontWeight::SEMIBOLD).child(label))
                .child(div().font_family(FONT_DISPLAY).text_size(px(26.0)).font_weight(FontWeight(650.0)).mt(px(4.0)).child(value))
                .child(div().text_size(px(11.5)).text_color(hex(DIM2)).child(sub))
        };
        let stats = div()
            .flex()
            .gap(px(10.0))
            .child(stat("Memory", format!("{} MB", self.mem_mb.round()), "all processes"))
            .child(stat("Idle CPU", "≈ 0%".into(), "event-driven, no polling"))
            .child(stat("Processes", "1".into(), "native GPUI, no WebView2"));
        div()
            .child(div().flex().justify_center().child(stage))
            .child(hero)
            .child(notices)
            .child(sect("Quick settings"))
            .child(tiles)
            .child(sect("Footprint"))
            .child(stats)
            .into_any_element()
    }

    fn slider_row(&self, cx: &mut Context<Self>) -> AnyElement {
        let v = self.hub.read(cx).settings.f("island.collapseDelay").unwrap_or(0.0).clamp(0.0, 10.0);
        let frac = (v / 10.0) as f32;
        let bounds = std::rc::Rc::new(std::cell::Cell::new(Bounds::default()));
        let (b1, b2) = (bounds.clone(), bounds.clone());
        let set_from = |this: &mut PanelView, x: Pixels, b: Bounds<Pixels>, cx: &mut Context<PanelView>| {
            if b.size.width.f() > 0.0 {
                let f = ((x - b.origin.x).f() / b.size.width.f()).clamp(0.0, 1.0);
                let val = (f * 10.0).round() as i64;
                if this.hub.read(cx).settings.f("island.collapseDelay").map(|c| c as i64) != Some(val) {
                    this.hub.update(cx, |h, cx| h.set_setting("island.collapseDelay", Value::from(val), cx));
                }
            }
        };
        div()
            .flex()
            .items_center()
            .gap(px(12.0))
            .w(px(250.0))
            .flex_none()
            .child(
                div()
                    .id("collapse-slider")
                    .relative()
                    .flex_1()
                    .h(px(22.0))
                    .cursor_pointer()
                    .child(crate::island::notch::probe(bounds))
                    .child(div().absolute().top(px(9.0)).left_0().right_0().h(px(4.0)).rounded(px(2.0)).bg(hexa(0x78788066)).child(div().h_full().rounded(px(2.0)).bg(hex(BLUE)).w(relative(frac))))
                    .child(div().absolute().top(px(1.0)).left(relative(frac)).ml(px(-10.0)).size(px(20.0)).rounded_full().bg(gpui::white()).shadow(vec![BoxShadow {
                        color: ba(0.4),
                        offset: point(px(0.), px(2.)),
                        blur_radius: px(6.),
                        spread_radius: px(0.),
                    }]))
                    .on_mouse_down(MouseButton::Left, cx.listener(move |this, e: &MouseDownEvent, _, cx| set_from(this, e.position.x, b1.get(), cx)))
                    .on_mouse_move(cx.listener(move |this, e: &MouseMoveEvent, _, cx| {
                        if e.pressed_button == Some(MouseButton::Left) {
                            set_from(this, e.position.x, b2.get(), cx)
                        }
                    })),
            )
            .child(div().min_w(px(58.0)).text_right().text_size(px(12.5)).text_color(hex(0x98989d)).child(if v == 0.0 { "Instantly".to_string() } else { format!("{} s", v) }))
            .into_any_element()
    }

    fn island_page(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let req = Some("island.enabled");
        let slider = self.slider_row(cx);
        let notif = {
            let s = &self.hub.read(cx).diag.notifications;
            if s == "ok" {
                "Windows toast notifications. Access granted."
            } else if s.is_empty() {
                "Windows toast notifications, when Windows allows access."
            } else {
                "Unavailable: Windows did not grant access to notifications (it usually reserves this for Store apps)."
            }
        };
        let notif_row = {
            let t = self.toggle("t-notif", "island.notifications", cx).into_any_element();
            self.row("bell", RED, "Notifications", Some(notif.into()), t, req, false, cx)
        };
        div()
            .child(title("Island", "Click the island (or hover, if you like) to open the dashboard. Scroll on it to change the volume."))
            .child(sect("Behaviour"))
            .child(
                card()
                    .child(self.tg_row("t-enabled", "island", INDIGO, "Show the island", None, "island.enabled", None, true, cx))
                    .child(self.tg_row("t-hover", "pointer", ORANGE, "Open on hover", Some("Expand the dashboard when the pointer rests on the island, instead of on click."), "island.hoverExpand", req, false, cx))
                    .child(self.row("clock", TEAL, "Close after the pointer leaves", Some("The dashboard closes when the mouse moves off it, even if you opened it with a click.".into()), slider, req, false, cx))
                    .child(self.tg_row("t-fs", "fullscreen", GREEN, "Hide during fullscreen apps", Some("Games and videos keep the top of the screen to themselves."), "island.hideInFullscreen", req, false, cx)),
            )
            .child(sect("Appearance"))
            .child(
                card()
                    .child(self.tg_row("t-idle", "clock", GRAPHITE, "Time & weather when idle", Some("Off gives you a pure, empty island until something happens."), "island.idleInfo", req, true, cx))
                    .child(self.tg_row(
                        "t-glow",
                        "sparkle",
                        BLUE,
                        "Album glow",
                        Some("While music plays, the island picks up the album art's colours (Dynamic Notch; the Dynamic Island capsule stays crisp)."),
                        "island.glow",
                        req,
                        false,
                        cx,
                    ))
                    .child(self.tg_row("t-time", "clock", GREY, "Show the time", None, "island.time", req, false, cx)),
            )
            .child(sect("Live activities & alerts"))
            .child(
                card()
                    .child(self.tg_row("t-vol", "vol", RED, "Volume changes", Some("The island widens with a volume bar whenever the system volume changes."), "island.volume", req, true, cx))
                    .child(self.tg_row("t-priv", "camera", GREEN, "Camera & mic indicators", Some("Green and orange dots while an app is using your camera or microphone."), "island.privacy", req, false, cx))
                    .child(notif_row)
                    .child(self.tg_row("t-sys", "cpu", PURPLE, "System stats", Some("CPU and memory in the dashboard, sampled only while it is open."), "island.system", req, false, cx))
                    .child(self.tg_row("t-events", "tree", GREEN, "Greetings & events", Some("A greeting at start-up and camera/mic alerts."), "island.events", req, false, cx)),
            )
            .into_any_element()
    }

    fn music_page(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let req = Some("island.enabled");
        let media = Some("island.media");
        div()
            .child(title(
                "Music & Lyrics",
                "Works with Spotify, YouTube and any app that shows media controls in Windows. With several players open, pick which one to control from the chips at the top of the dashboard (Dynamic Notch) or the source menu in the Live Activity (Dynamic Island).",
            ))
            .child(sect("Now playing"))
            .child(
                card()
                    .child(self.tg_row("m-media", "music", PINK, "Now playing", Some("Album art, seek bar and playback controls."), "island.media", req, true, cx))
                    .child(self.tg_row("m-pop", "drop", ORANGE, "Music pop-up", Some("Drop down with the album art, title and artist whenever the song changes."), "island.trackPopup", media, false, cx))
                    .child(self.tg_row("m-viz", "wave", TEAL, "Audio visualizer", Some("Bars that move with your real sound output, sampled only while music plays."), "island.visualizer", media, false, cx)),
            )
            .child(sect("Karaoke"))
            .child(
                card()
                    .child(self.tg_row("m-lyr", "mic", PURPLE, "Karaoke lyrics", Some("A Lyrics tab with time-synced lines that fill in as they're sung. Click a line to jump there."), "island.lyrics", media, true, cx))
                    .child(self.tg_row("m-sing", "island", INDIGO, "Sing along in the island", Some("While music plays, the island widens and shows the current line."), "island.lyricsNotch", Some("island.lyrics"), false, cx)),
            )
            .child(foot("Lyrics come from LRCLIB, a free and open-source lyrics database. Only the song's title, artist, album and length are sent, once per song, and the answer is kept on your PC."))
            .into_any_element()
    }

    fn weather_page(&mut self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let s = &self.hub.read(cx).settings;
        let city = s.s("weather.city");
        let now = if city.is_empty() {
            "Not set".to_string()
        } else {
            format!("{} ({:.2}, {:.2})", city, s.f("weather.lat").unwrap_or(0.0), s.f("weather.lon").unwrap_or(0.0))
        };
        let units = s.s("weather.units");
        let focused = self.city.read(cx).focused(window);
        let input = div()
            .h(px(30.0))
            .w(px(200.0))
            .rounded(px(8.0))
            .px(px(12.0))
            .bg(ba(0.25))
            .border_1()
            .border_color(if focused { with_alpha(hex(BLUE), 0.8) } else { wa(0.075) })
            .child(self.city.clone());
        let inline = div()
            .flex()
            .items_center()
            .gap(px(8.0))
            .child(input)
            .child(btn("b-city", "Save", 1).on_click(cx.listener(|this, _, _, cx| this.save_city(cx))))
            .child(btn("b-locate", "Use my location", 0).on_click(cx.listener(|this, _, _, cx| this.locate(cx))));
        let city_row = div()
            .flex()
            .flex_wrap()
            .items_center()
            .gap(px(13.0))
            .min_h(px(46.0))
            .py(px(9.0))
            .child(icon_sq("pin", RED, 24.0, 6.5))
            .child(div().flex_1().min_w(px(120.0)).child(div().text_size(px(13.5)).font_weight(FontWeight::MEDIUM).child("City")).child(div().text_size(px(12.0)).text_color(hex(0x98989d)).mt(px(2.0)).child(now)))
            .child(inline);
        let seg_t = self.anim("weather.units");
        let f_on = units == "f";
        let x = if f_on { seg_t } else { 1.0 - seg_t };
        let seg_btn = |id: &'static str, label: &'static str, val: &'static str, on: bool, cx: &mut Context<Self>| {
            div()
                .id(id)
                .w(px(56.0))
                .py(px(5.0))
                .flex()
                .justify_center()
                .text_size(px(12.5))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(if on { gpui::white() } else { hex(0x98989d).into() })
                .cursor_pointer()
                .child(label)
                .on_click(cx.listener(move |this, _, _, cx| this.set("weather.units", Value::String(val.into()), cx)))
        };
        let seg = div()
            .relative()
            .flex()
            .p(px(2.0))
            .rounded(px(9.0))
            .bg(hexa(0x7676803d))
            .child(div().absolute().top(px(2.0)).bottom(px(2.0)).left(px(2.0 + 56.0 * x)).w(px(56.0)).rounded(px(7.0)).bg(hex(0x636366)).shadow(vec![BoxShadow {
                color: ba(0.25),
                offset: point(px(0.), px(3.)),
                blur_radius: px(8.),
                spread_radius: px(0.),
            }]))
            .child(seg_btn("u-c", "°C", "c", !f_on, cx))
            .child(seg_btn("u-f", "°F", "f", f_on, cx));
        let units_row = self.row("thermo", ORANGE, "Units", None, seg.into_any_element(), None, false, cx);
        div()
            .child(title("Weather", "Shown in the idle island and the dashboard. Data from Open-Meteo (free, no account), refreshed every 30 minutes."))
            .child(sect("Location"))
            .child(card().child(city_row).child(units_row).child(self.tg_row("w-show", "cloud", BLUE, "Show weather", None, "island.weather", Some("island.enabled"), false, cx)))
            .into_any_element()
    }

    fn general_page(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let root = crate::settings::root().display().to_string();
        let data = crate::settings::data().display().to_string();
        let kv = |k: &'static str, v: String| {
            div()
                .flex()
                .gap(px(14.0))
                .child(div().w(px(120.0)).flex_none().text_size(px(12.5)).text_color(hex(0x98989d)).child(k))
                .child(div().flex_1().min_w_0().font_family(FONT_MONO).text_size(px(11.5)).text_color(hex(0xc7c7cc)).child(v))
        };
        let maint_row = |ic: &'static str, color: u32, t: &'static str, d: &'static str, b: AnyElement, first: bool, this: &PanelView, cx: &App| this.row(ic, color, t, Some(d.into()), b, None, first, cx);
        let reset = btn("b-reset", "Reset", 0)
            .on_click(cx.listener(|this, _, _, cx| {
                this.dialog = Some(Dialog {
                    title: "Reset all settings?",
                    text: "Every option returns to its default, and your weather city is cleared.",
                    yes: "Reset",
                    action: |p, cx| p.hub.update(cx, |h, cx| h.reset_settings(cx)),
                });
                cx.notify();
            }))
            .into_any_element();
        let quit = btn("b-quit", "Quit", 0)
            .on_click(cx.listener(|this, _, _, cx| {
                this.dialog = Some(Dialog {
                    title: "Exit WinIsland?",
                    text: "The island closes until your next sign-in (if autostart is on) or until you start it again.",
                    yes: "Exit",
                    action: |_, _| post(Event::Quit),
                });
                cx.notify();
            }))
            .into_any_element();
        let remove = btn("b-remove", "Remove", 2)
            .on_click(cx.listener(|this, _, _, cx| {
                this.dialog = Some(Dialog {
                    title: "Remove from startup?",
                    text: "Autostart is turned off and WinIsland exits. The files stay in place. Uninstall it from Windows Settings → Apps.",
                    yes: "Remove & exit",
                    action: |_, _| post(Event::RestoreQuit),
                });
                cx.notify();
            }))
            .into_any_element();
        let maint = card()
            .child(maint_row("restore", BLUE, "Reset settings", "Return every option to its default.", reset, true, self, cx))
            .child(maint_row("exit", GRAPHITE, "Quit WinIsland", "Close the island until the next sign-in, or until you start it again.", quit, false, self, cx))
            .child(maint_row("exit", RED, "Remove from startup & quit", "Turns off autostart and closes WinIsland. Uninstall it from Windows Settings → Apps.", remove, false, self, cx));
        div()
            .child(title("General", "WinIsland changes nothing on your system except one autostart entry, which you can switch off here."))
            .child(sect("Startup"))
            .child(
                card()
                    .child(self.tg_row("g-auto", "power", GREEN, "Start with Windows", None, "general.autostart", None, true, cx))
                    .child(self.tg_row("g-hotkey", "key", GREY, "Ctrl + Alt + F opens these settings", None, "general.hotkey", None, false, cx)),
            )
            .child(sect("Maintenance"))
            .child(maint)
            .child(sect("Files"))
            .child(
                card().child(
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(9.0))
                        .py(px(14.0))
                        .child(kv("Install folder", root))
                        .child(kv("Settings", format!("{}\\settings.json", data)))
                        .child(kv("Uninstall", "Windows Settings → Apps → Installed apps → WinIsland".into())),
                ),
            )
            .child(foot(
                div()
                    .child(div().font_weight(FontWeight::BOLD).text_color(hex(0x98989d)).child("Made with AI assistance."))
                    .child("Large parts of WinIsland's code, design, installer, website and promo video were generated with Anthropic's Claude, directed and reviewed by the author.")
                    .child(div().mt(px(6.0)).child("WinIsland isn't affiliated with Apple; macOS, iOS and Dynamic Island are trademarks of Apple Inc.")),
            ))
            .into_any_element()
    }

    fn dialog_view(&self, d: &Dialog, cx: &mut Context<Self>) -> AnyElement {
        let action = d.action;
        div()
            .id("dlg")
            .absolute()
            .top_0()
            .left_0()
            .size_full()
            .bg(ba(0.5))
            .flex()
            .items_center()
            .justify_center()
            .occlude()
            .child(
                div()
                    .w(px(380.0))
                    .p(px(22.0))
                    .rounded(px(16.0))
                    .bg(hex(0x2c2c2e))
                    .border_1()
                    .border_color(wa(0.08))
                    .shadow(vec![BoxShadow { color: ba(0.6), offset: point(px(0.), px(30.)), blur_radius: px(80.), spread_radius: px(0.) }])
                    .child(div().text_size(px(16.0)).font_weight(FontWeight(650.0)).mb(px(6.0)).child(d.title))
                    .child(div().text_color(hex(0x98989d)).text_size(px(13.0)).line_height(px(19.5)).child(d.text))
                    .child(
                        div()
                            .flex()
                            .justify_end()
                            .gap(px(8.0))
                            .mt(px(20.0))
                            .child(btn("dlg-no", "Cancel", 0).on_click(cx.listener(|this, _, _, cx| {
                                this.dialog = None;
                                cx.notify();
                            })))
                            .child(btn("dlg-yes", d.yes, 2).on_click(cx.listener(move |this, _, _, cx| {
                                this.dialog = None;
                                action(this, cx);
                                cx.notify();
                            }))),
                    ),
            )
            .into_any_element()
    }
}

fn notice(warn: bool, text: &'static str) -> Div {
    let c = if warn { ORANGE } else { BLUE };
    div()
        .flex()
        .gap(px(12.0))
        .items_start()
        .px(px(14.0))
        .py(px(12.0))
        .rounded(px(12.0))
        .bg(with_alpha(hex(c), if warn { 0.1 } else { 0.08 }))
        .border_1()
        .border_color(with_alpha(hex(c), 0.25))
        .mt(px(16.0))
        .text_size(px(12.5))
        .line_height(px(18.75))
        .child(icon(if warn { "warn" } else { "info" }, 17.0, hex(c)))
        .child(div().flex_1().child(text))
}
