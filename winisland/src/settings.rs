//! Settings live in one JSON document (settings.json). Same schema as the C# version (src\Settings.cs):
//! defaults are merged under the user's file, unknown keys are preserved, missing keys fall back to defaults.
//! The only schema change: `island.style` is "notch" | "island" (old values "macos" / "ios" are migrated).

use serde_json::{Map, Value};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

const DEFAULTS: &str = r#"{
  "island":  { "style": "notch", "enabled": true, "time": true, "weather": true, "media": true, "volume": true, "system": true,
               "notifications": true, "events": true, "collapseDelay": 0, "hideInFullscreen": true, "glow": true,
               "visualizer": true, "trackPopup": true, "lyrics": true, "lyricsNotch": true, "privacy": true, "hoverExpand": false, "idleInfo": true },
  "weather": { "city": "", "lat": null, "lon": null, "units": "c" },
  "general": { "autostart": true, "hotkey": true }
}"#;

/// The install root: the parent of the folder holding the exe (bin\winisland.exe → install folder).
pub fn root() -> &'static Path {
    static ROOT: OnceLock<PathBuf> = OnceLock::new();
    ROOT.get_or_init(|| {
        let exe = std::env::current_exe().unwrap_or_else(|_| PathBuf::from("."));
        let dir = exe.parent().map(Path::to_path_buf).unwrap_or_else(|| PathBuf::from("."));
        dir.parent().map(Path::to_path_buf).unwrap_or(dir)
    })
}

/// Where settings and caches live: the install root when writable, otherwise %LOCALAPPDATA%\WinIsland.
pub fn data() -> &'static Path {
    static DATA: OnceLock<PathBuf> = OnceLock::new();
    DATA.get_or_init(|| {
        let r = root();
        let probe = r.join(".write-test");
        if std::fs::write(&probe, b"").is_ok() {
            let _ = std::fs::remove_file(&probe);
            return r.to_path_buf();
        }
        let base = std::env::var_os("LOCALAPPDATA").map(PathBuf::from).unwrap_or_else(std::env::temp_dir);
        let d = base.join("WinIsland");
        let _ = std::fs::create_dir_all(&d);
        d
    })
}

pub fn cache_dir() -> PathBuf {
    data().join("cache")
}

pub fn file_path() -> PathBuf {
    data().join("settings.json")
}

pub fn defaults() -> Value {
    serde_json::from_str(DEFAULTS).expect("defaults")
}

pub fn merge(target: &mut Value, src: &Value) {
    if let (Value::Object(t), Value::Object(s)) = (&mut *target, src) {
        for (k, v) in s {
            match (t.get_mut(k), v) {
                (Some(tv @ Value::Object(_)), Value::Object(_)) => merge(tv, v),
                _ => {
                    t.insert(k.clone(), v.clone());
                }
            }
        }
    } else {
        *target = src.clone();
    }
}

/// "macos" → "notch", "ios" → "island"; anything unknown → "notch".
pub fn migrate(doc: &mut Value) {
    if let Some(island) = doc.get_mut("island").and_then(Value::as_object_mut) {
        let style = island.get("style").and_then(Value::as_str).unwrap_or("notch");
        let fixed = match style {
            "ios" | "island" => "island",
            _ => "notch",
        };
        island.insert("style".into(), Value::String(fixed.into()));
    }
}

#[derive(Clone)]
pub struct Settings {
    pub doc: Value,
    /// Demo / preview mode: never written to disk.
    pub read_only: bool,
}

impl Settings {
    pub fn load(read_only: bool) -> Self {
        let mut doc = defaults();
        if let Ok(text) = std::fs::read_to_string(file_path()) {
            if let Ok(user @ Value::Object(_)) = serde_json::from_str::<Value>(text.trim_start_matches('\u{feff}')) {
                merge(&mut doc, &user);
            }
        }
        migrate(&mut doc);
        Settings { doc, read_only }
    }

    pub fn save(&self) {
        if self.read_only {
            return;
        }
        let path = file_path();
        let tmp = path.with_extension("json.tmp");
        if let Ok(text) = serde_json::to_string_pretty(&self.doc) {
            if std::fs::write(&tmp, text).is_ok() {
                let _ = std::fs::rename(&tmp, &path);
            }
        }
    }

    pub fn apply(&mut self, patch: &Value) {
        merge(&mut self.doc, patch);
        migrate(&mut self.doc);
        self.save();
    }

    pub fn reset(&mut self) {
        self.doc = defaults();
        self.save();
    }

    pub fn get(&self, path: &str) -> Option<&Value> {
        let mut n = &self.doc;
        for part in path.split('.') {
            n = n.get(part)?;
        }
        Some(n)
    }

    pub fn b(&self, path: &str) -> bool {
        self.get(path).and_then(Value::as_bool).unwrap_or(false)
    }

    pub fn f(&self, path: &str) -> Option<f64> {
        self.get(path).and_then(Value::as_f64)
    }

    pub fn s(&self, path: &str) -> String {
        self.get(path).and_then(Value::as_str).unwrap_or("").to_string()
    }

    pub fn island(&self, k: &str) -> bool {
        self.b(&format!("island.{k}"))
    }

    pub fn is_island_style(&self) -> bool {
        self.s("island.style") == "island"
    }
}

/// Build `{"a":{"b":value}}` from "a.b".
pub fn patch(path: &str, v: Value) -> Value {
    let mut out = v;
    for part in path.rsplit('.') {
        let mut m = Map::new();
        m.insert(part.to_string(), out);
        out = Value::Object(m);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn migrates() {
        let mut d = defaults();
        merge(&mut d, &serde_json::json!({"island": {"style": "ios"}}));
        migrate(&mut d);
        assert_eq!(d["island"]["style"], "island");
        let mut d = defaults();
        merge(&mut d, &serde_json::json!({"island": {"style": "macos"}}));
        migrate(&mut d);
        assert_eq!(d["island"]["style"], "notch");
        assert_eq!(patch("a.b", Value::Bool(true)), serde_json::json!({"a":{"b":true}}));
    }
}
