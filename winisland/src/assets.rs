//! Embedded SVG icons (single stroke style, 24 px grid), served to gpui's `svg()` element.

use gpui::{AssetSource, Result, SharedString};
use std::borrow::Cow;

pub struct Assets;

const S: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="black" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round">"#;
const F: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="black" stroke="none">"#;

fn icon(name: &str) -> Option<String> {
    let (head, body): (&str, &str) = match name {
        "play" => (F, r#"<path d="M8 5.5v13l10.5-6.5z"/>"#),
        "pause" => (F, r#"<rect x="6.5" y="5" width="4" height="14" rx="1.2"/><rect x="13.5" y="5" width="4" height="14" rx="1.2"/>"#),
        "next" => (F, r#"<path d="M5.5 6v12l9-6z"/><rect x="16" y="6" width="2.6" height="12" rx="1"/>"#),
        "prev" => (F, r#"<path d="M18.5 6v12l-9-6z"/><rect x="5.4" y="6" width="2.6" height="12" rx="1"/>"#),
        "fwd" => (F, r#"<path d="M3 6.5v11l8.5-5.5z"/><path d="M12 6.5v11l8.5-5.5z"/>"#),
        "back" => (F, r#"<path d="M21 6.5v11l-8.5-5.5z"/><path d="M12 6.5v11L3.5 12z"/>"#),
        "vol" => (S, r#"<path d="M4 9.5h3.5L12 5.5v13l-4.5-4H4z" fill="black" stroke="none"/><path d="M15.5 9a4.2 4.2 0 0 1 0 6"/><path d="M18.2 6.5a7.8 7.8 0 0 1 0 11"/>"#),
        "volLow" => (S, r#"<path d="M4 9.5h3.5L12 5.5v13l-4.5-4H4z" fill="black" stroke="none"/><path d="M15.5 9a4.2 4.2 0 0 1 0 6"/>"#),
        "mute" => (S, r#"<path d="M4 9.5h3.5L12 5.5v13l-4.5-4H4z" fill="black" stroke="none"/><path d="M16 9.5l5 5M21 9.5l-5 5"/>"#),
        "speaker" => (F, r#"<path d="M3 9h4l5.5-4.5v15L7 15H3z"/><path d="M15.5 8.2a5 5 0 0 1 0 7.6" fill="none" stroke="black" stroke-width="2" stroke-linecap="round"/><path d="M18.3 5.6a8.6 8.6 0 0 1 0 12.8" fill="none" stroke="black" stroke-width="2" stroke-linecap="round"/>"#),
        "speakerMute" => (F, r#"<path d="M3 9h4l5.5-4.5v15L7 15H3z"/><path d="M16 9.5l5 5M21 9.5l-5 5" fill="none" stroke="black" stroke-width="2" stroke-linecap="round"/>"#),
        "gear" => (S, r#"<circle cx="12" cy="12" r="3"/><path d="M19.4 15a1.7 1.7 0 0 0 .3 1.8l.1.1a2 2 0 1 1-2.8 2.8l-.1-.1a1.7 1.7 0 0 0-1.8-.3 1.7 1.7 0 0 0-1 1.5V21a2 2 0 1 1-4 0v-.1a1.7 1.7 0 0 0-1.1-1.5 1.7 1.7 0 0 0-1.8.3l-.1.1a2 2 0 1 1-2.8-2.8l.1-.1a1.7 1.7 0 0 0 .3-1.8 1.7 1.7 0 0 0-1.5-1H3a2 2 0 1 1 0-4h.1a1.7 1.7 0 0 0 1.5-1.1 1.7 1.7 0 0 0-.3-1.8l-.1-.1a2 2 0 1 1 2.8-2.8l.1.1a1.7 1.7 0 0 0 1.8.3H9a1.7 1.7 0 0 0 1-1.5V3a2 2 0 1 1 4 0v.1a1.7 1.7 0 0 0 1 1.5 1.7 1.7 0 0 0 1.8-.3l.1-.1a2 2 0 1 1 2.8 2.8l-.1.1a1.7 1.7 0 0 0-.3 1.8V9a1.7 1.7 0 0 0 1.5 1H21a2 2 0 1 1 0 4h-.1a1.7 1.7 0 0 0-1.5 1z"/>"#),
        "cpu" => (S, r#"<rect x="6" y="6" width="12" height="12" rx="2.5"/><rect x="9.5" y="9.5" width="5" height="5" rx="1"/><path d="M9 3v3M15 3v3M9 18v3M15 18v3M3 9h3M3 15h3M18 9h3M18 15h3"/>"#),
        "bell" => (S, r#"<path d="M6 16V11a6 6 0 1 1 12 0v5l1.5 2h-15z"/><path d="M10 20.5a2.2 2.2 0 0 0 4 0"/>"#),
        "tree" => (S, r#"<path d="M12 2.8 15 7h-1.8l3.3 4.4h-2l3.8 5H5.7l3.8-5h-2L10.8 7H9z"/><path d="M12 16.4V21"/>"#),
        "music" => (S, r#"<path d="M9 18V6l11-2v12"/><circle cx="6.5" cy="18" r="2.5"/><circle cx="17.5" cy="16" r="2.5"/>"#),
        "home" => (S, r#"<path d="M4 11 12 4l8 7v8.5a1 1 0 0 1-1 1h-4.5V15h-5v5.5H5a1 1 0 0 1-1-1z"/>"#),
        "clock" => (S, r#"<circle cx="12" cy="12" r="8.5"/><path d="M12 7.5V12l3 2"/>"#),
        "mic" => (S, r#"<rect x="9" y="3.5" width="6" height="11" rx="3"/><path d="M5.5 11.5a6.5 6.5 0 0 0 13 0M12 18v2.5"/>"#),
        "micFill" => (F, r#"<rect x="8.5" y="2.5" width="7" height="12.5" rx="3.5"/><path d="M5.5 11.5a6.5 6.5 0 0 0 13 0M12 18v3" fill="none" stroke="black" stroke-width="2" stroke-linecap="round"/>"#),
        "camFill" => (F, r#"<rect x="2.5" y="6.5" width="13.5" height="11" rx="3"/><path d="m16.5 10.5 5-3v9l-5-3z"/>"#),
        "x" => (S, r#"<path d="M6.5 6.5l11 11M17.5 6.5l-11 11" stroke-width="2.4"/>"#),
        "check" => (S, r#"<path d="M5 12.5l4.5 4.5L19 7.5" stroke-width="2.2"/>"#),
        "chevDown" => (S, r#"<path d="M7 10l5 5 5-5" stroke-width="2.2"/>"#),
        "chevUp" => (S, r#"<path d="M7 14l5-5 5 5" stroke-width="2.2"/>"#),
        "chevRight" => (S, r#"<path d="M10 7l5 5-5 5" stroke-width="2.2"/>"#),
        "timerFill" => (F, r#"<path d="M9.5 2.5h5v2h-5z"/><circle cx="12" cy="13.5" r="8"/><path d="M12 9.5v4.5l2.5 1.5" fill="none" stroke="white" stroke-width="2" stroke-linecap="round"/>"#),
        // weather
        "clear" => (S, r#"<circle cx="12" cy="12" r="4.2"/><path d="M12 2.5v2M12 19.5v2M2.5 12h2M19.5 12h2M5.3 5.3l1.4 1.4M17.3 17.3l1.4 1.4M5.3 18.7l1.4-1.4M17.3 6.7l1.4-1.4"/>"#),
        "clearNight" => (S, r#"<path d="M20 14.5A8 8 0 1 1 9.5 4a6.5 6.5 0 0 0 10.5 10.5z"/>"#),
        "partly" => (S, r#"<path d="M8.5 4.5v1.2M3.6 9.4h1.2M5 6l.9.9M12 6l-.9.9"/><path d="M5.6 12.4a3.6 3.6 0 1 1 6.3-3"/><path d="M8 19.5h9a3.5 3.5 0 0 0 .3-7 5 5 0 0 0-9.6 1.4A2.8 2.8 0 0 0 8 19.5z"/>"#),
        "cloud" => (S, r#"<path d="M7 18.5h10.5a4 4 0 0 0 .4-8 6 6 0 0 0-11.5 1.6A3.3 3.3 0 0 0 7 18.5z"/>"#),
        "fog" => (S, r#"<path d="M4 9h16M3 13h14M6 17h15"/>"#),
        "rain" => (S, r#"<path d="M7 15h10.5a4 4 0 0 0 .4-8 6 6 0 0 0-11.5 1.6A3.3 3.3 0 0 0 7 15z"/><path d="M9 18l-1 2.5M13 18l-1 2.5M17 18l-1 2.5"/>"#),
        "sleet" => (S, r#"<path d="M7 15h10.5a4 4 0 0 0 .4-8 6 6 0 0 0-11.5 1.6A3.3 3.3 0 0 0 7 15z"/><path d="M9 18l-1 2.5M16 18.5v.1M12.5 20v.1"/>"#),
        "snow" => (S, r#"<path d="M7 14.5h10.5a4 4 0 0 0 .4-8 6 6 0 0 0-11.5 1.6A3.3 3.3 0 0 0 7 14.5z"/><path d="M8.5 18v.1M12 19.5v.1M15.5 18v.1M10 21.5v.1M14 21.5v.1"/>"#),
        "storm" => (S, r#"<path d="M7 15h10.5a4 4 0 0 0 .4-8 6 6 0 0 0-11.5 1.6A3.3 3.3 0 0 0 7 15z"/><path d="M12.5 15.5 10.5 19h3l-2 3.5"/>"#),
        // settings window
        "island" => (S, r#"<path d="M5 4h14v4.5a3.5 3.5 0 0 1-3.5 3.5h-7A3.5 3.5 0 0 1 5 8.5z" fill="black" fill-opacity=".25"/><path d="M8 17h8" opacity=".6"/>"#),
        "sparkle" => (S, r#"<path d="M12 3c.6 4.2 2.8 6.4 7 7-4.2.6-6.4 2.8-7 7-.6-4.2-2.8-6.4-7-7 4.2-.6 6.4-2.8 7-7z"/>"#),
        "fullscreen" => (S, r#"<path d="M4 9V4h5M20 9V4h-5M4 15v5h5M20 15v5h-5"/>"#),
        "wave" => (S, r#"<path d="M4 10v4M8 7v10M12 4v16M16 8v8M20 11v2"/>"#),
        "camera" => (S, r#"<rect x="3" y="6.5" width="13" height="11" rx="2.5"/><path d="m16 10.5 5-3v9l-5-3z"/>"#),
        "pin" => (S, r#"<path d="M12 21s-6.5-6-6.5-11a6.5 6.5 0 0 1 13 0c0 5-6.5 11-6.5 11z"/><circle cx="12" cy="10" r="2.3"/>"#),
        "thermo" => (S, r#"<path d="M10 14.5V5a2 2 0 1 1 4 0v9.5a4 4 0 1 1-4 0z"/>"#),
        "power" => (S, r#"<path d="M12 3.5v8"/><path d="M7 6.5a7.5 7.5 0 1 0 10 0"/>"#),
        "key" => (S, r#"<circle cx="8" cy="14" r="4"/><path d="m11 11 8.5-8.5M16 6l2.5 2.5M14 8l2 2"/>"#),
        "restore" => (S, r#"<path d="M4 12a8 8 0 1 0 2.4-5.7"/><path d="M4 4.5v4h4"/>"#),
        "exit" => (S, r#"<path d="M14 4.5H6.5v15H14"/><path d="M10.5 12H20M16.5 8.5 20 12l-3.5 3.5"/>"#),
        "info" => (S, r#"<circle cx="12" cy="12" r="8.5"/><path d="M12 11v5.5M12 7.8v.1"/>"#),
        "warn" => (S, r#"<path d="M12 4 2.8 19.5h18.4z"/><path d="M12 10v4.5M12 17.3v.1"/>"#),
        "laptop" => (S, r#"<rect x="4" y="5" width="16" height="11" rx="2"/><path d="M10 5h4v1.3a.7.7 0 0 1-.7.7h-2.6a.7.7 0 0 1-.7-.7z" fill="black"/><path d="M2 19h20"/>"#),
        "phone" => (S, r#"<rect x="6.5" y="2.5" width="11" height="19" rx="3"/><rect x="10" y="4.6" width="4" height="1.6" rx=".8" fill="black" stroke="none"/>"#),
        "pointer" => (S, r#"<path d="M6 3.5 18.5 12l-5.6 1.2 3.3 6.1-2.4 1.3-3.3-6.1L6 18.2z"/>"#),
        "drop" => (S, r#"<rect x="4" y="3.5" width="16" height="9" rx="4.5"/><path d="M12 15.5v5M9 18l3 2.5 3-2.5"/>"#),
        _ => return None,
    };
    Some(format!("{head}{body}</svg>"))
}

impl AssetSource for Assets {
    fn load(&self, path: &str) -> Result<Option<Cow<'static, [u8]>>> {
        let name = path.trim_start_matches("icons/").trim_end_matches(".svg");
        Ok(icon(name).map(|s| Cow::Owned(s.into_bytes())))
    }

    fn list(&self, _path: &str) -> Result<Vec<SharedString>> {
        Ok(vec![])
    }
}

pub fn path(name: &str) -> SharedString {
    SharedString::from(format!("icons/{name}.svg"))
}
