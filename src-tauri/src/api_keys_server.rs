//! API keys for apps that talk to the embedded media server ("api_keys"
//! switch). Only a salted hash of each key is stored; the full key is shown
//! once when it is issued. Lookups go by the key's public prefix, then a
//! constant-time hash comparison.
//!
//! The `api_keys` table holds metadata-provider credentials, so app keys live
//! in `app_api_keys`.

use crate::db::Database;
use crate::user_data;
use crate::AppState;
use rusqlite::{params, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sha2::{Digest, Sha256};
use tauri::State;

pub const PERMISSION_LIBRARY: &str = "library:read";
pub const PERMISSION_STREAM: &str = "stream:play";
const KEY_PREFIX: &str = "cvk_";

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ApiKeySummary {
    pub id: i64,
    pub name: String,
    /// Public part, safe to display: "cvk_1a2b3c4d".
    pub prefix: String,
    pub permissions: Vec<String>,
    pub created_at: String,
    pub last_used_at: Option<String>,
    pub revoked_at: Option<String>,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct IssuedApiKey {
    /// The full key. It is never retrievable again.
    pub key: String,
    pub summary: ApiKeySummary,
}

pub fn ensure_tables(db: &Database) -> rusqlite::Result<()> {
    db.conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS app_api_keys (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            name TEXT NOT NULL,
            prefix TEXT NOT NULL UNIQUE,
            salt TEXT NOT NULL,
            key_hash TEXT NOT NULL,
            permissions TEXT NOT NULL,
            created_at TEXT NOT NULL,
            last_used_at TEXT,
            revoked_at TEXT
        );",
    )
}

fn hash_key(salt: &str, key: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"cinavault-app-api-key-v1:");
    hasher.update(salt.as_bytes());
    hasher.update(b":");
    hasher.update(key.as_bytes());
    hasher
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn constant_time_eq(left: &[u8], right: &[u8]) -> bool {
    left.len() == right.len()
        && left
            .iter()
            .zip(right)
            .fold(0u8, |acc, (a, b)| acc | (a ^ b))
            == 0
}

/// "cvk_<8 hex>_<48 hex>" -> "cvk_<8 hex>"
fn prefix_of(key: &str) -> Option<&str> {
    let rest = key.strip_prefix(KEY_PREFIX)?;
    let (public, secret) = rest.split_once('_')?;
    (public.len() == 8
        && secret.len() == 48
        && public
            .chars()
            .chain(secret.chars())
            .all(|c| c.is_ascii_hexdigit()))
    .then(|| &key[..KEY_PREFIX.len() + 8])
}

fn normalize_permissions(permissions: &[String]) -> Result<Vec<String>, String> {
    let mut out: Vec<String> = Vec::new();
    for permission in permissions {
        let permission = permission.trim();
        if ![PERMISSION_LIBRARY, PERMISSION_STREAM].contains(&permission) {
            return Err(format!("Unknown permission: {permission}"));
        }
        if !out.iter().any(|existing| existing == permission) {
            out.push(permission.to_string());
        }
    }
    if out.is_empty() {
        return Err("Choose at least one permission".into());
    }
    out.sort();
    Ok(out)
}

fn row_to_summary(row: &rusqlite::Row) -> rusqlite::Result<ApiKeySummary> {
    let permissions: String = row.get(3)?;
    Ok(ApiKeySummary {
        id: row.get(0)?,
        name: row.get(1)?,
        prefix: row.get(2)?,
        permissions: permissions
            .split(',')
            .filter(|value| !value.is_empty())
            .map(String::from)
            .collect(),
        created_at: row.get(4)?,
        last_used_at: row.get(5)?,
        revoked_at: row.get(6)?,
    })
}

const SUMMARY_COLUMNS: &str = "id, name, prefix, permissions, created_at, last_used_at, revoked_at";

pub fn issue(db: &Database, name: &str, permissions: &[String]) -> Result<IssuedApiKey, String> {
    ensure_tables(db).map_err(|error| error.to_string())?;
    let name = name.trim();
    if name.is_empty() || name.chars().count() > 60 {
        return Err("Give the key a name of 1 to 60 characters".into());
    }
    let permissions = normalize_permissions(permissions)?;
    let random = format!(
        "{}{}",
        uuid::Uuid::new_v4().simple(),
        uuid::Uuid::new_v4().simple()
    );
    let key = format!("{KEY_PREFIX}{}_{}", &random[..8], &random[8..56]);
    let prefix = prefix_of(&key)
        .ok_or("Generated key is malformed")?
        .to_string();
    let salt = uuid::Uuid::new_v4().simple().to_string();
    let created_at = chrono::Utc::now().to_rfc3339();
    db.conn
        .execute(
            "INSERT INTO app_api_keys (name, prefix, salt, key_hash, permissions, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                name,
                prefix,
                salt,
                hash_key(&salt, &key),
                permissions.join(","),
                created_at
            ],
        )
        .map_err(|error| error.to_string())?;
    let id = db.conn.last_insert_rowid();
    Ok(IssuedApiKey {
        key,
        summary: ApiKeySummary {
            id,
            name: name.to_string(),
            prefix,
            permissions,
            created_at,
            last_used_at: None,
            revoked_at: None,
        },
    })
}

