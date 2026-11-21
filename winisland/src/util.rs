//! Small helpers: local time, formatting, weather codes, colours.

use windows::Win32::System::SystemInformation::GetLocalTime;

pub struct LocalTime {
    pub year: u16,
    pub month: u16,
    pub day: u16,
    pub weekday: u16,
    pub hour: u16,
    pub minute: u16,
    pub second: u16,
}

pub fn local_time() -> LocalTime {
    let st = unsafe { GetLocalTime() };
    LocalTime {
        year: st.wYear,
        month: st.wMonth,
        day: st.wDay,
        weekday: st.wDayOfWeek,
        hour: st.wHour,
        minute: st.wMinute,
        second: st.wSecond,
    }
}

pub fn time_str() -> String {
    static FAKE: std::sync::OnceLock<Option<String>> = std::sync::OnceLock::new();
    if let Some(t) = FAKE.get_or_init(|| std::env::var("WINISLAND_FAKE_TIME").ok()) {
        return t.clone();
    }
    let t = local_time();
    format!("{:02}:{:02}", t.hour, t.minute)
}

pub fn date_str() -> String {
    let t = local_time();
    const DAYS: [&str; 7] = ["Sunday", "Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday"];
    const MONTHS: [&str; 12] = ["January", "February", "March", "April", "May", "June", "July", "August", "September", "October", "November", "December"];
    format!("{}, {} {}", DAYS[(t.weekday % 7) as usize], t.day, MONTHS[((t.month + 11) % 12) as usize])
}

/// Milliseconds until the next minute boundary (+30 ms).
pub fn ms_to_next_minute() -> u64 {
    let t = local_time();
    let st = unsafe { GetLocalTime() };
    let ms = t.second as u64 * 1000 + st.wMilliseconds as u64;
    60_000u64.saturating_sub(ms) + 30
}

pub struct Christmas {
    pub days: i64,
    pub is_eve: bool,
    pub is_day: bool,
}

fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let mp = (m + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146097 + doe - 719468
}

pub fn christmas() -> Christmas {
    let t = local_time();
    let (y, m, d) = (t.year as i64, t.month as i64, t.day as i64);
    let is_eve = m == 12 && d == 24;
    let is_day = m == 12 && (d == 25 || d == 26);
    let today = days_from_civil(y, m, d);
    let mut xmas = days_from_civil(y, 12, 24);
    if today > days_from_civil(y, 12, 27) || (m == 12 && d >= 27) {
        xmas = days_from_civil(y + 1, 12, 24);
    }
    // JS: floor((xmas - now) / day) with now including the time of day → one less than the calendar difference
    let days = (xmas - today - 1).max(0);
    Christmas { days: if is_eve { 0 } else { days }, is_eve, is_day }
}

pub fn greeting() -> (String, String) {
    let h = local_time().hour;
    let c = christmas();
    let t1 = if c.is_eve {
        "God Jul! Merry Christmas Eve".to_string()
    } else if c.is_day {
        "Merry Christmas".to_string()
    } else if h < 5 {
        "Good night".into()
    } else if h < 12 {
        "Good morning".into()
    } else if h < 18 {
        "Good afternoon".into()
    } else {
        "Good evening".into()
    };
    let t2 = if c.is_eve || c.is_day { "Warm wishes from the north".to_string() } else { format!("{} days until Christmas Eve", c.days) };
    (t1, t2)
}

/// WMO weather code → (label, sky kind)
pub fn wmo(code: i64) -> (&'static str, &'static str) {
    match code {
        0 => ("Clear", "clear"),
        1 => ("Mostly clear", "clear"),
        2 => ("Partly cloudy", "partly"),
        3 => ("Overcast", "cloud"),
        45 => ("Fog", "fog"),
        48 => ("Freezing fog", "fog"),
        51 => ("Light drizzle", "rain"),
        53 => ("Drizzle", "rain"),
        55 => ("Heavy drizzle", "rain"),
        56 | 57 => ("Freezing drizzle", "sleet"),
        61 => ("Light rain", "rain"),
        63 => ("Rain", "rain"),
        65 => ("Heavy rain", "rain"),
        66 | 67 => ("Freezing rain", "sleet"),
        71 => ("Light snow", "snow"),
        73 => ("Snow", "snow"),
        75 => ("Heavy snow", "snow"),
        77 => ("Snow grains", "snow"),
        80 | 81 => ("Showers", "rain"),
        82 => ("Heavy showers", "rain"),
        85 => ("Snow showers", "snow"),
        86 => ("Heavy snow showers", "snow"),
        95 | 96 | 99 => ("Thunderstorm", "storm"),
        _ => ("—", "cloud"),
    }
}

