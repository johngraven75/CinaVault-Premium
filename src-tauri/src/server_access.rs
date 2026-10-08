//! Who is calling the embedded media server, and what the Feature Matrix
//! currently lets the server do: remote access, API keys, the bandwidth cap,
//! caching and the parental filter. The server reads a snapshot of these at
//! most every couple of seconds instead of on every request.

use crate::db::Database;
use crate::feature_flags;
use crate::parental;
use crate::server_cache::{DEFAULT_ARTWORK_CACHE_MB, DEFAULT_LIBRARY_MAX_AGE};
use crate::throttle;
use axum::http::HeaderMap;
use serde_json::{json, Value};
use std::net::IpAddr;

/// Where a request comes from, as far as access rules care.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClientOrigin {
    /// This machine, not through a proxy.
    Loopback,
    /// A private-network address (home LAN, link-local).
    Lan,
    /// Anything else, including traffic relayed through a tunnel or proxy.
    Remote,
}

impl ClientOrigin {
    pub fn as_str(self) -> &'static str {
        match self {
            ClientOrigin::Loopback => "loopback",
            ClientOrigin::Lan => "lan",
            ClientOrigin::Remote => "remote",
        }
    }
}

/// Headers proxies and tunnels add. The cloud relay connects over loopback, so
/// their presence marks a request as relayed from outside.
const PROXY_HEADERS: &[&str] = &[
    "cf-connecting-ip",
    "cf-ray",
    "x-forwarded-for",
    "x-forwarded-host",
    "forwarded",
    "x-real-ip",
];

fn is_private(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => v4.is_private() || v4.is_link_local(),
        IpAddr::V6(v6) => {
            if let Some(mapped) = v6.to_ipv4_mapped() {
                return is_private(IpAddr::V4(mapped));
            }
            let first = v6.segments()[0];
            (first & 0xfe00) == 0xfc00 || (first & 0xffc0) == 0xfe80
        }
    }
}

fn is_loopback(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => v4.is_loopback(),
        IpAddr::V6(v6) => {
            v6.is_loopback() || v6.to_ipv4_mapped().is_some_and(|v4| v4.is_loopback())
        }
    }
}

/// Classifies a request. A request that went through any proxy counts as
/// remote whatever address the proxy claims, so a relay cannot be used to look
/// local. An unknown peer is treated as remote.
pub fn classify(peer: Option<IpAddr>, headers: &HeaderMap) -> ClientOrigin {
    let Some(peer) = peer else {
        return ClientOrigin::Remote;
    };
    if PROXY_HEADERS.iter().any(|name| headers.contains_key(*name)) {
        return ClientOrigin::Remote;
    }
    if is_loopback(peer) {
        ClientOrigin::Loopback
    } else if is_private(peer) {
        ClientOrigin::Lan
    } else {
        ClientOrigin::Remote
    }
}

