//! Weather from Open-Meteo (free, no API key). Cached on disk; refreshed at most every 30 minutes.

use crate::model::{now_ms, post, Event, WeatherInfo};
use crate::net::{enc, get_json};
use serde_json::{json, Value};
use std::sync::mpsc::{channel, RecvTimeoutError, Sender};
use std::sync::OnceLock;
use std::time::Duration;

#[derive(Clone, Default)]
pub struct Params {
    pub lat: Option<f64>,
    pub lon: Option<f64>,
    pub units_f: bool,
    pub city: String,
}

pub enum Cmd {
    Refresh { force: bool, p: Params },
}

static TX: OnceLock<Sender<Cmd>> = OnceLock::new();

pub fn send(c: Cmd) {
    if let Some(tx) = TX.get() {
        let _ = tx.send(c);
    }
}

fn cache_path() -> std::path::PathBuf {
    crate::settings::cache_dir().join("weather.json")
}

pub fn parse(v: &Value) -> WeatherInfo {
    if v["needsCity"].as_bool() == Some(true) {
        return WeatherInfo { needs_city: true, ..Default::default() };
    }
    WeatherInfo {
        needs_city: false,
        city: v["city"].as_str().unwrap_or("").into(),
        units: v["units"].as_str().unwrap_or("C").into(),
        temp: v["temp"].as_f64(),
        feels: v["feels"].as_f64().unwrap_or(0.0),
        code: v["code"].as_i64().unwrap_or(3),
        wind: v["wind"].as_f64().unwrap_or(0.0),
        humidity: v["humidity"].as_i64().unwrap_or(0),
        is_day: v["isDay"].as_bool().unwrap_or(true),
        hi: v["hi"].as_f64(),
        lo: v["lo"].as_f64(),
    }
}

pub fn start(initial: Params) {
    let (tx, rx) = channel();
    let _ = TX.set(tx);
    std::thread::Builder::new()
        .name("weather".into())
        .spawn(move || {
            let mut current: Option<Value> = std::fs::read_to_string(cache_path()).ok().and_then(|t| serde_json::from_str(&t).ok());
            let mut p = initial;
            refresh(&mut current, &p, false);
            loop {
                match rx.recv_timeout(Duration::from_secs(30 * 60)) {
                    Ok(Cmd::Refresh { force, p: np }) => {
                        p = np;
                        refresh(&mut current, &p, force);
                    }
                    Err(RecvTimeoutError::Timeout) => refresh(&mut current, &p, false),
                    Err(_) => break,
                }
            }
        })
        .ok();
}

fn refresh(current: &mut Option<Value>, p: &Params, force: bool) {
    let (Some(lat), Some(lon)) = (p.lat, p.lon) else {
        post(Event::Weather(WeatherInfo { needs_city: true, ..Default::default() }));
        return;
    };
    if !force {
        if let Some(c) = current.as_ref() {
            let fresh = c["fetched"].as_i64().map(|f| now_ms() / 1000 - f < 29 * 60).unwrap_or(false);
            let same = (c["lat"].as_f64().unwrap_or(0.0) - lat).abs() < 0.01;
            if fresh && same {
                post(Event::Weather(parse(c)));
                return;
            }
        }
    }
    match fetch(lat, lon, p) {
        Some(msg) => {
            let _ = std::fs::create_dir_all(crate::settings::cache_dir());
            let _ = std::fs::write(cache_path(), msg.to_string());
            post(Event::Weather(parse(&msg)));
            *current = Some(msg);
        }
        None => {
            if let Some(c) = current.as_ref() {
                post(Event::Weather(parse(c))); // offline: keep showing the cached reading
            }
        }
    }
}