pub fn list(db: &Database) -> Result<Vec<ApiKeySummary>, String> {
    ensure_tables(db).map_err(|error| error.to_string())?;
    let mut stmt = db
        .conn
        .prepare(&format!(
            "SELECT {SUMMARY_COLUMNS} FROM app_api_keys ORDER BY revoked_at IS NOT NULL, id DESC"
        ))
        .map_err(|error| error.to_string())?;
    let rows = stmt
        .query_map([], row_to_summary)
        .map_err(|error| error.to_string())?;
    rows.collect::<Result<_, _>>()
        .map_err(|error| error.to_string())
}

pub fn revoke(db: &Database, id: i64) -> Result<ApiKeySummary, String> {
    ensure_tables(db).map_err(|error| error.to_string())?;
    db.conn
        .execute(
            "UPDATE app_api_keys SET revoked_at = ?2 WHERE id = ?1 AND revoked_at IS NULL",
            params![id, chrono::Utc::now().to_rfc3339()],
        )
        .map_err(|error| error.to_string())?;
    db.conn
        .query_row(
            &format!("SELECT {SUMMARY_COLUMNS} FROM app_api_keys WHERE id = ?1"),
            params![id],
            row_to_summary,
        )
        .optional()
        .map_err(|error| error.to_string())?
        .ok_or_else(|| format!("No API key {id}"))
}

/// The active key matching `key`, recording its use. None for unknown,
/// malformed or revoked keys.
pub fn authenticate(db: &Database, key: &str) -> Result<Option<ApiKeySummary>, String> {
    let key = key.trim();
    let Some(prefix) = prefix_of(key) else {
        return Ok(None);
    };
    ensure_tables(db).map_err(|error| error.to_string())?;
    let row = db
        .conn
        .query_row(
            &format!(
                "SELECT {SUMMARY_COLUMNS}, salt, key_hash FROM app_api_keys
                 WHERE prefix = ?1 AND revoked_at IS NULL"
            ),
            params![prefix],
            |row| {
                Ok((
                    row_to_summary(row)?,
                    row.get::<_, String>(7)?,
                    row.get::<_, String>(8)?,
                ))
            },
        )
        .optional()
        .map_err(|error| error.to_string())?;
    let Some((summary, salt, stored)) = row else {
        return Ok(None);
    };
    if !constant_time_eq(hash_key(&salt, key).as_bytes(), stored.as_bytes()) {
        return Ok(None);
    }
    let now = chrono::Utc::now().to_rfc3339();
    db.conn
        .execute(
            "UPDATE app_api_keys SET last_used_at = ?2 WHERE id = ?1",
            params![summary.id, now],
        )
        .ok();
    Ok(Some(ApiKeySummary {
        last_used_at: Some(now),
        ..summary
    }))
}

/// Server permissions an API key grants. Reading the library also allows the
/// server info endpoint an app needs to identify the server.
pub fn server_permissions(summary: &ApiKeySummary) -> Vec<String> {
    let mut permissions = summary.permissions.clone();
    if permissions.iter().any(|p| p == PERMISSION_LIBRARY) {
        permissions.push("server:read".into());
    }
    permissions
}

/// Key from `X-Api-Key: <key>` or `Authorization: ApiKey <key>`.
pub fn key_from_headers(headers: &axum::http::HeaderMap) -> Option<String> {
    headers
        .get("x-api-key")
        .and_then(|value| value.to_str().ok())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(String::from)
        .or_else(|| {
            headers
                .get(axum::http::header::AUTHORIZATION)
                .and_then(|value| value.to_str().ok())
                .and_then(|value| value.strip_prefix("ApiKey "))
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(String::from)
        })
}

// ------------------------------------------------------------- commands ----

