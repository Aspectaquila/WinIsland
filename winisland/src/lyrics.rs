//! Time-synced lyrics from LRCLIB (lrclib.net: open-source, free, no account). Only the track's artist, title,
//! album and length are sent. Fetched on demand and cached on disk, so each song is looked up once.

use crate::model::LyricsResult;
use crate::net::{enc, get_json};
use regex::Regex;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

fn mem() -> &'static Mutex<HashMap<String, Value>> {
    static M: OnceLock<Mutex<HashMap<String, Value>>> = OnceLock::new();
    M.get_or_init(|| Mutex::new(HashMap::new()))
}

fn to_result(v: &Value) -> LyricsResult {
    LyricsResult {
        found: v["found"].as_bool().unwrap_or(false),
        synced: v["synced"].as_str().map(str::to_string),
        plain: v["plain"].as_str().map(str::to_string),
        instrumental: v["instrumental"].as_bool().unwrap_or(false),
    }
}

/// Blocking: call from a background thread.
pub fn get(artist: &str, title: &str, album: &str, duration: f64, browser: bool) -> LyricsResult {
    if title.trim().is_empty() {
        return LyricsResult::default();
    }
    let dur = duration.round_ties_even();
    let key = hash(&format!("{artist}|{title}|{dur}"));
    if let Some(hit) = mem().lock().unwrap().get(&key) {
        return to_result(hit);
    }
    let dir = crate::settings::cache_dir().join("lyrics");
    let file = dir.join(format!("{key}.json"));
    if let Ok(text) = std::fs::read_to_string(&file) {
        if let Ok(v @ Value::Object(_)) = serde_json::from_str::<Value>(&text) {
            mem().lock().unwrap().insert(key, v.clone());
            return to_result(&v);
        }
    }

    let good = |o: &Value| o.is_object() && (o["syncedLyrics"].is_string() || o["instrumental"].as_bool() == Some(true));
    let mut best: Option<Value> = None;
    let mut tries: Vec<(String, String)> = vec![];
    if !browser && !artist.is_empty() {
        tries.push((artist.to_string(), title.trim().to_string()));
    }
    let c = clean(artist, title, browser);
    if !tries.contains(&c) {
        tries.push(c);
    }
    let mut failed = false;
    'outer: for (a, t) in &tries {
        // 1) exact match (LRCLIB tolerates ±2 s of duration)
        if !a.is_empty() && duration > 0.0 {
            let mut q = format!("https://lrclib.net/api/get?artist_name={}&track_name={}&duration={}", enc(a), enc(t), dur);
            if !album.is_empty() && !browser {
                q.push_str(&format!("&album_name={}", enc(album)));
            }
            match get_json(&q) {
                Ok(Some(o)) if o.is_object() => {
                    if good(&o) {
                        best = Some(o);
                        break 'outer;
                    }
                    if best.is_none() {
                        best = Some(o);
                    }
                }
                Ok(_) => {}
                Err(_) => {
                    failed = true;
                    break 'outer;
                }
            }
        }
        // 2) search, preferring synced results with the closest length
        let q = if !a.is_empty() {
            format!("https://lrclib.net/api/search?artist_name={}&track_name={}", enc(a), enc(t))
        } else {
            format!("https://lrclib.net/api/search?q={}", enc(t))
        };
        match get_json(&q) {
            Ok(Some(Value::Array(arr))) => {
                let mut cands: Vec<&Value> = arr
                    .iter()
                    .filter(|o| o.is_object())
                    .filter(|o| duration <= 0.0 || (o["duration"].as_f64().unwrap_or(duration) - duration).abs() < 12.0)
                    .collect();
                cands.sort_by(|x, y| {
                    let sx = x["syncedLyrics"].is_string();
                    let sy = y["syncedLyrics"].is_string();
                    sy.cmp(&sx).then_with(|| {
                        let dx = if duration > 0.0 { (x["duration"].as_f64().unwrap_or(0.0) - duration).abs() } else { 0.0 };
                        let dy = if duration > 0.0 { (y["duration"].as_f64().unwrap_or(0.0) - duration).abs() } else { 0.0 };
                        dx.partial_cmp(&dy).unwrap_or(std::cmp::Ordering::Equal)
                    })
                });
                if let Some(pick) = cands.first() {
                    if good(pick) {
                        best = Some((*pick).clone());
                        break 'outer;
                    }
                    if best.is_none() {
                        best = Some((*pick).clone());
                    }
                }
            }
            Ok(_) => {}
            Err(_) => {
                failed = true;
                break 'outer;
            }
        }
    }
    if failed && best.is_none() {
        return LyricsResult::default(); // offline: don't cache, try again next time
    }
    let b = best.unwrap_or(Value::Null);
    let entry = json!({
        "found": b.is_object() && (b["syncedLyrics"].is_string() || b["plainLyrics"].is_string() || b["instrumental"].as_bool() == Some(true)),
        "synced": b["syncedLyrics"].as_str(),
        "plain": b["plainLyrics"].as_str(),
        "instrumental": b["instrumental"].as_bool().unwrap_or(false),
    });
    mem().lock().unwrap().insert(key, entry.clone());
    let _ = std::fs::create_dir_all(&dir);
    let _ = std::fs::write(&file, entry.to_string());
    to_result(&entry)
}

