//! Win32 plumbing for gpui windows: HWND access, overlay styles, click region, DWM attributes.

use gpui::Window;
use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use windows::Win32::Foundation::{HWND, RECT};
use windows::Win32::Graphics::Dwm::*;
use windows::Win32::Graphics::Gdi::*;
use windows::Win32::UI::HiDpi::{GetDpiForMonitor, MDT_EFFECTIVE_DPI};
use windows::Win32::UI::WindowsAndMessaging::*;

pub fn hwnd_of(window: &Window) -> Option<HWND> {
    let h = HasWindowHandle::window_handle(window).ok()?;
    match h.as_raw() {
        RawWindowHandle::Win32(w) => Some(HWND(w.hwnd.get() as *mut _)),
        _ => None,
    }
}

/// Primary monitor bounds in physical pixels and its scale factor.
pub fn primary_monitor() -> (RECT, f32) {
    unsafe {
        let mon = MonitorFromPoint(windows::Win32::Foundation::POINT { x: 1, y: 1 }, MONITOR_DEFAULTTOPRIMARY);
        let mut mi = MONITORINFO { cbSize: std::mem::size_of::<MONITORINFO>() as u32, ..Default::default() };
        let _ = GetMonitorInfoW(mon, &mut mi);
        let (mut dx, mut dy) = (96u32, 96u32);
        let _ = GetDpiForMonitor(mon, MDT_EFFECTIVE_DPI, &mut dx, &mut dy);
        // capture hook: must match the forced gpui scale (see vendor/gpui window.rs)
        let forced = std::env::var("WINISLAND_CAPTURE_SCALE").ok().and_then(|s| s.parse::<f32>().ok());
        (mi.rcMonitor, forced.unwrap_or(dx as f32 / 96.0))
    }
}

pub fn primary_work_area() -> RECT {
    unsafe {
        let mon = MonitorFromPoint(windows::Win32::Foundation::POINT { x: 1, y: 1 }, MONITOR_DEFAULTTOPRIMARY);
        let mut mi = MONITORINFO { cbSize: std::mem::size_of::<MONITORINFO>() as u32, ..Default::default() };
        let _ = GetMonitorInfoW(mon, &mut mi);
        mi.rcWork
    }
}

/// Turn a gpui popup into the island overlay: no taskbar entry, never activates, always on top. Shown without
/// activation at the given physical rect.
static CLIENT_DX: std::sync::atomic::AtomicI32 = std::sync::atomic::AtomicI32::new(0);
static CLIENT_DY: std::sync::atomic::AtomicI32 = std::sync::atomic::AtomicI32::new(0);

/// Position the window so that its *client area* covers `r` (gpui insets the client area of frameless popups by
/// the resize-frame thickness), and remember the client offset for the window region.
fn place_client(hwnd: HWND, r: RECT, flags: SET_WINDOW_POS_FLAGS) {
    use std::sync::atomic::Ordering;
    unsafe {
        let (w, h) = (r.right - r.left, r.bottom - r.top);
        let _ = SetWindowPos(hwnd, Some(HWND_TOPMOST), r.left, r.top, w, h, flags);
        let (mut wr, mut cr) = (RECT::default(), RECT::default());
        let _ = GetWindowRect(hwnd, &mut wr);
        let _ = GetClientRect(hwnd, &mut cr);
        let mut pt = windows::Win32::Foundation::POINT { x: 0, y: 0 };
        let _ = windows::Win32::Graphics::Gdi::ClientToScreen(hwnd, &mut pt);
        let (dx, dy) = (pt.x - wr.left, pt.y - wr.top);
        let (ew, eh) = ((wr.right - wr.left) - cr.right, (wr.bottom - wr.top) - cr.bottom);
        if dx != 0 || dy != 0 || ew != 0 || eh != 0 {
            let _ = SetWindowPos(hwnd, Some(HWND_TOPMOST), r.left - dx, r.top - dy, w + ew, h + eh, SWP_NOACTIVATE);
        }
        CLIENT_DX.store(dx, Ordering::SeqCst);
        CLIENT_DY.store(dy, Ordering::SeqCst);
    }
}

