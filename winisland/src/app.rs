//! Wiring: services → Hub, the island and settings windows, visibility (fullscreen / lock / screen off),
//! settings side effects (autostart, hotkey, weather), tray and second-instance commands.

use crate::hub::{Hub, HubEvent};
use crate::island::{IslandView, CANVAS_H, CANVAS_W};
use crate::model::Event;
use crate::panel::{Page, PanelView};
use crate::settings::Settings;
use futures::StreamExt;
use gpui::*;
use serde_json::Value;
use std::time::Duration;
use windows::Win32::Foundation::{HWND, RECT};

pub struct Options {
    pub demo: Option<String>,
    pub demo_style: Option<String>,
    pub panel: bool,
    pub panel_page: Option<String>,
}

pub struct AppState {
    pub hub: Entity<Hub>,
    pub island: Option<WindowHandle<IslandView>>,
    pub island_hwnd: Option<HWND>,
    pub glow_hwnd: Option<HWND>,
    pub panel: Option<WindowHandle<PanelView>>,
    pub locked: bool,
    pub display_off: bool,
    pub fullscreen: bool,
    pub demo: bool,
    pub hide_island: bool,
    _subs: Vec<Subscription>,
}

impl Global for AppState {}

pub fn run(cx: &mut App, opts: Options) {
    crate::input::bind_keys(cx);
    let demo = opts.demo.is_some();
    let rx = crate::model::init_channel();

    let hub = if let Some(_) = &opts.demo {
        let style = opts.demo_style.clone().unwrap_or_else(|| Settings::load(true).s("island.style"));
        let style = if style == "island" || style == "ios" { "island" } else { "notch" };
        crate::hub::demo_hub(cx, style, true, false)
    } else {
        let s = Settings::load(false);
        let _ = std::fs::create_dir_all(crate::settings::cache_dir());
        if !crate::settings::file_path().exists() {
            s.save();
        }
        cx.new(|cx| Hub::new(s, None, false, cx))
    };

    if !demo {
        let (hotkey, autostart, enabled) = {
            let s = &hub.read(cx).settings;
            (s.b("general.hotkey"), s.b("general.autostart"), s.b("island.enabled"))
        };
        crate::system::ISLAND_ENABLED.store(enabled, std::sync::atomic::Ordering::SeqCst);
        let msg = crate::system::create_message_window(hotkey);
        crate::occlusion::start(msg);
        if autostart {
            crate::system::set_autostart(true);
        }
        crate::audio::start();
        crate::media::start();
        crate::privacy::start();
        crate::notify::start();
        crate::weather::start(weather_params(&hub.read(cx).settings));
    }

    let subs = vec![cx.subscribe(&hub, |hub, ev: &HubEvent, cx| {
        if let HubEvent::SettingsChanged(patch) = ev {
            on_settings_changed(hub, patch.clone(), cx);
        }
    })];
    cx.set_global(AppState { hub: hub.clone(), island: None, island_hwnd: None, glow_hwnd: None, panel: None, locked: false, display_off: false, fullscreen: false, demo, hide_island: opts.demo.as_deref() == Some("none"), _subs: subs });

    open_island(cx);
    apply_visibility(cx);
    if opts.panel {
        let page = match opts.panel_page.as_deref() {
            Some("island") => Page::Island,
            Some("music") => Page::Music,
            Some("weather") => Page::Weather,
            Some("general") | Some("about") => Page::General,
            _ => Page::Overview,
        };
        open_panel(cx, page);
    }

    // services → UI
    cx.spawn(async move |cx| {
        let mut rx = rx;
        while let Some(ev) = rx.next().await {
            if cx.update(|cx| dispatch(ev, cx)).is_err() {
                break;
            }
        }
    })
    .detach();

    if let Some(scene) = opts.demo.clone() {
        cx.spawn(async move |cx| {
            cx.background_executor().timer(Duration::from_millis(900)).await;
            let _ = cx.update(|cx| {
                let h = cx.global::<AppState>().hub.clone();
                h.update(cx, |_, cx| cx.emit(HubEvent::Scene(scene)));
            });
        })
        .detach();
    }
}

