//! System volume via Core Audio with change *notifications* (no polling), following default-device changes,
//! plus a peak meter for the visualizer that samples at 15 Hz only while someone needs it.

use crate::model::{post, Event};
use std::sync::mpsc::{channel, Receiver, RecvTimeoutError, Sender};
use std::sync::OnceLock;
use std::time::Duration;
use windows::core::{implement, GUID, PCWSTR};
use windows::Win32::Foundation::PROPERTYKEY;
use windows::Win32::Media::Audio::Endpoints::*;
use windows::Win32::Media::Audio::*;
use windows::Win32::System::Com::*;

pub enum Cmd {
    SetLevel(f32),
    ToggleMute,
    Metering(bool),
    Rebind,
}

static TX: OnceLock<Sender<Cmd>> = OnceLock::new();
/// Identifies volume changes we make ourselves (so the island doesn't show the HUD twice).
const OUR_CTX: GUID = GUID::from_u128(0x5f1e2c3a_9b7d_4e21_a0c4_57696e49736c);

pub fn send(c: Cmd) {
    if let Some(tx) = TX.get() {
        let _ = tx.send(c);
    }
}

pub fn start() {
    let (tx, rx) = channel();
    let _ = TX.set(tx.clone());
    std::thread::Builder::new()
        .name("audio".into())
        .spawn(move || unsafe {
            let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
            run(tx, rx);
        })
        .ok();
}

#[implement(IAudioEndpointVolumeCallback)]
struct VolumeCallback;

impl IAudioEndpointVolumeCallback_Impl for VolumeCallback_Impl {
    fn OnNotify(&self, p: *mut AUDIO_VOLUME_NOTIFICATION_DATA) -> windows::core::Result<()> {
        if let Some(d) = unsafe { p.as_ref() } {
            post(Event::Volume { level: d.fMasterVolume, muted: d.bMuted.as_bool(), ours: d.guidEventContext == OUR_CTX });
        }
        Ok(())
    }
}

#[implement(IMMNotificationClient)]
struct DeviceCallback {
    tx: Sender<Cmd>,
}

impl IMMNotificationClient_Impl for DeviceCallback_Impl {
    fn OnDeviceStateChanged(&self, _: &PCWSTR, _: DEVICE_STATE) -> windows::core::Result<()> {
        Ok(())
    }
    fn OnDeviceAdded(&self, _: &PCWSTR) -> windows::core::Result<()> {
        Ok(())
    }
    fn OnDeviceRemoved(&self, _: &PCWSTR) -> windows::core::Result<()> {
        Ok(())
    }
    fn OnDefaultDeviceChanged(&self, flow: EDataFlow, role: ERole, _: &PCWSTR) -> windows::core::Result<()> {
        if flow == eRender && role == eMultimedia {
            let _ = self.tx.send(Cmd::Rebind);
        }
        Ok(())
    }
    fn OnPropertyValueChanged(&self, _: &PCWSTR, _: &PROPERTYKEY) -> windows::core::Result<()> {
        Ok(())
    }
}

struct Bound {
    vol: IAudioEndpointVolume,
    meter: Option<IAudioMeterInformation>,
}

unsafe fn bind(en: &IMMDeviceEnumerator, cb: &IAudioEndpointVolumeCallback, old: Option<Bound>) -> Option<Bound> {
    if let Some(b) = old {
        let _ = b.vol.UnregisterControlChangeNotify(cb);
    }
    let dev = en.GetDefaultAudioEndpoint(eRender, eMultimedia).ok()?;
    let vol: IAudioEndpointVolume = dev.Activate(CLSCTX_ALL, None).ok()?;
    let _ = vol.RegisterControlChangeNotify(cb);
    let meter: Option<IAudioMeterInformation> = dev.Activate(CLSCTX_ALL, None).ok();
    let level = vol.GetMasterVolumeLevelScalar().unwrap_or(0.0);
    let muted = vol.GetMute().map(|b| b.as_bool()).unwrap_or(false);
    post(Event::Volume { level, muted, ours: true }); // initial state (ours = no HUD)
    Some(Bound { vol, meter })
}

unsafe fn run(tx: Sender<Cmd>, rx: Receiver<Cmd>) {
    let Ok(en) = CoCreateInstance::<_, IMMDeviceEnumerator>(&MMDeviceEnumerator, None, CLSCTX_ALL) else { return };
    let dev_cb: IMMNotificationClient = DeviceCallback { tx }.into();
    let _ = en.RegisterEndpointNotificationCallback(&dev_cb);
    let vol_cb: IAudioEndpointVolumeCallback = VolumeCallback.into();
    let mut b = bind(&en, &vol_cb, None);
    let mut metering = false;
    let mut muted_cache = false;
    loop {
        let cmd = if metering {
            match rx.recv_timeout(Duration::from_millis(66)) {
                Ok(c) => Some(c),
                Err(RecvTimeoutError::Timeout) => None,
                Err(_) => break,
            }
        } else {
            match rx.recv() {
                Ok(c) => Some(c),
                Err(_) => break,
            }
        };
        match cmd {
            None => {
                if let Some(m) = b.as_ref().and_then(|b| b.meter.as_ref()) {
                    let mut peaks = [0f32; 2];
                    let n = m.GetMeteringChannelCount().unwrap_or(0);
                    let v = if n >= 2 && m.GetChannelsPeakValues(&mut peaks).is_ok() {
                        peaks[0].max(peaks[1])
                    } else {
                        m.GetPeakValue().unwrap_or(0.0)
                    };
                    post(Event::Levels(v));
                }
            }
            Some(Cmd::Metering(on)) => {
                if metering && !on {
                    post(Event::Levels(0.0));
                }
                metering = on;
            }
            Some(Cmd::SetLevel(v)) => {
                if let Some(b) = &b {
                    let _ = b.vol.SetMasterVolumeLevelScalar(v.clamp(0.0, 1.0), &OUR_CTX);
                    muted_cache = b.vol.GetMute().map(|m| m.as_bool()).unwrap_or(false);
                    if muted_cache && v > 0.0 {
                        let _ = b.vol.SetMute(false, &OUR_CTX);
                    }
                }
            }
            Some(Cmd::ToggleMute) => {
                if let Some(b) = &b {
                    let m = b.vol.GetMute().map(|m| m.as_bool()).unwrap_or(false);
                    let _ = b.vol.SetMute(!m, &OUR_CTX);
                }
            }
            Some(Cmd::Rebind) => {
                b = bind(&en, &vol_cb, b.take());
            }
        }
    }
    let _ = muted_cache;
}
