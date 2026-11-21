//! Is a fullscreen app covering the primary display? Entirely event-driven (WinEvent hooks on the UI thread),
//! debounced with a window timer. No polling.

use crate::model::{post, Event};
use std::sync::atomic::{AtomicBool, AtomicIsize, Ordering};
use windows::Win32::Foundation::*;
use windows::Win32::Graphics::Dwm::{DwmGetWindowAttribute, DWMWA_CLOAKED};
use windows::Win32::Graphics::Gdi::*;
use windows::Win32::UI::Accessibility::{SetWinEventHook, UnhookWinEvent, HWINEVENTHOOK};
use windows::Win32::UI::Shell::{SHQueryUserNotificationState, QUNS_PRESENTATION_MODE, QUNS_RUNNING_D3D_FULL_SCREEN};
use windows::Win32::UI::WindowsAndMessaging::*;

pub const TIMER_ID: usize = 0x0CC1;
static TIMER_HWND: AtomicIsize = AtomicIsize::new(0);
static PENDING: AtomicBool = AtomicBool::new(false);
static LAST: AtomicIsize = AtomicIsize::new(-1);
static FORCED: AtomicBool = AtomicBool::new(true);
static mut HOOKS: Vec<HWINEVENTHOOK> = Vec::new();

const EVENT_SYSTEM_FOREGROUND: u32 = 3;
const EVENT_SYSTEM_MOVESIZEEND: u32 = 0xB;
const EVENT_SYSTEM_MINIMIZESTART: u32 = 0x16;
const EVENT_SYSTEM_MINIMIZEEND: u32 = 0x17;
const EVENT_OBJECT_DESTROY: u32 = 0x8001;
const EVENT_OBJECT_SHOW: u32 = 0x8002;
const EVENT_OBJECT_HIDE: u32 = 0x8003;
const EVENT_OBJECT_LOCATIONCHANGE: u32 = 0x800B;
const EVENT_OBJECT_CLOAKED: u32 = 0x8017;
const EVENT_OBJECT_UNCLOAKED: u32 = 0x8018;

/// Install the hooks. `timer_hwnd` is the message window that receives the debounce WM_TIMER.
pub fn start(timer_hwnd: HWND) {
    TIMER_HWND.store(timer_hwnd.0 as isize, Ordering::SeqCst);
    unsafe {
        let flags = 0x0000 | 0x0002; // WINEVENT_OUTOFCONTEXT | WINEVENT_SKIPOWNPROCESS
        let hook = |a: u32, b: u32| {
            let h = SetWinEventHook(a, b, None, Some(on_event), 0, 0, flags);
            if !h.is_invalid() {
                #[allow(static_mut_refs)]
                HOOKS.push(h);
            }
        };
        hook(EVENT_SYSTEM_FOREGROUND, EVENT_SYSTEM_FOREGROUND);
        hook(EVENT_SYSTEM_MOVESIZEEND, EVENT_SYSTEM_MOVESIZEEND);
        hook(EVENT_SYSTEM_MINIMIZESTART, EVENT_SYSTEM_MINIMIZEEND);
        hook(EVENT_OBJECT_DESTROY, EVENT_OBJECT_HIDE);
        hook(EVENT_OBJECT_LOCATIONCHANGE, EVENT_OBJECT_LOCATIONCHANGE);
        hook(EVENT_OBJECT_CLOAKED, EVENT_OBJECT_UNCLOAKED);
    }
    invalidate();
}

pub fn stop() {
    unsafe {
        #[allow(static_mut_refs)]
        for h in HOOKS.drain(..) {
            let _ = UnhookWinEvent(h);
        }
    }
}

fn schedule() {
    if PENDING.swap(true, Ordering::SeqCst) {
        return;
    }
    let h = HWND(TIMER_HWND.load(Ordering::SeqCst) as *mut _);
    unsafe {
        SetTimer(Some(h), TIMER_ID, 220, None);
    }
}

pub fn invalidate() {
    FORCED.store(true, Ordering::SeqCst);
    PENDING.store(false, Ordering::SeqCst);
    schedule();
}