fn weather_params(s: &Settings) -> crate::weather::Params {
    crate::weather::Params { lat: s.f("weather.lat"), lon: s.f("weather.lon"), units_f: s.s("weather.units") == "f", city: s.s("weather.city") }
}

fn dispatch(ev: Event, cx: &mut App) {
    debug_count(&ev);
    let hub = cx.global::<AppState>().hub.clone();
    match ev {
        Event::OpenPanel => open_panel(cx, Page::Overview),
        Event::Quit => quit(cx),
        Event::RestoreQuit => {
            hub.update(cx, |h, cx| h.set_setting("general.autostart", Value::Bool(false), cx));
            crate::system::set_autostart(false);
            quit(cx);
        }
        Event::ToggleIsland => {
            let on = !hub.read(cx).settings.b("island.enabled");
            hub.update(cx, |h, cx| h.set_setting("island.enabled", Value::Bool(on), cx));
        }
        Event::Occlusion { primary_fullscreen } => {
            cx.global_mut::<AppState>().fullscreen = primary_fullscreen;
            apply_visibility(cx);
        }
        Event::Session { locked } => {
            cx.global_mut::<AppState>().locked = locked;
            apply_visibility(cx);
        }
        Event::DisplayPower { on } => {
            cx.global_mut::<AppState>().display_off = !on;
            apply_visibility(cx);
        }
        Event::IslandHover(on) => {
            if let Some(island) = cx.global::<AppState>().island {
                let _ = island.update(cx, |v, _, cx| v.on_hover_change(on, cx));
            }
        }
        Event::DisplayChanged => {
            cx.spawn(async move |cx| {
                cx.background_executor().timer(Duration::from_millis(800)).await;
                let hwnds = cx.update(|cx| (cx.global::<AppState>().glow_hwnd, cx.global::<AppState>().island_hwnd)).unwrap_or((None, None));
                // outside of an App update: gpui re-enters on WM_SIZE
                for h in [hwnds.0, hwnds.1].into_iter().flatten() {
                    crate::win::move_overlay(h, island_rect());
                    crate::win::sync_size(h);
                }
                let _ = cx.update(|cx| {
                    if let Some(i) = cx.global::<AppState>().island {
                        let _ = i.update(cx, |v, _, cx| v.reset_region(cx));
                    }
                    crate::occlusion::invalidate();
                });
            })
            .detach();
        }
        Event::WeatherLocation(patch, err) => match patch {
            Some(p) => {
                // the settings change refreshes the weather (new coordinates → fresh fetch)
                hub.update(cx, |h, cx| h.apply_settings(serde_json::json!({ "weather": p }), cx));
            }
            None => hub.update(cx, |_, cx| cx.emit(HubEvent::Toast(err.to_string()))),
        },
        other => hub.update(cx, |h, cx| h.on_event(other, cx)),
    }
}

fn on_settings_changed(hub: Entity<Hub>, patch: Value, cx: &mut App) {
    let demo = cx.global::<AppState>().demo;
    let s = hub.read(cx).settings.clone();
    if !demo {
        if patch.get("general").is_some() {
            crate::system::set_autostart(s.b("general.autostart"));
            if s.b("general.hotkey") != crate::system::hotkey_on() {
                crate::system::set_hotkey(s.b("general.hotkey"));
            }
        }
        if patch.get("weather").is_some() {
            crate::weather::send(crate::weather::Cmd::Refresh { force: patch["weather"].get("units").is_some(), p: weather_params(&s) });
        }
    }
    crate::system::ISLAND_ENABLED.store(s.b("island.enabled"), std::sync::atomic::Ordering::SeqCst);
    apply_visibility(cx);
}