fn re(p: &str) -> Regex {
    Regex::new(p).unwrap()
}

/// YouTube & co.: "Artist - Song (Official Video) [4K]" with the channel as artist ("ArtistVEVO", "Artist - Topic").
pub fn clean(artist: &str, title: &str, browser: bool) -> (String, String) {
    let mut title = re(r"(?i)\s*[\(\[【][^\)\]】]*(official|video|audio|lyric|visuali[sz]er|mv|m/v|4k|hd|hq|remaster|live performance)[^\)\]】]*[\)\]】]")
        .replace_all(title, "")
        .to_string();
    title = re(r"\s*\|.*$").replace(&title, "").trim().to_string();
    let mut artist = re(r"(?i)\s*-\s*Topic$|VEVO$|\s+Official$").replace(artist, "").trim().to_string();
    if browser || artist.is_empty() {
        if let Some(m) = re(r"^(.+?)\s+[-–—]\s+(.+)$").captures(&title) {
            artist = m[1].trim().to_string();
            title = m[2].trim().to_string();
        }
    }
    title = re(r"(?i)\s*[\(\[](feat|ft|with|from)\b\.?[^\)\]]*[\)\]]").replace_all(&title, "").trim().to_string();
    title = re(r"(?i)\s+(feat|ft)\.\s.*$").replace(&title, "").trim().to_string();
    title = re(r"(?i)\s+-\s+(\d{4}\s+)?(remaster(ed)?|live|radio edit|mono|stereo|single version|bonus track).*$")
        .replace(&title, "")
        .trim()
        .to_string();
    (artist, title)
}

fn hash(s: &str) -> String {
    let d = sha1_smol::Sha1::from(s.to_lowercase()).digest().to_string().to_uppercase();
    d[..16].to_string()
}

/// "[mm:ss.xx] text" lines → sorted (seconds, text)
pub fn parse_lrc(s: &str) -> Vec<(f64, String)> {
    let tag = re(r"\[(\d+):(\d+(?:[.:]\d+)?)\]");
    let strip = re(r"\[[^\]]*\]");
    let mut out = vec![];
    for line in s.lines() {
        let tags: Vec<_> = tag.captures_iter(line).collect();
        if tags.is_empty() {
            continue;
        }
        let text = strip.replace_all(line, "").trim().to_string();
        for t in tags {
            let m: f64 = t[1].parse().unwrap_or(0.0);
            let sec: f64 = t[2].replace(':', ".").parse().unwrap_or(0.0);
            out.push((m * 60.0 + sec, text.clone()));
        }
    }
    out.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
    out
}

#[cfg(test)]
mod tests {
    #[test]
    fn cleans() {
        let (a, t) = super::clean("ArtistVEVO", "Artist - Song (Official Video) [4K]", true);
        assert_eq!((a.as_str(), t.as_str()), ("Artist", "Song"));
        let l = super::parse_lrc("[00:09.60]Cold light\n[00:13.80]Silver");
        assert_eq!(l.len(), 2);
        assert!((l[0].0 - 9.6).abs() < 1e-6);
    }
}
