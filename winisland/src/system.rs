//! Hidden message window (on the UI thread, pumped by gpui's message loop): tray icon, Ctrl+Alt+F hotkey,
//! display changes, lock/unlock, display power, commands forwarded from a second instance, occlusion timer.
//! Plus single-instance and autostart helpers.

use crate::model::{post, Event};
use crate::util::wide;
use std::sync::atomic::{AtomicBool, AtomicIsize, AtomicU32, Ordering};
use windows::core::{GUID, PCWSTR};
use windows::Win32::Foundation::*;
use windows::Win32::System::LibraryLoader::{GetModuleHandleW, GetProcAddress, LoadLibraryW};
use windows::Win32::System::Power::{RegisterPowerSettingNotification, POWERBROADCAST_SETTING};
use windows::Win32::System::Registry::*;
use windows::Win32::System::RemoteDesktop::{WTSRegisterSessionNotification, NOTIFY_FOR_THIS_SESSION};
use windows::Win32::System::Threading::CreateMutexW;
use windows::Win32::UI::Input::KeyboardAndMouse::{RegisterHotKey, UnregisterHotKey, MOD_ALT, MOD_CONTROL, MOD_NOREPEAT};
use windows::Win32::UI::Shell::*;
use windows::Win32::UI::WindowsAndMessaging::*;

pub const CLASS: &str = "WinIslandRs.MessageWindow";
const WM_TRAY: u32 = WM_APP + 1;
const GUID_CONSOLE_DISPLAY_STATE: GUID = GUID::from_u128(0x6FE69556_704A_47A0_8F24_C28D936FDA47);
const RUN_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";
const RUN_NAME: &str = "FrostDesktop";

static HWND_MSG: AtomicIsize = AtomicIsize::new(0);
static MSG_OPEN: AtomicU32 = AtomicU32::new(0);
static MSG_CMD: AtomicU32 = AtomicU32::new(0);
static MSG_TASKBAR: AtomicU32 = AtomicU32::new(0);
pub static ISLAND_ENABLED: AtomicBool = AtomicBool::new(true);
static HOTKEY_ON: AtomicBool = AtomicBool::new(false);
static TRAY_ON: AtomicBool = AtomicBool::new(false);

fn reg_msg(s: &str) -> u32 {
    let w = wide(s);
    unsafe { RegisterWindowMessageW(PCWSTR(w.as_ptr())) }
}

/// Returns false if another instance already runs (the command was forwarded to it).
/// `cmd`: 0 = open settings, 1 = quit, 2 = restore (remove autostart) and quit.
pub fn single_instance(cmd: u32) -> bool {
    let name = wide(r"Local\WinIslandRs.Singleton");
    unsafe {
        let h = CreateMutexW(None, true, PCWSTR(name.as_ptr()));
        let exists = GetLastError() == ERROR_ALREADY_EXISTS;
        if h.is_ok() && !exists {
            std::mem::forget(h); // held for the lifetime of the process
            return true;
        }
        let cls = wide(CLASS);
        if let Ok(w) = FindWindowW(PCWSTR(cls.as_ptr()), PCWSTR::null()) {
            let m = if cmd == 0 { reg_msg("WinIslandRs.OpenPanel") } else { reg_msg("WinIslandRs.Command") };
            let _ = PostMessageW(Some(w), m, WPARAM(cmd as usize), LPARAM(0));
        }
        false
    }
}

pub fn set_autostart(on: bool) {
    unsafe {
        let key = wide(RUN_KEY);
        let mut k = HKEY::default();
        if RegOpenKeyExW(HKEY_CURRENT_USER, PCWSTR(key.as_ptr()), Some(0), KEY_READ | KEY_WRITE, &mut k).is_err() {
            return;
        }
        let name = wide(RUN_NAME);
        if on {
            let val = wide(&format!("\"{}\" --autostart", crate::util::exe_path()));
            let bytes = std::slice::from_raw_parts(val.as_ptr() as *const u8, val.len() * 2);
            let _ = RegSetValueExW(k, PCWSTR(name.as_ptr()), Some(0), REG_SZ, Some(bytes));
        } else {
            let _ = RegDeleteValueW(k, PCWSTR(name.as_ptr()));
        }
        let _ = RegCloseKey(k);
    }
}