/// The island window rect in physical pixels: top-centre of the primary monitor.
fn island_rect() -> RECT {
    let (m, scale) = crate::win::primary_monitor();
    let w = (CANVAS_W * scale).round() as i32;
    let h = (CANVAS_H * scale).round() as i32;
    let x = m.left + (m.right - m.left - w) / 2;
    RECT { left: x, top: m.top, right: x + w, bottom: m.top + h }
}

fn open_island(cx: &mut App) {
    let hub = cx.global::<AppState>().hub.clone();
    let r = island_rect();
    let (_, scale) = crate::win::primary_monitor();
    let bounds = Bounds::new(point(px(r.left as f32 / scale), px(r.top as f32 / scale)), size(px(CANVAS_W), px(CANVAS_H)));
    // the album glow: a fully click-through window right behind the island (created first = lower in z-order)
    let mut glow_hwnd = None;
    let glow = cx
        .open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                titlebar: None,
                focus: false,
                show: false,
                kind: WindowKind::PopUp,
                is_movable: false,
                is_resizable: false,
                is_minimizable: false,
                window_background: WindowBackgroundAppearance::Transparent,
                ..Default::default()
            },
            |window, cx| {
                glow_hwnd = crate::win::hwnd_of(window);
                cx.new(|_| crate::glow::GlowView::new(CANVAS_W / 2.0))
            },
        )
        .ok()
        .and_then(|h| h.entity(cx).ok());
    let mut hwnd_out = None;
    let handle = cx.open_window(
        WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            titlebar: None,
            focus: false,
            show: false,
            kind: WindowKind::PopUp,
            is_movable: false,
            is_resizable: false,
            is_minimizable: false,
            window_background: WindowBackgroundAppearance::Transparent,
            app_id: Some("WinIsland".into()),
            ..Default::default()
        },
        |window, cx| {
            let hwnd = crate::win::hwnd_of(window);
            hwnd_out = hwnd;
            cx.new(|cx| {
                let mut v = IslandView::new(hub, hwnd, false, size(CANVAS_W, CANVAS_H), cx);
                v.glow = glow;
                v
            })
        },
    );
    let st = cx.global_mut::<AppState>();
    st.island = handle.ok();
    st.island_hwnd = hwnd_out;
    st.glow_hwnd = glow_hwnd;
    // Win32 calls that make gpui re-enter (WM_SIZE → resize callback) must run outside an App update
    if let Some(h) = hwnd_out {
        cx.spawn(async move |cx| {
            if let Some(g) = glow_hwnd {
                crate::win::make_glow_overlay(g, r);
                crate::win::sync_size(g);
            }
            crate::win::make_overlay(h, r);
            crate::win::sync_size(h);
            crate::win::watch_hover(h);
            let _ = cx.update(|cx| {
                apply_visibility(cx);
                if let Some(i) = cx.global::<AppState>().island {
                    let _ = i.update(cx, |v, _, cx| v.reset_region(cx));
                }
            });
        })
        .detach();
    }
}

/// Hide during fullscreen apps on the main display, while locked and while the screen is off.
pub fn apply_visibility(cx: &mut App) {
    let st = cx.global::<AppState>();
    let hub = st.hub.clone();
    let s = &hub.read(cx).settings;
    let asleep = st.locked || st.display_off;
    let show = !st.hide_island && s.b("island.enabled") && !asleep && !(st.fullscreen && s.b("island.hideInFullscreen"));
    let (g, i) = (st.glow_hwnd, st.island_hwnd);
    // ShowWindow makes gpui re-enter (WM_SHOWWINDOW → draw); run it outside the current App update
    cx.spawn(async move |_| {
        if let Some(g) = g {
            crate::win::show_no_activate(g, show);
        }
        if let Some(h) = i {
            crate::win::show_no_activate(h, show);
        }
    })
    .detach();
    if hub.read(cx).visible != show {
        hub.update(cx, |h, cx| {
            h.visible = show;
            cx.notify();
        });
    }
}