fn with_db<T>(
    state: &State<AppState>,
    f: impl FnOnce(&Database) -> Result<T, String>,
) -> Result<T, String> {
    let db = state.db.lock().map_err(|error| error.to_string())?;
    f(&db)
}

#[tauri::command]
pub fn api_keys_list(state: State<AppState>) -> Result<Vec<ApiKeySummary>, String> {
    with_db(&state, list)
}

#[tauri::command]
pub fn api_key_issue(
    state: State<AppState>,
    name: String,
    permissions: Vec<String>,
) -> Result<IssuedApiKey, String> {
    with_db(&state, |db| {
        let issued = issue(db, &name, &permissions)?;
        user_data::emit(
            db,
            "apikey.issued",
            &issued.summary.name,
            json!({ "prefix": issued.summary.prefix, "permissions": issued.summary.permissions }),
        );
        Ok(issued)
    })
}

#[tauri::command]
pub fn api_key_revoke(state: State<AppState>, id: i64) -> Result<ApiKeySummary, String> {
    with_db(&state, |db| {
        let summary = revoke(db, id)?;
        user_data::emit(
            db,
            "apikey.revoked",
            &summary.name,
            json!({ "prefix": summary.prefix }),
        );
        Ok(summary)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_db() -> (Database, std::path::PathBuf) {
        let path =
            std::env::temp_dir().join(format!("cinavault-apikeys-{}.db", uuid::Uuid::new_v4()));
        (Database::new(path.to_str().unwrap()).unwrap(), path)
    }

    #[test]
    fn keys_are_hashed_listed_by_prefix_and_revocable() {
        let (db, path) = temp_db();
        let issued = issue(
            &db,
            "Living room TV",
            &[
                "stream:play".into(),
                "library:read".into(),
                "stream:play".into(),
            ],
        )
        .unwrap();
        assert!(issued.key.starts_with("cvk_"));
        assert_eq!(issued.key.len(), 4 + 8 + 1 + 48);
        assert_eq!(issued.summary.prefix, &issued.key[..12]);
        assert_eq!(
            issued.summary.permissions,
            vec!["library:read", "stream:play"]
        );

        let stored: String = db
            .conn
            .query_row("SELECT key_hash FROM app_api_keys", [], |row| row.get(0))
            .unwrap();
        assert!(
            !stored.contains(&issued.key[13..]),
            "the secret is never stored"
        );
        let listed = list(&db).unwrap();
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].prefix, issued.summary.prefix);

        let found = authenticate(&db, &issued.key).unwrap().unwrap();
        assert_eq!(found.id, issued.summary.id);
        assert!(found.last_used_at.is_some());
        assert!(server_permissions(&found).contains(&"server:read".to_string()));

        let mut tampered = issued.key.clone();
        tampered.replace_range(20..21, if &issued.key[20..21] == "a" { "b" } else { "a" });
        assert!(authenticate(&db, &tampered).unwrap().is_none());
        assert!(authenticate(&db, "cvk_nothex").unwrap().is_none());

        let revoked = revoke(&db, issued.summary.id).unwrap();
        assert!(revoked.revoked_at.is_some());
        assert!(authenticate(&db, &issued.key).unwrap().is_none());
        std::fs::remove_file(path).ok();
    }

    #[test]
    fn issuing_validates_name_and_permissions() {
        let (db, path) = temp_db();
        assert!(issue(&db, " ", &["library:read".into()]).is_err());
        assert!(issue(&db, "x", &[]).is_err());
        assert!(issue(&db, "x", &["admin".into()]).is_err());
        let stream_only = issue(&db, "Player", &["stream:play".into()]).unwrap();
        assert!(!server_permissions(&stream_only.summary).contains(&"server:read".to_string()));
        std::fs::remove_file(path).ok();
    }

    #[test]
    fn keys_are_read_from_either_header() {
        let mut headers = axum::http::HeaderMap::new();
        assert_eq!(key_from_headers(&headers), None);
        headers.insert("authorization", "ApiKey cvk_abc".parse().unwrap());
        assert_eq!(key_from_headers(&headers).as_deref(), Some("cvk_abc"));
        headers.insert("x-api-key", " cvk_xyz ".parse().unwrap());
        assert_eq!(key_from_headers(&headers).as_deref(), Some("cvk_xyz"));
        let mut bearer = axum::http::HeaderMap::new();
        bearer.insert("authorization", "Bearer cvrs_token".parse().unwrap());
        assert_eq!(key_from_headers(&bearer), None);
    }
}