fn fetch(lat: f64, lon: f64, p: &Params) -> Option<Value> {
    let url = format!(
        "https://api.open-meteo.com/v1/forecast?latitude={:.3}&longitude={:.3}&current=temperature_2m,apparent_temperature,weather_code,wind_speed_10m,relative_humidity_2m,is_day&hourly=temperature_2m,weather_code&daily=temperature_2m_max,temperature_2m_min,sunrise,sunset,weather_code&forecast_days=4&timezone=auto{}",
        lat,
        lon,
        if p.units_f { "&temperature_unit=fahrenheit&wind_speed_unit=mph" } else { "" }
    );
    let raw = get_json(&url).ok()??;
    let cur = &raw["current"];
    let now_local = cur["time"].as_str().unwrap_or("");
    let now_key = parse_local(now_local);
    let mut hours = vec![];
    if let (Some(ht), Some(hv), Some(hc)) = (raw["hourly"]["time"].as_array(), raw["hourly"]["temperature_2m"].as_array(), raw["hourly"]["weather_code"].as_array()) {
        for i in 0..ht.len() {
            if hours.len() >= 6 {
                break;
            }
            let t = parse_local(ht[i].as_str().unwrap_or(""));
            if t < now_key - 30 || (t / 60 % 24) % 2 != 0 {
                continue;
            }
            hours.push(json!({ "h": t / 60 % 24, "t": hv[i].as_f64().unwrap_or(0.0).round(), "c": hc[i].as_i64().unwrap_or(0) }));
        }
    }
    let d = &raw["daily"];
    let mut days = vec![];
    if let Some(dt) = d["time"].as_array() {
        for i in 0..dt.len() {
            days.push(json!({
                "d": dt[i], "hi": d["temperature_2m_max"][i].as_f64().unwrap_or(0.0).round(),
                "lo": d["temperature_2m_min"][i].as_f64().unwrap_or(0.0).round(), "c": d["weather_code"][i].as_i64().unwrap_or(0)
            }));
        }
    }
    Some(json!({
        "type": "weather",
        "city": p.city,
        "lat": lat, "lon": lon,
        "units": if p.units_f { "F" } else { "C" },
        "temp": cur["temperature_2m"].as_f64()?.round(),
        "feels": cur["apparent_temperature"].as_f64().unwrap_or(0.0).round(),
        "code": cur["weather_code"].as_i64().unwrap_or(0),
        "wind": cur["wind_speed_10m"].as_f64().unwrap_or(0.0).round(),
        "humidity": cur["relative_humidity_2m"].as_i64().unwrap_or(0),
        "isDay": cur["is_day"].as_i64() == Some(1),
        "hi": days.first().map(|x| x["hi"].clone()),
        "lo": days.first().map(|x| x["lo"].clone()),
        "sunrise": d["sunrise"][0], "sunset": d["sunset"][0],
        "hours": hours, "days": days,
        "fetched": now_ms() / 1000,
    }))
}

/// "2026-11-21T14:00" → minutes since an arbitrary epoch (only differences / hour-of-day matter).
fn parse_local(s: &str) -> i64 {
    let n = |a: usize, b: usize| s.get(a..b).and_then(|x| x.parse::<i64>().ok()).unwrap_or(0);
    let (y, mo, d, h, mi) = (n(0, 4), n(5, 7), n(8, 10), n(11, 13), n(14, 16));
    let days = y * 372 + mo * 31 + d;
    days * 1440 + h * 60 + mi
}

/// City name → settings patch for "weather" (Open-Meteo geocoding). Blocking.
pub fn geocode(city: &str) -> Option<Value> {
    let raw = get_json(&format!("https://geocoding-api.open-meteo.com/v1/search?count=1&language=en&name={}", enc(city))).ok()??;
    let r = raw["results"].get(0)?;
    let mut label = r["name"].as_str()?.to_string();
    if let Some(c) = r["country"].as_str() {
        label.push_str(", ");
        label.push_str(c);
    }
    Some(json!({ "city": label, "lat": r["latitude"].as_f64()?, "lon": r["longitude"].as_f64()? }))
}

/// Windows location services (only when the user presses "Use my location"). Blocking.
pub fn locate() -> Option<Value> {
    use windows::Devices::Geolocation::*;
    use windows::Foundation::TimeSpan;
    unsafe {
        let _ = windows::Win32::System::Com::CoInitializeEx(None, windows::Win32::System::Com::COINIT_MULTITHREADED);
    }
    let access = Geolocator::RequestAccessAsync().ok()?.get().ok()?;
    if access != GeolocationAccessStatus::Allowed {
        return None;
    }
    let g = Geolocator::new().ok()?;
    let _ = g.SetDesiredAccuracy(PositionAccuracy::Default);
    let pos = g
        .GetGeopositionAsyncWithAgeAndTimeout(TimeSpan { Duration: 30 * 60 * 10_000_000 }, TimeSpan { Duration: 10 * 10_000_000 })
        .ok()?
        .get()
        .ok()?;
    let p = pos.Coordinate().ok()?.Point().ok()?.Position().ok()?;
    let (lat, lon) = ((p.Latitude * 100.0).round() / 100.0, (p.Longitude * 100.0).round() / 100.0);
    let mut name = "My location".to_string();
    if let Ok(Some(raw)) = get_json(&format!("https://geocoding-api.open-meteo.com/v1/search?count=1&name={lat},{lon}")) {
        if let Some(n) = raw["results"][0]["name"].as_str() {
            name = n.to_string();
        }
    }
    Some(json!({ "city": name, "lat": lat, "lon": lon }))
}
