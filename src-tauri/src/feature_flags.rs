//! Feature switches from Advanced > Feature Matrix, as seen by the back end.
//! Saved values come from the `feature_settings` table; a switch nobody has
//! touched uses the default in src/features/featureDefaults.json, the same
//! file the front end reads.

use crate::db::Database;
use rusqlite::{params, OptionalExtension};
use serde_json::Value;
use std::sync::OnceLock;

const DEFAULTS_JSON: &str = include_str!("../../src/features/featureDefaults.json");

fn defaults() -> &'static serde_json::Map<String, Value> {
    static DEFAULTS: OnceLock<serde_json::Map<String, Value>> = OnceLock::new();
    DEFAULTS.get_or_init(|| {
        serde_json::from_str(DEFAULTS_JSON).expect("featureDefaults.json must be a JSON object")
    })
}

/// Default for `key`; unknown keys are off.
pub fn default_enabled(key: &str) -> bool {
    defaults()
        .get(key)
        .and_then(Value::as_bool)
        .unwrap_or(false)
}

fn saved(db: &Database, key: &str) -> Option<(bool, String)> {
    db.conn
        .query_row(
            "SELECT enabled, config_json FROM feature_settings WHERE feature_key = ?1",
            params![key],
            |row| Ok((row.get::<_, bool>(0)?, row.get::<_, String>(1)?)),
        )
        .optional()
        .ok()
        .flatten()
}

/// Whether the switch is on: the saved value, else its default.
pub fn is_enabled(db: &Database, key: &str) -> bool {
    saved(db, key)
        .map(|(enabled, _)| enabled)
        .unwrap_or_else(|| default_enabled(key))
}

/// The switch's saved config object, or an empty object.
pub fn config(db: &Database, key: &str) -> serde_json::Map<String, Value> {
    saved(db, key)
        .and_then(|(_, raw)| serde_json::from_str::<Value>(&raw).ok())
        .and_then(|value| value.as_object().cloned())
        .unwrap_or_default()
}

/// Convenience for `State<AppState>` callers; a poisoned lock reads as the default.
pub fn is_enabled_state(state: &crate::AppState, key: &str) -> bool {
    state
        .db
        .lock()
        .map(|db| is_enabled(&db, key))
        .unwrap_or_else(|_| default_enabled(key))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_db() -> (Database, std::path::PathBuf) {
        let path =
            std::env::temp_dir().join(format!("cinavault-flags-{}.db", uuid::Uuid::new_v4()));
        (Database::new(path.to_str().unwrap()).unwrap(), path)
    }

    #[test]
    fn defaults_apply_until_a_switch_is_saved() {
        let (db, path) = temp_db();
        assert!(is_enabled(&db, "auto_metadata"));
        assert!(!is_enabled(&db, "webhook"));
        assert!(!is_enabled(&db, "no_such_switch"));
        db.set_feature_setting_data("auto_metadata", false, "{}")
            .unwrap();
        db.set_feature_setting_data("webhook", true, r#"{"urls":["https://example.com/hook"]}"#)
            .unwrap();
        assert!(!is_enabled(&db, "auto_metadata"));
        assert!(is_enabled(&db, "webhook"));
        assert_eq!(
            config(&db, "webhook")["urls"][0],
            "https://example.com/hook"
        );
        assert!(config(&db, "auto_metadata").is_empty());
        drop(db);
        std::fs::remove_file(path).ok();
    }

    #[test]
    fn every_default_is_a_boolean() {
        assert!(defaults().len() >= 40);
        assert!(defaults().values().all(Value::is_boolean));
    }
}