pub fn open_panel(cx: &mut App, page: Page) {
    if let Some(p) = cx.global::<AppState>().panel {
        if p.update(cx, |_, window, _| window.activate_window()).is_ok() {
            return;
        }
    }
    let hub = cx.global::<AppState>().hub.clone();
    // test hook: render the settings window off-screen without activating it (captured with PrintWindow)
    let offscreen = std::env::var("WINISLAND_PANEL_OFFSCREEN").is_ok();
    let wa = crate::win::primary_work_area();
    let (_, scale) = crate::win::primary_monitor();
    let (ww, wh) = ((wa.right - wa.left) as f32 / scale, (wa.bottom - wa.top) as f32 / scale);
    let (w, hgt) = ((ww - 40.0).min(1160.0), (wh - 40.0).min(820.0));
    let h = hgt;
    let origin = point(px(wa.left as f32 / scale + (ww - w) / 2.0), px(wa.top as f32 / scale + (wh - h) / 2.0));
    let handle = cx.open_window(
        WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(Bounds::new(origin, size(px(w), px(h))))),
            titlebar: Some(TitlebarOptions { title: Some("WinIsland — Settings".into()), appears_transparent: false, traffic_light_position: None }),
            focus: !offscreen,
            show: !offscreen,
            kind: WindowKind::Normal,
            is_movable: true,
            is_resizable: true,
            is_minimizable: true,
            window_background: WindowBackgroundAppearance::Transparent,
            window_min_size: Some(size(px(940.0), px(640.0))),
            app_id: Some("WinIsland".into()),
            ..Default::default()
        },
        |window, cx| {
            if let Some(h) = crate::win::hwnd_of(window) {
                crate::win::style_panel(h, true);
                crate::win::set_window_icon(h);
            }
            cx.new(|cx| PanelView::new(hub, page, window, cx))
        },
    );
    if let Ok(handle) = handle {
        cx.global_mut::<AppState>().panel = Some(handle);
        let hw = handle.update(cx, |_, window, _| {
            if !offscreen {
                window.activate_window();
            }
            crate::win::hwnd_of(window)
        });
        if let (true, Ok(Some(hw))) = (offscreen, hw) {
            cx.spawn(async move |_| crate::win::park_offscreen(hw, (w * scale) as i32, (hgt * scale) as i32)).detach();
        }
    }
}

pub fn quit(cx: &mut App) {
    crate::system::shutdown();
    cx.quit();
}

/// WINISLAND_DEBUG=1: log how many events of each kind arrive (to keep an eye on idle wake-ups).
fn debug_count(ev: &Event) {
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::sync::Mutex;
    static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    if !*ON.get_or_init(|| std::env::var("WINISLAND_DEBUG").is_ok()) {
        return;
    }
    static COUNTS: Mutex<Vec<(&'static str, u64)>> = Mutex::new(Vec::new());
    static LAST: AtomicU64 = AtomicU64::new(0);
    let name = match ev {
        Event::Media(_) => "media",
        Event::Volume { .. } => "volume",
        Event::Levels(_) => "levels",
        Event::Weather(_) => "weather",
        Event::Privacy(_) => "privacy",
        Event::Occlusion { .. } => "occlusion",
        Event::Lyrics { .. } => "lyrics",
        _ => "other",
    };
    let mut c = COUNTS.lock().unwrap();
    match c.iter_mut().find(|x| x.0 == name) {
        Some(x) => x.1 += 1,
        None => c.push((name, 1)),
    }
    let now = crate::model::now_ms() as u64 / 1000;
    if now.saturating_sub(LAST.load(Ordering::Relaxed)) >= 10 {
        LAST.store(now, Ordering::Relaxed);
        crate::model::log(&format!("events: {:?} island renders: {}", *c, crate::island::RENDERS.swap(0, Ordering::Relaxed)));
        c.clear();
    }
}