/// Icon name for a weather code.
pub fn weather_icon(code: i64, is_day: bool) -> &'static str {
    match wmo(code).1 {
        "clear" if !is_day => "clearNight",
        "clear" => "clear",
        "partly" => "partly",
        "fog" => "fog",
        "rain" => "rain",
        "sleet" => "sleet",
        "snow" => "snow",
        "storm" => "storm",
        _ => "cloud",
    }
}

pub fn clock(s: f64) -> String {
    if !s.is_finite() || s < 0.0 {
        return "0:00".into();
    }
    let s = s.floor() as i64;
    format!("{}:{:02}", s / 60, s % 60)
}

/// Dominant vivid colour of an RGBA image, forced to a lightness/saturation that reads well on black.
/// Returns (h, s, l) in 0..1, like the JS version (hsl(h 85% 68%), or light grey for grey covers).
pub fn tint_from_rgba(w: u32, h: u32, px: &[u8]) -> (f32, f32, f32) {
    let n = 14u32;
    let (mut best, mut best_score) = (None::<(f32, f32, f32)>, -1.0f32);
    let (mut sr, mut sg, mut sb, mut cnt) = (0f32, 0f32, 0f32, 0f32);
    for yy in 0..n {
        for xx in 0..n {
            let x = (xx * w / n).min(w.saturating_sub(1));
            let y = (yy * h / n).min(h.saturating_sub(1));
            let i = ((y * w + x) * 4) as usize;
            if i + 2 >= px.len() {
                continue;
            }
            let (r, g, b) = (px[i] as f32, px[i + 1] as f32, px[i + 2] as f32);
            let mx = r.max(g).max(b);
            let mn = r.min(g).min(b);
            let sat = if mx > 0.0 { (mx - mn) / mx } else { 0.0 };
            let score = sat * 0.7 + (mx / 255.0) * 0.3;
            sr += r;
            sg += g;
            sb += b;
            cnt += 1.0;
            if score > best_score && mx > 60.0 {
                best_score = score;
                best = Some((r, g, b));
            }
        }
    }
    let grey = best.is_none() || best_score < 0.25;
    if grey {
        return (0.0, 0.0, 0.82);
    }
    let _ = (sr, sg, sb, cnt);
    let (r, g, b) = best.unwrap();
    let (r, g, b) = (r / 255.0, g / 255.0, b / 255.0);
    let mx = r.max(g).max(b);
    let mn = r.min(g).min(b);
    let d = mx - mn;
    let mut hh = 0.0;
    if d > 0.0 {
        hh = if mx == r {
            ((g - b) / d).rem_euclid(6.0)
        } else if mx == g {
            (b - r) / d + 2.0
        } else {
            (r - g) / d + 4.0
        };
    }
    (((hh * 60.0 + 360.0) % 360.0) / 360.0, 0.85, 0.68)
}

pub fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

pub fn exe_path() -> String {
    std::env::current_exe().map(|p| p.display().to_string()).unwrap_or_default()
}

/// Working set and private bytes of this process, in MB.
pub fn process_memory_mb() -> (f64, f64) {
    use windows::Win32::System::ProcessStatus::{GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS, PROCESS_MEMORY_COUNTERS_EX};
    use windows::Win32::System::Threading::GetCurrentProcess;
    unsafe {
        let mut c = PROCESS_MEMORY_COUNTERS_EX { cb: std::mem::size_of::<PROCESS_MEMORY_COUNTERS_EX>() as u32, ..Default::default() };
        if GetProcessMemoryInfo(GetCurrentProcess(), &mut c as *mut _ as *mut PROCESS_MEMORY_COUNTERS, c.cb).is_ok() {
            return (c.WorkingSetSize as f64 / 1048576.0, c.PrivateUsage as f64 / 1048576.0);
        }
    }
    (0.0, 0.0)
}

pub fn process_running(names: &[&str]) -> Option<String> {
    use windows::Win32::System::Diagnostics::ToolHelp::*;
    unsafe {
        let snap = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0).ok()?;
        let mut e = PROCESSENTRY32W { dwSize: std::mem::size_of::<PROCESSENTRY32W>() as u32, ..Default::default() };
        let mut found = None;
        if Process32FirstW(snap, &mut e).is_ok() {
            loop {
                let len = e.szExeFile.iter().position(|&c| c == 0).unwrap_or(e.szExeFile.len());
                let name = String::from_utf16_lossy(&e.szExeFile[..len]).to_lowercase();
                let stem = name.trim_end_matches(".exe");
                if let Some(n) = names.iter().find(|n| n.eq_ignore_ascii_case(stem)) {
                    found = Some(n.to_string());
                    break;
                }
                if Process32NextW(snap, &mut e).is_err() {
                    break;
                }
            }
        }
        let _ = windows::Win32::Foundation::CloseHandle(snap);
        found
    }
}

/// `Pixels` → f32
pub trait PxF {
    fn f(self) -> f32;
}
impl PxF for gpui::Pixels {
    fn f(self) -> f32 {
        f32::from(self)
    }
}