pub fn make_overlay(hwnd: HWND, r: RECT) {
    unsafe {
        let ex = GetWindowLongPtrW(hwnd, GWL_EXSTYLE) as u32;
        let ex = (ex | WS_EX_TOOLWINDOW.0 | WS_EX_NOACTIVATE.0 | WS_EX_TOPMOST.0) & !WS_EX_APPWINDOW.0;
        SetWindowLongPtrW(hwnd, GWL_EXSTYLE, ex as isize);
        let corner = DWMWCP_DONOTROUND;
        let _ = DwmSetWindowAttribute(hwnd, DWMWA_WINDOW_CORNER_PREFERENCE, &corner as *const _ as _, 4);
        // no DWM transition animations when showing / hiding
        let on: i32 = 1;
        let _ = DwmSetWindowAttribute(hwnd, DWMWA_TRANSITIONS_FORCEDISABLED, &on as *const _ as _, 4);
        set_region(hwnd, &[]);
    }
    place_client(hwnd, r, SWP_NOACTIVATE | SWP_FRAMECHANGED);
}

pub fn move_overlay(hwnd: HWND, r: RECT) {
    place_client(hwnd, r, SWP_NOACTIVATE);
}

pub fn show_no_activate(hwnd: HWND, show: bool) {
    unsafe {
        let _ = ShowWindow(hwnd, if show { SW_SHOWNOACTIVATE } else { SW_HIDE });
        if show {
            let _ = SetWindowPos(hwnd, Some(HWND_TOPMOST), 0, 0, 0, 0, SWP_NOACTIVATE | SWP_NOMOVE | SWP_NOSIZE);
        }
    }
}

/// One clickable shape in device pixels (relative to the window).
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Shape {
    Rect { l: i32, t: i32, r: i32, b: i32 },
    Round { l: i32, t: i32, r: i32, b: i32, radius: i32 },
}

/// Clip the window's hit-test (and visible) area to the island shapes; everything else is click-through.
pub fn set_region(hwnd: HWND, shapes: &[Shape]) {
    unsafe {
        let (ox, oy) = (CLIENT_DX.load(std::sync::atomic::Ordering::SeqCst), CLIENT_DY.load(std::sync::atomic::Ordering::SeqCst));
        let rgn = CreateRectRgn(0, 0, 0, 0);
        for s in shapes {
            let part = match *s {
                Shape::Rect { l, t, r, b } => CreateRectRgn(l + ox, t + oy, r + ox, b + oy),
                Shape::Round { l, t, r, b, radius } => CreateRoundRectRgn(l + ox, t + oy, r + ox + 1, b + oy + 1, radius * 2, radius * 2),
            };
            CombineRgn(Some(rgn), Some(rgn), Some(part), RGN_OR);
            let _ = DeleteObject(part.into());
        }
        if SetWindowRgn(hwnd, Some(rgn), false) == 0 {
            let _ = DeleteObject(rgn.into());
        }
    }
}

/// Dark title bar, Mica backdrop, rounded corners and a caption colour for a normal window.
pub fn style_panel(hwnd: HWND, mica: bool) {
    unsafe {
        let on: i32 = 1;
        let _ = DwmSetWindowAttribute(hwnd, DWMWA_USE_IMMERSIVE_DARK_MODE, &on as *const _ as _, 4);
        let round = DWMWCP_ROUND;
        let _ = DwmSetWindowAttribute(hwnd, DWMWA_WINDOW_CORNER_PREFERENCE, &round as *const _ as _, 4);
        if mica {
            let backdrop = DWMSBT_MAINWINDOW; // Mica
            let _ = DwmSetWindowAttribute(hwnd, DWMWA_SYSTEMBACKDROP_TYPE, &backdrop as *const _ as _, 4);
            let m = windows::Win32::UI::Controls::MARGINS { cxLeftWidth: -1, cxRightWidth: -1, cyTopHeight: -1, cyBottomHeight: -1 };
            let _ = DwmExtendFrameIntoClientArea(hwnd, &m);
        } else {
            let caption: u32 = 0x001E1C1C;
            let _ = DwmSetWindowAttribute(hwnd, DWMWA_CAPTION_COLOR, &caption as *const _ as _, 4);
        }
    }
}

pub fn set_window_icon(hwnd: HWND) {
    unsafe {
        let module = windows::Win32::System::LibraryLoader::GetModuleHandleW(None).unwrap_or_default();
        for (kind, size) in [(ICON_BIG, 32), (ICON_SMALL, 16)] {
            if let Ok(h) = LoadImageW(Some(module.into()), windows::core::PCWSTR(1 as _), IMAGE_ICON, size, size, LR_SHARED) {
                SendMessageW(hwnd, WM_SETICON, Some(windows::Win32::Foundation::WPARAM(kind as usize)), Some(windows::Win32::Foundation::LPARAM(h.0 as isize)));
            }
        }
    }
}