/// What the switches say, read in one go.
#[derive(Debug, Clone, PartialEq)]
pub struct ServerSwitches {
    pub remote_access: bool,
    pub api_keys: bool,
    /// Per-stream cap in bytes per second while "bandwidth_limit" is on.
    pub stream_cap: Option<f64>,
    pub cache: Option<CacheSettings>,
    /// Parental visibility predicate for the active profile.
    pub library_filter: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CacheSettings {
    pub artwork_bytes: usize,
    pub library_max_age: u32,
}

fn number(config: &serde_json::Map<String, Value>, key: &str) -> Option<f64> {
    config.get(key).and_then(|value| match value {
        Value::Number(number) => number.as_f64(),
        Value::String(text) => text.trim().parse().ok(),
        _ => None,
    })
}

/// The bandwidth cap in Mbps: the switch config, else the Remote Access tab's
/// older upload-limit setting, else 20.
pub fn bandwidth_mbps(db: &Database) -> f64 {
    number(&feature_flags::config(db, "bandwidth_limit"), "mbps")
        .or_else(|| {
            db.get_setting_data("remote_upload_limit_mbps")
                .ok()
                .flatten()
                .and_then(|raw| raw.trim().parse().ok())
        })
        .filter(|mbps: &f64| mbps.is_finite() && *mbps > 0.0)
        .unwrap_or(throttle::DEFAULT_MBPS)
}

pub fn read_switches(db: &Database) -> ServerSwitches {
    let cache = feature_flags::is_enabled(db, "cdn_cache").then(|| {
        let config = feature_flags::config(db, "cdn_cache");
        CacheSettings {
            artwork_bytes: (number(&config, "artworkCacheMb")
                .unwrap_or(DEFAULT_ARTWORK_CACHE_MB as f64)
                .clamp(0.0, 2048.0)
                * 1024.0
                * 1024.0) as usize,
            library_max_age: number(&config, "libraryMaxAge")
                .unwrap_or(DEFAULT_LIBRARY_MAX_AGE as f64)
                .clamp(0.0, 3600.0) as u32,
        }
    });
    ServerSwitches {
        remote_access: feature_flags::is_enabled(db, "remote_access"),
        api_keys: feature_flags::is_enabled(db, "api_keys"),
        stream_cap: feature_flags::is_enabled(db, "bandwidth_limit")
            .then(|| throttle::bytes_per_second(bandwidth_mbps(db))),
        cache,
        library_filter: parental::active_filter(db),
    }
}

/// One-time move of the Remote Access tab's own settings into the switches:
/// `remote_access_enabled` becomes the "remote_access" switch and
/// `remote_upload_limit_mbps` the "bandwidth_limit" config. Only applies while
/// the switch has never been saved, so it never overrides a matrix choice.
pub fn migrate_legacy_settings(db: &Database) -> rusqlite::Result<()> {
    let saved = |key: &str| -> rusqlite::Result<bool> {
        db.conn.query_row(
            "SELECT COUNT(*) FROM feature_settings WHERE feature_key = ?1",
            [key],
            |row| row.get::<_, i64>(0).map(|count| count > 0),
        )
    };
    if !saved("remote_access")? {
        if let Some(raw) = db.get_setting_data("remote_access_enabled")? {
            db.set_feature_setting_data("remote_access", raw.trim() != "false", "{}")?;
        }
    }
    if !saved("bandwidth_limit")? {
        if let Some(mbps) = db
            .get_setting_data("remote_upload_limit_mbps")?
            .and_then(|raw| raw.trim().parse::<f64>().ok())
            .filter(|mbps| *mbps > 0.0)
        {
            db.set_feature_setting_data(
                "bandwidth_limit",
                feature_flags::default_enabled("bandwidth_limit"),
                &json!({ "mbps": mbps }).to_string(),
            )?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::HeaderValue;

    fn ip(text: &str) -> Option<IpAddr> {
        Some(text.parse().unwrap())
    }

    #[test]
    fn clients_are_classified_by_address_and_proxy_headers() {
        let none = HeaderMap::new();
        assert_eq!(classify(ip("127.0.0.1"), &none), ClientOrigin::Loopback);
        assert_eq!(classify(ip("::1"), &none), ClientOrigin::Loopback);
        assert_eq!(
            classify(ip("::ffff:127.0.0.1"), &none),
            ClientOrigin::Loopback
        );
        assert_eq!(classify(ip("192.168.1.20"), &none), ClientOrigin::Lan);
        assert_eq!(classify(ip("10.0.0.5"), &none), ClientOrigin::Lan);
        assert_eq!(classify(ip("172.16.4.1"), &none), ClientOrigin::Lan);
        assert_eq!(classify(ip("169.254.10.1"), &none), ClientOrigin::Lan);
        assert_eq!(classify(ip("fd12::1"), &none), ClientOrigin::Lan);
        assert_eq!(classify(ip("fe80::1"), &none), ClientOrigin::Lan);
        assert_eq!(classify(ip("::ffff:192.168.0.2"), &none), ClientOrigin::Lan);
        assert_eq!(classify(ip("8.8.8.8"), &none), ClientOrigin::Remote);
        assert_eq!(classify(ip("172.32.0.1"), &none), ClientOrigin::Remote);
        assert_eq!(classify(ip("2001:db8::1"), &none), ClientOrigin::Remote);
        assert_eq!(classify(None, &none), ClientOrigin::Remote);

        let mut relayed = HeaderMap::new();
        relayed.insert("cf-connecting-ip", HeaderValue::from_static("192.168.1.2"));
        assert_eq!(classify(ip("127.0.0.1"), &relayed), ClientOrigin::Remote);
        let mut forwarded = HeaderMap::new();
        forwarded.insert("x-forwarded-for", HeaderValue::from_static("10.0.0.1"));
        assert_eq!(
            classify(ip("192.168.1.9"), &forwarded),
            ClientOrigin::Remote
        );
    }

    fn temp_db() -> (Database, std::path::PathBuf) {
        let path =
            std::env::temp_dir().join(format!("cinavault-access-{}.db", uuid::Uuid::new_v4()));
        (Database::new(path.to_str().unwrap()).unwrap(), path)
    }

    #[test]
    fn switches_snapshot_reads_defaults_and_configs() {
        let (db, path) = temp_db();
        let defaults = read_switches(&db);
        assert!(defaults.remote_access);
        assert!(defaults.api_keys);
        assert_eq!(defaults.stream_cap, None, "bandwidth_limit defaults off");
        assert_eq!(
            defaults.cache,
            Some(CacheSettings {
                artwork_bytes: 64 * 1024 * 1024,
                library_max_age: 30
            })
        );
        assert_eq!(defaults.library_filter, None);

        db.set_feature_setting_data("bandwidth_limit", true, r#"{"mbps":"8"}"#)
            .unwrap();
        db.set_feature_setting_data("cdn_cache", false, "{}")
            .unwrap();
        db.set_feature_setting_data("remote_access", false, "{}")
            .unwrap();
        let changed = read_switches(&db);
        assert_eq!(changed.stream_cap, Some(1_000_000.0));
        assert_eq!(changed.cache, None);
        assert!(!changed.remote_access);
        std::fs::remove_file(path).ok();
    }

    #[test]
    fn legacy_remote_settings_move_into_the_switches_once() {
        let (db, path) = temp_db();
        db.set_setting_data("remote_access_enabled", "false")
            .unwrap();
        db.set_setting_data("remote_upload_limit_mbps", "12")
            .unwrap();
        migrate_legacy_settings(&db).unwrap();
        assert!(!feature_flags::is_enabled(&db, "remote_access"));
        assert_eq!(bandwidth_mbps(&db), 12.0);
        assert!(!feature_flags::is_enabled(&db, "bandwidth_limit"));
        // A later matrix choice is never overridden.
        db.set_feature_setting_data("remote_access", true, "{}")
            .unwrap();
        migrate_legacy_settings(&db).unwrap();
        assert!(feature_flags::is_enabled(&db, "remote_access"));
        std::fs::remove_file(path).ok();
    }
}
