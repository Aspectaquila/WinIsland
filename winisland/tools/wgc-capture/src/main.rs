//! wgc-capture: website screenshots of WinIsland without the desktop behind it.
//!
//!   wgc-capture island <out.png> [--pid N] [--pad 24]
//!       Windows.Graphics.Capture of the island window (keeps per-pixel alpha), with the click-through glow /
//!       shadow window composited behind it, cropped to the visible pixels + padding.
//!   wgc-capture seq <out-dir> [--pid N] [--seconds 6] [--fps 30] [--pad 24]
//!       The same as a numbered PNG sequence, all frames cropped to one common box.
//!   wgc-capture panel <out.png> [--pid N] [--radius 8]
//!       The settings window via PrintWindow(PW_RENDERFULLCONTENT) (works while it is cloaked), cropped to the
//!       visible frame with rounded corners.

use std::time::{Duration, Instant};
use windows::core::{factory, Interface, Result, BOOL, PCWSTR};
use windows::Foundation::TypedEventHandler;
use windows::Graphics::Capture::{Direct3D11CaptureFramePool, GraphicsCaptureItem, GraphicsCaptureSession};
use windows::Graphics::DirectX::Direct3D11::IDirect3DDevice;
use windows::Graphics::DirectX::DirectXPixelFormat;
use windows::Win32::Foundation::{HMODULE, HWND, LPARAM, RECT};
use windows::Win32::Graphics::Direct3D::D3D_DRIVER_TYPE_HARDWARE;
use windows::Win32::Graphics::Direct3D11::*;
use windows::Win32::Graphics::Dwm::{DwmGetWindowAttribute, DWMWA_EXTENDED_FRAME_BOUNDS};
use windows::Win32::Graphics::Dxgi::IDXGIDevice;
use windows::Win32::Graphics::Gdi::*;
use windows::Win32::System::WinRT::Direct3D11::{CreateDirect3D11DeviceFromDXGIDevice, IDirect3DDxgiInterfaceAccess};
use windows::Win32::System::WinRT::Graphics::Capture::IGraphicsCaptureItemInterop;
use windows::Win32::UI::WindowsAndMessaging::*;
use windows::Win32::Storage::Xps::{PrintWindow, PRINT_WINDOW_FLAGS};

/// Premultiplied BGRA image.
#[derive(Clone)]
struct Img {
    w: u32,
    h: u32,
    px: Vec<u8>,
}

