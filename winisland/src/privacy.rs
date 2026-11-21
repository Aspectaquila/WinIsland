//! Camera / microphone in use: Windows records this under CapabilityAccessManager\ConsentStore
//! (LastUsedTimeStop == 0 while in use). We block on registry change notifications, so this costs nothing while
//! nothing changes, and confirm candidates against running processes (Windows leaves stale entries after crashes).

use crate::model::{post, Event, PrivacyInfo};
use crate::util::wide;
use std::collections::HashSet;
use windows::core::PCWSTR;
use windows::Win32::Foundation::{CloseHandle, HANDLE, WAIT_OBJECT_0};
use windows::Win32::System::Registry::*;
use windows::Win32::System::Threading::*;

const BASE: &str = r"Software\Microsoft\Windows\CurrentVersion\CapabilityAccessManager\ConsentStore\";

pub fn start() {
    std::thread::Builder::new()
        .name("privacy".into())
        .spawn(|| unsafe { run() })
        .ok();
}

unsafe fn open(path: &str) -> Option<HKEY> {
    let mut k = HKEY::default();
    let p = wide(path);
    if RegOpenKeyExW(HKEY_CURRENT_USER, PCWSTR(p.as_ptr()), Some(0), KEY_READ | KEY_NOTIFY, &mut k).is_ok() {
        Some(k)
    } else {
        None
    }
}

unsafe fn run() {
    let caps = ["webcam", "microphone"];
    let mut keys = vec![];
    let mut events = vec![];
    for c in caps {
        if let Some(k) = open(&format!("{BASE}{c}")) {
            if let Ok(ev) = CreateEventW(None, false, false, None) {
                arm(k, ev);
                keys.push(k);
                events.push(ev);
            }
        }
    }
    let mut last = evaluate();
    post(Event::Privacy(last.clone()));
    if events.is_empty() {
        return;
    }
    loop {
        let r = WaitForMultipleObjects(&events, false, INFINITE);
        let i = r.0.wrapping_sub(WAIT_OBJECT_0.0) as usize;
        if i >= events.len() {
            break;
        }
        arm(keys[i], events[i]);
        // apps write several values in a row; let them settle
        std::thread::sleep(std::time::Duration::from_millis(120));
        let now = evaluate();
        if now != last {
            last = now.clone();
            post(Event::Privacy(now));
        }
    }
    for e in events {
        let _ = CloseHandle(e);
    }
}

unsafe fn arm(k: HKEY, ev: HANDLE) {
    let _ = RegNotifyChangeKeyValue(k, true, REG_NOTIFY_CHANGE_NAME | REG_NOTIFY_CHANGE_LAST_SET, Some(ev), true);
}

fn evaluate() -> PrivacyInfo {
    let (camera, camera_app) = in_use("webcam");
    let (mic, mic_app) = in_use("microphone");
    PrivacyInfo { camera, mic, camera_app, mic_app }
}

unsafe fn subkeys(k: HKEY) -> Vec<String> {
    let mut out = vec![];
    let mut i = 0u32;
    loop {
        let mut buf = [0u16; 512];
        let mut len = buf.len() as u32;
        if RegEnumKeyExW(k, i, Some(windows::core::PWSTR(buf.as_mut_ptr())), &mut len, None, None, None, None).is_err() {
            break;
        }
        out.push(String::from_utf16_lossy(&buf[..len as usize]));
        i += 1;
    }
    out
}

unsafe fn qword(k: HKEY, name: &str) -> Option<u64> {
    let n = wide(name);
    let mut v = 0u64;
    let mut size = 8u32;
    let mut ty = REG_VALUE_TYPE::default();
    if RegQueryValueExW(k, PCWSTR(n.as_ptr()), None, Some(&mut ty), Some(&mut v as *mut u64 as *mut u8), Some(&mut size)).is_ok() && ty == REG_QWORD {
        Some(v)
    } else {
        None
    }
}

unsafe fn active(k: HKEY) -> bool {
    qword(k, "LastUsedTimeStop") == Some(0) && qword(k, "LastUsedTimeStart").map(|s| s > 0).unwrap_or(false)
}

unsafe fn open_sub(parent: HKEY, name: &str) -> Option<HKEY> {
    let mut k = HKEY::default();
    let p = wide(name);
    if RegOpenKeyExW(parent, PCWSTR(p.as_ptr()), Some(0), KEY_READ, &mut k).is_ok() {
        Some(k)
    } else {
        None
    }
}

fn in_use(cap: &str) -> (bool, String) {
    unsafe {
        let Some(root) = open(&format!("{BASE}{cap}")) else { return (false, String::new()) };
        let mut running: Option<HashSet<String>> = None;
        let mut result = (false, String::new());
        'outer: for name in subkeys(root) {
            let Some(k) = open_sub(root, &name) else { continue };
            if name == "NonPackaged" {
                for sub in subkeys(k) {
                    let Some(s) = open_sub(k, &sub) else { continue };
                    let on = active(s);
                    let _ = RegCloseKey(s);
                    if !on {
                        continue;
                    }
                    let set = running.get_or_insert_with(running_images);
                    if set.contains(&sub.replace('#', "\\").to_lowercase()) {
                        result = (true, friendly(&sub));
                        let _ = RegCloseKey(k);
                        break 'outer;
                    }
                }
            } else if active(k) {
                let set = running.get_or_insert_with(running_images);
                let pkg = name.split('_').next().unwrap_or("").to_lowercase();
                let needle = format!("\\windowsapps\\{pkg}_");
                if set.iter().any(|p| p.contains(&needle)) {
                    result = (true, friendly(&name));
                    let _ = RegCloseKey(k);
                    break 'outer;
                }
            }
            let _ = RegCloseKey(k);
        }
        let _ = RegCloseKey(root);
        result
    }
}

fn running_images() -> HashSet<String> {
    use windows::Win32::System::Diagnostics::ToolHelp::*;
    let mut set = HashSet::new();
    unsafe {
        let Ok(snap) = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) else { return set };
        let mut e = PROCESSENTRY32W { dwSize: std::mem::size_of::<PROCESSENTRY32W>() as u32, ..Default::default() };
        if Process32FirstW(snap, &mut e).is_ok() {
            loop {
                if let Ok(h) = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, e.th32ProcessID) {
                    let mut buf = [0u16; 1024];
                    let mut n = buf.len() as u32;
                    if QueryFullProcessImageNameW(h, PROCESS_NAME_WIN32, windows::core::PWSTR(buf.as_mut_ptr()), &mut n).is_ok() {
                        set.insert(String::from_utf16_lossy(&buf[..n as usize]).to_lowercase());
                    }
                    let _ = CloseHandle(h);
                }
                if Process32NextW(snap, &mut e).is_err() {
                    break;
                }
            }
        }
        let _ = CloseHandle(snap);
    }
    set
}

fn friendly(id: &str) -> String {
    let mut n = id.split(['#', '_']).filter(|p| !p.is_empty()).last().unwrap_or("").to_string();
    if n.to_lowercase().ends_with(".exe") {
        n.truncate(n.len() - 4);
    }
    n.chars().take(24).collect()
}