/// Test hook: show a window far off-screen without activating it.
pub fn park_offscreen(hwnd: HWND, w: i32, h: i32) {
    unsafe {
        // cloaked: DWM keeps the content up to date but never shows it on screen
        let peek = std::env::var("WINISLAND_PANEL_PEEK").is_ok();
        if !peek {
            let on: i32 = 1;
            let _ = DwmSetWindowAttribute(hwnd, DWMWA_CLOAK, &on as *const _ as _, 4);
        }
        let (x, y) = if peek { (2560 - w + 8, 1440 - h - 40) } else { (200, 100) };
        let _ = SetWindowPos(hwnd, if peek { Some(HWND_TOPMOST) } else { None }, x, y, w, h, SWP_NOACTIVATE | if peek { SET_WINDOW_POS_FLAGS(0) } else { SWP_NOZORDER });
        let _ = ShowWindow(hwnd, SW_SHOWNOACTIVATE);
    }
    sync_size(hwnd);
}

/// gpui learns its size only from WM_SIZE; replay it so its layout matches the real client area.
pub fn sync_size(hwnd: HWND) {
    unsafe {
        let mut cr = RECT::default();
        let _ = GetClientRect(hwnd, &mut cr);
        let lp = ((cr.bottom as u32 & 0xFFFF) << 16) | (cr.right as u32 & 0xFFFF);
        SendMessageW(hwnd, WM_SIZE, Some(windows::Win32::Foundation::WPARAM(0)), Some(windows::Win32::Foundation::LPARAM(lp as isize)));
    }
}

/// The glow window: like the overlay, but fully click-through (layered + transparent) and without a region.
pub fn make_glow_overlay(hwnd: HWND, r: RECT) {
    unsafe {
        let ex = GetWindowLongPtrW(hwnd, GWL_EXSTYLE) as u32;
        let ex = (ex | WS_EX_TOOLWINDOW.0 | WS_EX_NOACTIVATE.0 | WS_EX_TOPMOST.0 | WS_EX_LAYERED.0 | WS_EX_TRANSPARENT.0) & !WS_EX_APPWINDOW.0;
        SetWindowLongPtrW(hwnd, GWL_EXSTYLE, ex as isize);
        let corner = DWMWCP_DONOTROUND;
        let _ = DwmSetWindowAttribute(hwnd, DWMWA_WINDOW_CORNER_PREFERENCE, &corner as *const _ as _, 4);
        let on: i32 = 1;
        let _ = DwmSetWindowAttribute(hwnd, DWMWA_TRANSITIONS_FORCEDISABLED, &on as *const _ as _, 4);
    }
    place_client(hwnd, r, SWP_NOACTIVATE | SWP_FRAMECHANGED);
}

static HOVERED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// gpui reports the pointer leaving a window only at window level (not to element hover listeners), so watch
/// WM_MOUSEMOVE / WM_MOUSELEAVE on the island window ourselves. Its region is exactly the island shapes, so
/// "pointer in the window" is "pointer on the island".
pub fn watch_hover(hwnd: HWND) {
    unsafe {
        let _ = windows::Win32::UI::Shell::SetWindowSubclass(hwnd, Some(hover_proc), 0x4953, 0);
    }
}

unsafe extern "system" fn hover_proc(
    hwnd: HWND,
    msg: u32,
    wp: windows::Win32::Foundation::WPARAM,
    lp: windows::Win32::Foundation::LPARAM,
    _id: usize,
    _data: usize,
) -> windows::Win32::Foundation::LRESULT {
    use std::sync::atomic::Ordering;
    match msg {
        WM_MOUSEMOVE => {
            if !HOVERED.swap(true, Ordering::SeqCst) {
                crate::model::post(crate::model::Event::IslandHover(true));
            }
        }
        0x02A3 => {
            // WM_MOUSELEAVE
            if HOVERED.swap(false, Ordering::SeqCst) {
                crate::model::post(crate::model::Event::IslandHover(false));
            }
        }
        _ => {}
    }
    windows::Win32::UI::Shell::DefSubclassProc(hwnd, msg, wp, lp)
}