fn arg(args: &[String], name: &str) -> Option<String> {
    args.iter().position(|a| a == name).and_then(|i| args.get(i + 1)).cloned()
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 3 {
        eprintln!("usage: wgc-capture island|seq|panel <out> [--pid N] ...");
        std::process::exit(2);
    }
    unsafe {
        let _ = windows::Win32::UI::HiDpi::SetProcessDpiAwarenessContext(windows::Win32::UI::HiDpi::DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
    }
    let pid = arg(&args, "--pid").and_then(|s| s.parse().ok()).unwrap_or_else(find_pid);
    let pad: u32 = arg(&args, "--pad").and_then(|s| s.parse().ok()).unwrap_or(24);
    let r = match args[1].as_str() {
        "island" => island(pid, &args[2], pad),
        "seq" => seq(
            pid,
            &args[2],
            arg(&args, "--seconds").and_then(|s| s.parse().ok()).unwrap_or(6.0),
            arg(&args, "--fps").and_then(|s| s.parse().ok()).unwrap_or(30.0),
            pad,
        ),
        "panel" => panel(pid, &args[2], arg(&args, "--radius").and_then(|s| s.parse().ok()).unwrap_or(8)),
        _ => {
            eprintln!("unknown mode");
            std::process::exit(2);
        }
    };
    if let Err(e) = r {
        eprintln!("error: {e:?}");
        std::process::exit(1);
    }
}

// ------------------------------------------------------------------ window lookup

fn find_pid() -> u32 {
    // the newest winisland.exe from the Rust build
    let out = std::process::Command::new("powershell")
        .args(["-NoProfile", "-Command", "(Get-Process winisland | Where-Object { $_.Path -like '*\\winisland\\target\\*' } | Sort-Object StartTime -Descending | Select-Object -First 1).Id"])
        .output()
        .expect("powershell");
    String::from_utf8_lossy(&out.stdout).trim().parse().expect("no running winisland.exe (Rust build) found")
}

struct Found {
    pid: u32,
    island: Option<HWND>,
    glow: Option<HWND>,
    panel: Option<HWND>,
}

unsafe extern "system" fn enum_cb(h: HWND, lp: LPARAM) -> BOOL {
    let f = &mut *(lp.0 as *mut Found);
    let mut pid = 0u32;
    GetWindowThreadProcessId(h, Some(&mut pid));
    if pid != f.pid || !IsWindowVisible(h).as_bool() {
        return true.into();
    }
    let mut cls = [0u16; 64];
    let n = GetClassNameW(h, &mut cls);
    let cls = String::from_utf16_lossy(&cls[..n as usize]);
    let mut title = [0u16; 128];
    let n = GetWindowTextW(h, &mut title);
    let title = String::from_utf16_lossy(&title[..n as usize]);
    if cls != "Zed::Window" {
        return true.into();
    }
    let ex = GetWindowLongPtrW(h, GWL_EXSTYLE) as u32;
    if title.contains("Settings") {
        f.panel = Some(h);
    } else if ex & WS_EX_TRANSPARENT.0 != 0 {
        f.glow = Some(h);
    } else if ex & WS_EX_TOOLWINDOW.0 != 0 {
        f.island = Some(h);
    }
    true.into()
}

fn find(pid: u32) -> Found {
    let mut f = Found { pid, island: None, glow: None, panel: None };
    unsafe {
        let _ = EnumWindows(Some(enum_cb), LPARAM(&mut f as *mut _ as isize));
    }
    f
}

// ------------------------------------------------------------------ Windows.Graphics.Capture

struct Capturer {
    device: ID3D11Device,
    context: ID3D11DeviceContext,
    pool: Direct3D11CaptureFramePool,
    _session: GraphicsCaptureSession,
}

fn d3d() -> Result<(ID3D11Device, ID3D11DeviceContext, IDirect3DDevice)> {
    unsafe {
        let mut device = None;
        let mut context = None;
        D3D11CreateDevice(None, D3D_DRIVER_TYPE_HARDWARE, HMODULE::default(), D3D11_CREATE_DEVICE_BGRA_SUPPORT, None, D3D11_SDK_VERSION, Some(&mut device), None, Some(&mut context))?;
        let device: ID3D11Device = device.unwrap();
        let dxgi: IDXGIDevice = device.cast()?;
        let inspectable = CreateDirect3D11DeviceFromDXGIDevice(&dxgi)?;
        Ok((device, context.unwrap(), inspectable.cast()?))
    }
}

impl Capturer {
    fn new(hwnd: HWND) -> Result<Self> {
        let (device, context, rt_device) = d3d()?;
        let interop = factory::<GraphicsCaptureItem, IGraphicsCaptureItemInterop>()?;
        let item: GraphicsCaptureItem = unsafe { interop.CreateForWindow(hwnd)? };
        let size = item.Size()?;
        let pool = Direct3D11CaptureFramePool::CreateFreeThreaded(&rt_device, DirectXPixelFormat::B8G8R8A8UIntNormalized, 2, size)?;
        let session = pool.CreateCaptureSession(&item)?;
        let _ = session.SetIsCursorCaptureEnabled(false);
        let _ = session.SetIsBorderRequired(false);
        let _ = pool.FrameArrived(&TypedEventHandler::new(|_, _| Ok(())));
        session.StartCapture()?;
        Ok(Capturer { device, context, pool, _session: session })
    }

    /// Latest frame available within `wait`.
    fn grab(&self, wait: Duration) -> Result<Option<Img>> {
        let t0 = Instant::now();
        let mut latest = None;
        loop {
            while let Ok(f) = self.pool.TryGetNextFrame() {
                latest = Some(f);
            }
            if latest.is_some() || t0.elapsed() > wait {
                break;
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        let Some(frame) = latest else { return Ok(None) };
        let surface = frame.Surface()?;
        let access: IDirect3DDxgiInterfaceAccess = surface.cast()?;
        let tex: ID3D11Texture2D = unsafe { access.GetInterface()? };
        Ok(Some(self.read(&tex)?))
    }

    fn read(&self, tex: &ID3D11Texture2D) -> Result<Img> {
        unsafe {
            let mut desc = D3D11_TEXTURE2D_DESC::default();
            tex.GetDesc(&mut desc);
            desc.Usage = D3D11_USAGE_STAGING;
            desc.BindFlags = 0;
            desc.CPUAccessFlags = D3D11_CPU_ACCESS_READ.0 as u32;
            desc.MiscFlags = 0;
            let mut staging = None;
            self.device.CreateTexture2D(&desc, None, Some(&mut staging))?;
            let staging = staging.unwrap();
            self.context.CopyResource(&staging, tex);
            let mut m = D3D11_MAPPED_SUBRESOURCE::default();
            self.context.Map(&staging, 0, D3D11_MAP_READ, 0, Some(&mut m))?;
            let (w, h) = (desc.Width, desc.Height);
            let mut px = vec![0u8; (w * h * 4) as usize];
            for y in 0..h {
                let src = std::slice::from_raw_parts((m.pData as *const u8).add((y * m.RowPitch) as usize), (w * 4) as usize);
                px[(y * w * 4) as usize..((y + 1) * w * 4) as usize].copy_from_slice(src);
            }
            self.context.Unmap(&staging, 0);
            Ok(Img { w, h, px })
        }
    }
}

// ------------------------------------------------------------------ image helpers

/// `top` over `bottom`, both premultiplied, aligned at the top-left.
fn over(top: &Img, bottom: Option<&Img>) -> Img {
    let Some(b) = bottom else { return top.clone() };
    let mut out = top.clone();
    for y in 0..top.h.min(b.h) {
        for x in 0..top.w.min(b.w) {
            let i = ((y * top.w + x) * 4) as usize;
            let j = ((y * b.w + x) * 4) as usize;
            let a = top.px[i + 3] as u32;
            for c in 0..4 {
                let v = top.px[i + c] as u32 + (b.px[j + c] as u32 * (255 - a) + 127) / 255;
                out.px[i + c] = v.min(255) as u8;
            }
        }
    }
    out
}

fn bbox(img: &Img) -> Option<(u32, u32, u32, u32)> {
    let (mut x0, mut y0, mut x1, mut y1) = (u32::MAX, u32::MAX, 0, 0);
    for y in 0..img.h {
        for x in 0..img.w {
            if img.px[((y * img.w + x) * 4 + 3) as usize] > 2 {
                x0 = x0.min(x);
                y0 = y0.min(y);
                x1 = x1.max(x);
                y1 = y1.max(y);
            }
        }
    }
    (x0 != u32::MAX).then_some((x0, y0, x1 + 1, y1 + 1))
}

/// Crop to the box (+pad, transparent outside the source) and convert to straight-alpha RGBA.
fn to_rgba(img: &Img, b: (u32, u32, u32, u32), pad: u32) -> image::RgbaImage {
    let (x0, y0, x1, y1) = b;
    let (w, h) = (x1 - x0 + 2 * pad, y1 - y0 + 2 * pad);
    let mut out = image::RgbaImage::new(w, h);
    for y in 0..h {
        for x in 0..w {
            let sx = x as i64 + x0 as i64 - pad as i64;
            let sy = y as i64 + y0 as i64 - pad as i64;
            if sx < 0 || sy < 0 || sx >= img.w as i64 || sy >= img.h as i64 {
                continue;
            }
            let i = ((sy as u32 * img.w + sx as u32) * 4) as usize;
            let a = img.px[i + 3];
            if a == 0 {
                continue;
            }
            let un = |c: u8| ((c as u32 * 255 + a as u32 / 2) / a as u32).min(255) as u8;
            out.put_pixel(x, y, image::Rgba([un(img.px[i + 2]), un(img.px[i + 1]), un(img.px[i]), a]));
        }
    }
    out
}

/// Island (+ glow behind it) as one premultiplied frame.
fn capture_island(island: &Capturer, glow: Option<&Capturer>) -> Result<Img> {
    let top = island.grab(Duration::from_secs(3))?.expect("no frame from the island window");
    let bottom = match glow {
        Some(g) => g.grab(Duration::from_millis(600))?,
        None => None,
    };
    Ok(over(&top, bottom.as_ref()))
}

fn island(pid: u32, out: &str, pad: u32) -> Result<()> {
    let f = find(pid);
    let island = Capturer::new(f.island.expect("island window not found"))?;
    let glow = f.glow.and_then(|g| Capturer::new(g).ok());
    std::thread::sleep(Duration::from_millis(250)); // let the pool fill with a current frame
    let img = capture_island(&island, glow.as_ref())?;
    let b = bbox(&img).expect("island frame is fully transparent");
    to_rgba(&img, b, pad).save(out).expect("save png");
    println!("{out}: {}x{} (from {}x{} window frame)", b.2 - b.0 + 2 * pad, b.3 - b.1 + 2 * pad, img.w, img.h);
    Ok(())
}

fn seq(pid: u32, dir: &str, seconds: f32, fps: f32, pad: u32) -> Result<()> {
    std::fs::create_dir_all(dir).ok();
    let f = find(pid);
    let island = Capturer::new(f.island.expect("island window not found"))?;
    let glow = f.glow.and_then(|g| Capturer::new(g).ok());
    let mut frames = vec![];
    let mut last_glow: Option<Img> = None;
    let t0 = Instant::now();
    let step = Duration::from_secs_f32(1.0 / fps);
    let n = (seconds * fps).round() as u32;
    for k in 0..n {
        let due = step * k;
        if let Some(d) = due.checked_sub(t0.elapsed()) {
            std::thread::sleep(d);
        }
        // the window only produces frames when it changes: reuse the previous one in between
        let top = island.grab(Duration::from_millis(1))?;
        if let Some(g) = glow.as_ref() {
            if let Some(gi) = g.grab(Duration::from_millis(0))? {
                last_glow = Some(gi);
            }
        }
        match top {
            Some(t) => frames.push(over(&t, last_glow.as_ref())),
            None => {
                if let Some(prev) = frames.last().cloned() {
                    frames.push(prev)
                }
            }
        }
    }
    let mut ub: Option<(u32, u32, u32, u32)> = None;
    for fr in &frames {
        if let Some(b) = bbox(fr) {
            ub = Some(match ub {
                None => b,
                Some(u) => (u.0.min(b.0), u.1.min(b.1), u.2.max(b.2), u.3.max(b.3)),
            });
        }
    }
    let ub = ub.expect("all frames transparent");
    let imgs: Vec<image::RgbaImage> = frames.iter().map(|fr| to_rgba(fr, ub, pad)).collect();
    drop(frames);
    for (i, im) in imgs.iter().enumerate() {
        im.save(format!("{dir}\\frame_{i:04}.png")).expect("save png");
    }
    // plus one animated PNG (APNG: keeps alpha, plays in every current browser)
    let (w, h) = imgs[0].dimensions();
    let file = std::fs::File::create(format!("{dir}\\sequence.png")).expect("create apng");
    let mut enc = png::Encoder::new(std::io::BufWriter::new(file), w, h);
    enc.set_color(png::ColorType::Rgba);
    enc.set_depth(png::BitDepth::Eight);
    enc.set_animated(imgs.len() as u32, 0).expect("apng");
    enc.set_frame_delay(1, fps.round() as u16).expect("apng");
    enc.set_blend_op(png::BlendOp::Source).expect("apng");
    let mut wr = enc.write_header().expect("apng header");
    for im in &imgs {
        wr.write_image_data(im.as_raw()).expect("apng frame");
    }
    wr.finish().expect("apng finish");
    let frames = imgs;
    println!("{dir}: {} frames, {}x{}", frames.len(), ub.2 - ub.0 + 2 * pad, ub.3 - ub.1 + 2 * pad);
    Ok(())
}

// ------------------------------------------------------------------ settings window

fn panel(pid: u32, out: &str, radius: u32) -> Result<()> {
    let f = find(pid);
    let h = f.panel.expect("settings window not found");
    unsafe {
        let mut wr = RECT::default();
        GetWindowRect(h, &mut wr)?;
        let mut fr = RECT::default();
        let _ = DwmGetWindowAttribute(h, DWMWA_EXTENDED_FRAME_BOUNDS, &mut fr as *mut _ as _, std::mem::size_of::<RECT>() as u32);
        let (w, hh) = (wr.right - wr.left, wr.bottom - wr.top);
        let screen = GetDC(None);
        let dc = CreateCompatibleDC(Some(screen));
        let bmp = CreateCompatibleBitmap(screen, w, hh);
        let old = SelectObject(dc, bmp.into());
        let _ = PrintWindow(h, dc, PRINT_WINDOW_FLAGS(2)); // PW_RENDERFULLCONTENT
        let mut bi = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER { biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32, biWidth: w, biHeight: -hh, biPlanes: 1, biBitCount: 32, ..Default::default() },
            ..Default::default()
        };
        let mut px = vec![0u8; (w * hh * 4) as usize];
        GetDIBits(dc, bmp, 0, hh as u32, Some(px.as_mut_ptr() as _), &mut bi, DIB_RGB_COLORS);
        SelectObject(dc, old);
        let _ = DeleteObject(bmp.into());
        let _ = DeleteDC(dc);
        ReleaseDC(None, screen);
        // visible frame (without the invisible resize borders)
        let (x0, y0) = ((fr.left - wr.left).max(0) as u32, (fr.top - wr.top).max(0) as u32);
        let (cw, ch) = ((fr.right - fr.left).min(w) as u32, (fr.bottom - fr.top).min(hh) as u32);
        let mut img = image::RgbaImage::new(cw, ch);
        let r = radius as f32;
        for y in 0..ch {
            for x in 0..cw {
                let i = (((y + y0) * w as u32 + x + x0) * 4) as usize;
                // rounded window corners (anti-aliased)
                let (fx, fy) = (x as f32 + 0.5, y as f32 + 0.5);
                let cx = if fx < r { r } else if fx > cw as f32 - r { cw as f32 - r } else { fx };
                let cy = if fy < r { r } else if fy > ch as f32 - r { ch as f32 - r } else { fy };
                let d = ((fx - cx).powi(2) + (fy - cy).powi(2)).sqrt();
                let a = (r + 0.5 - d).clamp(0.0, 1.0);
                img.put_pixel(x, y, image::Rgba([px[i + 2], px[i + 1], px[i], (a * 255.0) as u8]));
            }
        }
        img.save(out).expect("save png");
        println!("{out}: {cw}x{ch}");
        let _ = PCWSTR::null();
    }
    Ok(())
}