/// Is the Windhawk "Dynamic Island for Windows" mod also running (two islands in the same spot)?
pub fn windhawk_island_active() -> bool {
    unsafe {
        let p = wide(r"SOFTWARE\Windhawk\Engine\Mods\dynamic-island-for-windows");
        let mut k = HKEY::default();
        if RegOpenKeyExW(HKEY_LOCAL_MACHINE, PCWSTR(p.as_ptr()), Some(0), KEY_READ, &mut k).is_err() {
            return false;
        }
        let n = wide("Disabled");
        let mut v = 0u32;
        let mut size = 4u32;
        let r = RegQueryValueExW(k, PCWSTR(n.as_ptr()), None, None, Some(&mut v as *mut u32 as *mut u8), Some(&mut size));
        let _ = RegCloseKey(k);
        !r.is_ok() || v == 0
    }
}

pub fn fluent_flyout_running() -> bool {
    crate::util::process_running(&["FluentFlyout"]).is_some()
}

/// Dark context menus (undocumented uxtheme ordinals, the same ones Explorer uses).
fn enable_dark_menus() {
    unsafe {
        let lib = wide("uxtheme.dll");
        if let Ok(m) = LoadLibraryW(PCWSTR(lib.as_ptr())) {
            if let Some(f) = GetProcAddress(m, windows::core::PCSTR(135 as *const u8)) {
                let set_preferred_app_mode: extern "system" fn(i32) -> i32 = std::mem::transmute(f);
                set_preferred_app_mode(2); // ForceDark
            }
            if let Some(f) = GetProcAddress(m, windows::core::PCSTR(136 as *const u8)) {
                let flush: extern "system" fn() = std::mem::transmute(f);
                flush();
            }
        }
    }
}

pub fn create_message_window(hotkey: bool) -> HWND {
    unsafe {
        MSG_OPEN.store(reg_msg("WinIslandRs.OpenPanel"), Ordering::SeqCst);
        MSG_CMD.store(reg_msg("WinIslandRs.Command"), Ordering::SeqCst);
        MSG_TASKBAR.store(reg_msg("TaskbarCreated"), Ordering::SeqCst);
        enable_dark_menus();
        let inst = GetModuleHandleW(None).unwrap_or_default();
        let cls = wide(CLASS);
        let wc = WNDCLASSW { lpfnWndProc: Some(wndproc), hInstance: inst.into(), lpszClassName: PCWSTR(cls.as_ptr()), ..Default::default() };
        RegisterClassW(&wc);
        let hwnd = CreateWindowExW(
            WS_EX_TOOLWINDOW,
            PCWSTR(cls.as_ptr()),
            PCWSTR(cls.as_ptr()),
            WS_POPUP,
            0,
            0,
            0,
            0,
            None,
            None,
            Some(inst.into()),
            None,
        )
        .unwrap_or_default();
        HWND_MSG.store(hwnd.0 as isize, Ordering::SeqCst);
        let _ = WTSRegisterSessionNotification(hwnd, NOTIFY_FOR_THIS_SESSION);
        let _ = RegisterPowerSettingNotification(HANDLE(hwnd.0), &GUID_CONSOLE_DISPLAY_STATE, REGISTER_NOTIFICATION_FLAGS(0));
        set_hotkey(hotkey);
        add_tray();
        hwnd
    }
}

pub fn message_hwnd() -> HWND {
    HWND(HWND_MSG.load(Ordering::SeqCst) as *mut _)
}

pub fn set_hotkey(on: bool) {
    let h = message_hwnd();
    unsafe {
        let _ = UnregisterHotKey(Some(h), 1);
        // Ctrl + Alt + F → island settings
        let ok = on && RegisterHotKey(Some(h), 1, MOD_CONTROL | MOD_ALT | MOD_NOREPEAT, 'F' as u32).is_ok();
        HOTKEY_ON.store(ok, Ordering::SeqCst);
    }
}

pub fn hotkey_on() -> bool {
    HOTKEY_ON.load(Ordering::SeqCst)
}

fn tray_data() -> NOTIFYICONDATAW {
    let mut nid = NOTIFYICONDATAW {
        cbSize: std::mem::size_of::<NOTIFYICONDATAW>() as u32,
        hWnd: message_hwnd(),
        uID: 1,
        uFlags: NIF_MESSAGE | NIF_ICON | NIF_TIP,
        uCallbackMessage: WM_TRAY,
        ..Default::default()
    };
    unsafe {
        let inst = GetModuleHandleW(None).unwrap_or_default();
        let cx = GetSystemMetrics(SM_CXSMICON);
        if let Ok(h) = LoadImageW(Some(inst.into()), PCWSTR(1 as _), IMAGE_ICON, cx, cx, LR_DEFAULTCOLOR) {
            nid.hIcon = HICON(h.0);
        }
    }
    let tip = wide("WinIsland");
    nid.szTip[..tip.len()].copy_from_slice(&tip);
    nid
}

