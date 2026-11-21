//! CPU / RAM sampling. Only runs while the dashboard is open (the UI drives it with a 2 s timer).

use crate::model::StatsInfo;
use windows::Win32::Foundation::FILETIME;
use windows::Win32::System::SystemInformation::{GlobalMemoryStatusEx, MEMORYSTATUSEX};
use windows::Win32::System::Threading::GetSystemTimes;

#[derive(Default)]
pub struct Sampler {
    idle: u64,
    total: u64,
}

fn ft(f: FILETIME) -> u64 {
    ((f.dwHighDateTime as u64) << 32) | f.dwLowDateTime as u64
}

impl Sampler {
    fn times() -> (u64, u64) {
        let (mut i, mut k, mut u) = (FILETIME::default(), FILETIME::default(), FILETIME::default());
        unsafe {
            let _ = GetSystemTimes(Some(&mut i), Some(&mut k), Some(&mut u));
        }
        (ft(i), ft(k) + ft(u))
    }

    pub fn prime(&mut self) {
        let (i, t) = Self::times();
        self.idle = i;
        self.total = t;
    }

    pub fn sample(&mut self) -> StatsInfo {
        let (idle, total) = Self::times();
        let cpu = if total > self.total { 100.0 * (1.0 - (idle - self.idle) as f64 / (total - self.total) as f64) } else { 0.0 };
        self.idle = idle;
        self.total = total;
        let mut m = MEMORYSTATUSEX { dwLength: std::mem::size_of::<MEMORYSTATUSEX>() as u32, ..Default::default() };
        unsafe {
            let _ = GlobalMemoryStatusEx(&mut m);
        }
        StatsInfo {
            cpu: (cpu.clamp(0.0, 100.0) * 10.0).round() / 10.0,
            ram: m.dwMemoryLoad as f64,
            ram_used: ((m.ullTotalPhys - m.ullAvailPhys) as f64 / 1073741824.0 * 10.0).round() / 10.0,
            ram_total: (m.ullTotalPhys as f64 / 1073741824.0 * 10.0).round() / 10.0,
        }
    }
}