unsafe extern "system" fn on_event(_: HWINEVENTHOOK, evt: u32, hwnd: HWND, id_object: i32, id_child: i32, _: u32, _: u32) {
    if id_object != 0 || id_child != 0 || hwnd.0.is_null() {
        return; // only whole windows
    }
    if matches!(evt, EVENT_OBJECT_LOCATIONCHANGE | EVENT_OBJECT_SHOW | EVENT_OBJECT_HIDE) && GetAncestor(hwnd, GA_PARENT) != GetDesktopWindow() {
        return; // these fire constantly for child controls; only top-level windows matter
    }
    schedule();
}

/// Called from the message window on WM_TIMER.
pub fn on_timer(h: HWND) {
    unsafe {
        let _ = KillTimer(Some(h), TIMER_ID);
    }
    PENDING.store(false, Ordering::SeqCst);
    let fs = evaluate();
    let v = fs as isize;
    let forced = FORCED.swap(false, Ordering::SeqCst);
    if LAST.swap(v, Ordering::SeqCst) != v || forced {
        post(Event::Occlusion { primary_fullscreen: fs });
    }
}

struct Ctx {
    mon: RECT,
    shell: HWND,
    pid: u32,
    fullscreen: bool,
}

fn class_name(h: HWND) -> String {
    let mut buf = [0u16; 256];
    let n = unsafe { GetClassNameW(h, &mut buf) };
    String::from_utf16_lossy(&buf[..n.max(0) as usize])
}

unsafe extern "system" fn enum_cb(h: HWND, lp: LPARAM) -> windows::core::BOOL {
    let ctx = &mut *(lp.0 as *mut Ctx);
    if !IsWindowVisible(h).as_bool() || IsIconic(h).as_bool() {
        return TRUE;
    }
    let mut cloaked = 0i32;
    if DwmGetWindowAttribute(h, DWMWA_CLOAKED, &mut cloaked as *mut _ as _, 4).is_ok() && cloaked != 0 {
        return TRUE;
    }
    let mut pid = 0u32;
    GetWindowThreadProcessId(h, Some(&mut pid));
    if pid == ctx.pid || h == ctx.shell {
        return TRUE;
    }
    let ex = GetWindowLongPtrW(h, GWL_EXSTYLE) as u32;
    if ex & WS_EX_TRANSPARENT.0 != 0 && ex & WS_EX_LAYERED.0 != 0 {
        return TRUE; // click-through overlays (Steam/NVIDIA/etc.)
    }
    if ex & WS_EX_TOOLWINDOW.0 != 0 && ex & WS_EX_APPWINDOW.0 == 0 {
        return TRUE;
    }
    let cls = class_name(h);
    if matches!(
        cls.as_str(),
        "Progman" | "WorkerW" | "Shell_TrayWnd" | "Shell_SecondaryTrayWnd" | "Windows.UI.Core.CoreWindow" | "XamlExplorerHostIslandWindow" | "TopLevelWindowForOverflowXamlIsland"
    ) {
        return TRUE;
    }
    let mut r = RECT::default();
    if GetWindowRect(h, &mut r).is_err() {
        return TRUE;
    }
    let m = ctx.mon;
    if r.left <= m.left && r.top <= m.top && r.right >= m.right && r.bottom >= m.bottom {
        ctx.fullscreen = true;
        return FALSE;
    }
    TRUE
}

fn evaluate() -> bool {
    unsafe {
        let (mon, _) = crate::win::primary_monitor();
        let mut ctx = Ctx { mon, shell: GetShellWindow(), pid: std::process::id(), fullscreen: false };
        let _ = EnumWindows(Some(enum_cb), LPARAM(&mut ctx as *mut _ as isize));
        if !ctx.fullscreen {
            // Exclusive fullscreen D3D apps may not report a covering window rect reliably.
            if let Ok(q) = SHQueryUserNotificationState() {
                if q == QUNS_RUNNING_D3D_FULL_SCREEN || q == QUNS_PRESENTATION_MODE {
                    let fg = GetForegroundWindow();
                    let m = MonitorFromWindow(fg, MONITOR_DEFAULTTONEAREST);
                    let mut mi = MONITORINFO { cbSize: std::mem::size_of::<MONITORINFO>() as u32, ..Default::default() };
                    if GetMonitorInfoW(m, &mut mi).as_bool() && mi.dwFlags & 1 != 0 {
                        ctx.fullscreen = true;
                    }
                }
            }
        }
        ctx.fullscreen
    }
}