fn add_tray() {
    let mut nid = tray_data();
    unsafe {
        let ok = Shell_NotifyIconW(NIM_ADD, &nid).as_bool();
        if std::env::var("WINISLAND_DEBUG").is_ok() {
            crate::model::log(&format!("tray added: {ok}, hotkey: {}", hotkey_on()));
        }
        if ok {
            TRAY_ON.store(true, Ordering::SeqCst);
            nid.Anonymous.uVersion = NOTIFYICON_VERSION_4;
            let _ = Shell_NotifyIconW(NIM_SETVERSION, &nid);
        }
    }
}

pub fn remove_tray() {
    if TRAY_ON.swap(false, Ordering::SeqCst) {
        let nid = tray_data();
        unsafe {
            let _ = Shell_NotifyIconW(NIM_DELETE, &nid);
        }
    }
}

pub fn shutdown() {
    remove_tray();
    crate::occlusion::stop();
    unsafe {
        let _ = UnregisterHotKey(Some(message_hwnd()), 1);
    }
}

fn show_tray_menu(hwnd: HWND) {
    unsafe {
        let Ok(menu) = CreatePopupMenu() else { return };
        let item = |id: usize, text: &str, flags: MENU_ITEM_FLAGS| {
            let w = wide(text);
            let _ = AppendMenuW(menu, flags, id, PCWSTR(w.as_ptr()));
        };
        item(1, "Island settings", MF_STRING);
        let checked = if ISLAND_ENABLED.load(Ordering::SeqCst) { MF_CHECKED } else { MF_UNCHECKED };
        item(2, "Show island", MF_STRING | checked);
        let _ = AppendMenuW(menu, MF_SEPARATOR, 0, PCWSTR::null());
        item(3, "Exit", MF_STRING);
        let _ = SetMenuDefaultItem(menu, 1, 0);
        let mut pt = POINT::default();
        let _ = GetCursorPos(&mut pt);
        let _ = SetForegroundWindow(hwnd);
        let cmd = TrackPopupMenu(menu, TPM_RETURNCMD | TPM_RIGHTBUTTON | TPM_BOTTOMALIGN, pt.x, pt.y, Some(0), hwnd, None);
        let _ = PostMessageW(Some(hwnd), WM_NULL, WPARAM(0), LPARAM(0));
        let _ = DestroyMenu(menu);
        match cmd.0 {
            1 => post(Event::OpenPanel),
            2 => post(Event::ToggleIsland),
            3 => post(Event::Quit),
            _ => {}
        }
    }
}

unsafe extern "system" fn wndproc(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    if msg != 0 && msg == MSG_OPEN.load(Ordering::Relaxed) {
        post(Event::OpenPanel);
        return LRESULT(0);
    }
    if msg != 0 && msg == MSG_CMD.load(Ordering::Relaxed) {
        post(if wp.0 == 2 { Event::RestoreQuit } else { Event::Quit });
        return LRESULT(0);
    }
    if msg != 0 && msg == MSG_TASKBAR.load(Ordering::Relaxed) {
        TRAY_ON.store(false, Ordering::SeqCst);
        add_tray(); // Explorer restarted
        return LRESULT(0);
    }
    match msg {
        WM_TRAY => {
            let ev = (lp.0 & 0xFFFF) as u32;
            match ev {
                WM_LBUTTONUP | NIN_SELECT | 1025 => post(Event::OpenPanel),
                WM_CONTEXTMENU | WM_RBUTTONUP => show_tray_menu(hwnd),
                _ => {}
            }
            LRESULT(0)
        }
        WM_HOTKEY if wp.0 == 1 => {
            post(Event::OpenPanel);
            LRESULT(0)
        }
        WM_TIMER if wp.0 == crate::occlusion::TIMER_ID => {
            crate::occlusion::on_timer(hwnd);
            LRESULT(0)
        }
        WM_DISPLAYCHANGE => {
            post(Event::DisplayChanged);
            DefWindowProcW(hwnd, msg, wp, lp)
        }
        WM_WTSSESSION_CHANGE => {
            match wp.0 as u32 {
                7 => post(Event::Session { locked: true }),   // WTS_SESSION_LOCK
                8 => post(Event::Session { locked: false }),  // WTS_SESSION_UNLOCK
                _ => {}
            }
            LRESULT(0)
        }
        WM_POWERBROADCAST if wp.0 as u32 == PBT_POWERSETTINGCHANGE => {
            if let Some(ps) = (lp.0 as *const POWERBROADCAST_SETTING).as_ref() {
                if ps.PowerSetting == GUID_CONSOLE_DISPLAY_STATE {
                    post(Event::DisplayPower { on: ps.Data[0] != 0 });
                }
            }
            LRESULT(1)
        }
        _ => DefWindowProcW(hwnd, msg, wp, lp),
    }
}
