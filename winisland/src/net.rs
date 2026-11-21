//! HTTP for Open-Meteo and LRCLIB. Generic User-Agent, nothing personal is sent.

use std::sync::{Arc, OnceLock};
use std::time::Duration;

fn agent() -> &'static ureq::Agent {
    static A: OnceLock<ureq::Agent> = OnceLock::new();
    A.get_or_init(|| {
        let mut b = ureq::AgentBuilder::new()
            .user_agent("WinIsland/1.0")
            .timeout_connect(Duration::from_secs(8))
            .timeout(Duration::from_secs(12));
        if let Ok(tls) = native_tls::TlsConnector::new() {
            b = b.tls_connector(Arc::new(tls));
        }
        b.build()
    })
}

/// GET → parsed JSON. `Ok(None)` for a non-success HTTP status, `Err` for network failures.
pub fn get_json(url: &str) -> Result<Option<serde_json::Value>, String> {
    match agent().get(url).call() {
        Ok(r) => {
            let text = r.into_string().map_err(|e| e.to_string())?;
            Ok(serde_json::from_str(&text).ok())
        }
        Err(ureq::Error::Status(_, _)) => Ok(None),
        Err(e) => Err(e.to_string()),
    }
}

pub fn enc(s: &str) -> String {
    let mut out = String::new();
    for b in s.as_bytes() {
        match *b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => out.push(*b as char),
            _ => out.push_str(&format!("%{:02X}", b)),
        }
    }
    out
}
