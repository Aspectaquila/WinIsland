//! WinIsland: a Dynamic Island for Windows, in Rust + GPUI.
#![windows_subsystem = "windows"]

mod app;
mod assets;
mod audio;
mod chime;
mod demo;
mod glow;
mod hub;
mod input;
mod island;
mod lyrics;
mod media;
mod model;
mod net;
mod notify;
mod occlusion;
mod panel;
mod privacy;
mod settings;
mod spring;
mod stats;
mod system;
mod theme;
mod util;
mod weather;
mod win;

fn arg_value(args: &[String], name: &str) -> Option<String> {
    args.iter().position(|a| a == name).and_then(|i| args.get(i + 1)).filter(|v| !v.starts_with("--")).cloned()
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let has = |n: &str| args.iter().any(|a| a == n);

    // test hook: `--demo <scene>` (or WINISLAND_DEMO=<scene>) runs the island on demo data without touching the
    // system (no settings writes, no autostart, no hotkey, no tray, no single-instance lock)
    let demo = arg_value(&args, "--demo").or_else(|| if has("--demo") { Some("music".into()) } else { None }).or_else(|| std::env::var("WINISLAND_DEMO").ok().filter(|s| !s.is_empty()));
    let demo_style = arg_value(&args, "--demo-style").or_else(|| std::env::var("WINISLAND_DEMO_STYLE").ok());

    let cmd = if has("--restore") { 2 } else if has("--quit") { 1 } else { 0 };
    if demo.is_none() {
        if !system::single_instance(cmd) {
            return; // already running: the request was forwarded to that instance
        }
        if cmd != 0 {
            if cmd == 2 {
                system::set_autostart(false); // not running: just remove autostart
            }
            return;
        }
    }

    let opts = app::Options { demo, demo_style, panel: has("--panel"), panel_page: arg_value(&args, "--page") };
    gpui::Application::new().with_assets(assets::Assets).run(move |cx| app::run(cx, opts));
}
