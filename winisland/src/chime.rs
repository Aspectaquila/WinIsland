//! The timer chime: three soft sine notes, synthesised once into an in-memory WAV and played asynchronously.

use std::sync::OnceLock;
use windows::core::PCWSTR;
use windows::Win32::Media::Audio::{PlaySoundW, SND_ASYNC, SND_MEMORY, SND_NODEFAULT};

fn wav() -> &'static [u8] {
    static W: OnceLock<Vec<u8>> = OnceLock::new();
    W.get_or_init(|| {
        let rate = 22050u32;
        let n = (rate as f32 * 1.75) as usize;
        let mut samples = vec![0f32; n];
        for (f, d) in [(880.0f32, 0.0f32), (1318.5, 0.18), (1760.0, 0.36)] {
            let start = (d * rate as f32) as usize;
            for i in 0..((1.3 * rate as f32) as usize) {
                let idx = start + i;
                if idx >= n {
                    break;
                }
                let t = i as f32 / rate as f32;
                let g = if t < 0.02 { 0.18 * t / 0.02 } else { 0.18 * (0.001f32 / 0.18).powf((t - 0.02) / 1.18) };
                samples[idx] += (t * f * std::f32::consts::TAU).sin() * g;
            }
        }
        let data: Vec<u8> = samples.iter().flat_map(|s| ((s.clamp(-1.0, 1.0) * 32767.0) as i16).to_le_bytes()).collect();
        let mut out = Vec::with_capacity(44 + data.len());
        out.extend_from_slice(b"RIFF");
        out.extend_from_slice(&(36 + data.len() as u32).to_le_bytes());
        out.extend_from_slice(b"WAVEfmt ");
        out.extend_from_slice(&16u32.to_le_bytes());
        out.extend_from_slice(&1u16.to_le_bytes());
        out.extend_from_slice(&1u16.to_le_bytes());
        out.extend_from_slice(&rate.to_le_bytes());
        out.extend_from_slice(&(rate * 2).to_le_bytes());
        out.extend_from_slice(&2u16.to_le_bytes());
        out.extend_from_slice(&16u16.to_le_bytes());
        out.extend_from_slice(b"data");
        out.extend_from_slice(&(data.len() as u32).to_le_bytes());
        out.extend_from_slice(&data);
        out
    })
}

pub fn play() {
    let w = wav();
    unsafe {
        let _ = PlaySoundW(PCWSTR(w.as_ptr() as *const u16), None, SND_MEMORY | SND_ASYNC | SND_NODEFAULT);
    }
}
