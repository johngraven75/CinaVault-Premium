//! Parental controls ("parental_ctrl" switch): which titles a restricted
//! profile may see, and the PIN that guards leaving a restricted profile or
//! loosening the rules.
//!
//! Visibility is a SQL predicate over `media_items`, so every library query
//! (desktop commands, shelves and the embedded server) filters in the database
//! and pagination stays correct. Certifications are stored normalised in
//! `media_items.content_rating` (see `normalize_certification`), and the SQL age
//! expression is generated from the same table the Rust side uses, so both
//! agree on what "PG-13" means.

use crate::db::Database;
use crate::feature_flags;
use crate::user_data;
use crate::AppState;
use argon2::{
    password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Argon2,
};
use rusqlite::{params, OptionalExtension};
use serde::Serialize;
use serde_json::{json, Map, Value};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};
use tauri::State;

pub const SWITCH: &str = "parental_ctrl";
const UNLOCK_WINDOW: Duration = Duration::from_secs(5 * 60);
const MAX_FAILURES: u32 = 5;
const LOCKOUT: Duration = Duration::from_secs(60);

/// Certification label -> youngest age it suits. US film and TV, UK, and the
/// labels NFO files and TMDB commonly carry. Numeric labels ("12", "16") are
/// read as the age itself.
const RATING_AGES: &[(&str, u8)] = &[
    ("G", 0),
    ("U", 0),
    ("TV-Y", 0),
    ("TV-G", 0),
    ("E", 0),
    ("ALL", 0),
    ("TV-Y7", 7),
    ("TV-Y7-FV", 7),
    ("PG", 8),
    ("TV-PG", 8),
    ("12A", 12),
    ("PG-13", 13),
    ("TV-14", 14),
    ("M", 15),
    ("MA15+", 15),
    ("R", 17),
    ("TV-MA", 17),
    ("NC-17", 18),
    ("X", 18),
    ("R18", 18),
    ("R18+", 18),
    ("XXX", 18),
];

/// Highest rating when none has been chosen (matches the panel default).
const DEFAULT_MAX_RATING: &str = "PG";

/// Choices offered for "highest allowed rating", lowest first.
pub const MAX_RATING_CHOICES: &[&str] = &["G", "PG", "PG-13", "R", "NC-17"];

#[derive(Debug, Clone, PartialEq)]
pub struct Policy {
    /// Oldest certification age allowed; None means ratings are not checked.
    pub max_age: Option<u8>,
    pub blocked_genres: Vec<String>,
    pub block_adult: bool,
    /// With a max rating set, hide titles that have no known certification.
    pub block_unrated: bool,
}

impl Policy {
    pub fn from_config(config: &Map<String, Value>) -> Self {
        // Unset means the panel default (PG); an empty string means any rating.
        let max_age = match config.get("maxRating").and_then(Value::as_str) {
            Some(label) => rating_age(label),
            None => rating_age(DEFAULT_MAX_RATING),
        };
        let blocked_genres = config
            .get("blockedGenres")
            .and_then(Value::as_array)
            .map(|genres| {
                genres
                    .iter()
                    .filter_map(Value::as_str)
                    .map(|genre| genre.trim().to_string())
                    .filter(|genre| !genre.is_empty())
                    .collect()
            })
            .unwrap_or_default();
        Policy {
            max_age,
            blocked_genres,
            block_adult: config
                .get("blockAdult")
                .and_then(Value::as_bool)
                .unwrap_or(true),
            block_unrated: config
                .get("blockUnrated")
                .and_then(Value::as_bool)
                .unwrap_or(true),
        }
    }
}

/// Upper-cased label with country and "Rated" prefixes removed, or None when
/// the text is not a certification we can place (e.g. "NR", "Unrated").
pub fn normalize_certification(raw: &str) -> Option<String> {
    let mut text = raw.trim().to_ascii_uppercase();
    for prefix in ["RATED ", "RATING ", "CERTIFICATE "] {
        if let Some(rest) = text.strip_prefix(prefix) {
            text = rest.trim().to_string();
        }
    }
    // "US:PG-13", "GB:12A", "DE:FSK 12"
    if let Some((country, rest)) = text.split_once(':') {
        if country.len() <= 3 && country.chars().all(|c| c.is_ascii_alphabetic()) {
            text = rest.trim().to_string();
        }
    }
    for prefix in ["FSK ", "FSK", "AGE "] {
        if let Some(rest) = text.strip_prefix(prefix) {
            text = rest.trim().to_string();
        }
    }
    // "PG-13 for violence" -> "PG-13"
    let label = text.split_whitespace().next().unwrap_or("").to_string();
    rating_age(&label).map(|_| label)
}

/// Age a certification suits, or None when unknown.
pub fn rating_age(label: &str) -> Option<u8> {
    let label = label.trim().to_ascii_uppercase();
    if label.is_empty() {
        return None;
    }
    if let Some((_, age)) = RATING_AGES.iter().find(|(name, _)| *name == label) {
        return Some(*age);
    }
    let digits: String = label.chars().take_while(char::is_ascii_digit).collect();
    let rest = &label[digits.len()..];
    if !digits.is_empty() && (rest.is_empty() || rest == "+" || rest == "A") {
        return digits.parse::<u8>().ok().filter(|age| *age <= 21);
    }
    None
}

/// SQL expression for the age of `<alias>content_rating`, NULL when unknown.
/// Mirrors `rating_age` for normalised labels.
pub fn age_sql(alias: &str) -> String {
    let column = format!("upper(trim({alias}content_rating))");
    let mut sql = format!("(CASE {column}");
    for (label, age) in RATING_AGES {
        sql.push_str(&format!(" WHEN '{label}' THEN {age}"));
    }
    sql.push_str(&format!(
        " ELSE CASE WHEN {column} GLOB '[0-9]*' AND CAST({column} AS INTEGER) <= 21 \
         AND ltrim({column}, '0123456789') IN ('', '+', 'A') \
         THEN CAST({column} AS INTEGER) END END)"
    ));
    sql
}

fn sql_text(value: &str) -> String {
    format!("'{}'", value.replace('\'', "''"))
}

fn like_pattern(value: &str) -> String {
    let escaped = value
        .to_lowercase()
        .replace('\\', "\\\\")
        .replace('%', "\\%")
        .replace('_', "\\_");
    sql_text(&format!("%{escaped}%"))
}

/// Predicate that keeps only titles `policy` allows. `alias` is "" or "m.".
pub fn visibility_sql(policy: &Policy, alias: &str) -> String {
    let mut blocked = Vec::new();
    if policy.block_adult {
        blocked.push(format!("lower(trim({alias}media_type)) = 'adult'"));
    }
    for genre in &policy.blocked_genres {
        blocked.push(format!(
            "lower(coalesce({alias}genre, '')) LIKE {} ESCAPE '\\'",
            like_pattern(genre)
        ));
    }
    if let Some(max_age) = policy.max_age {
        let age = age_sql(alias);
        blocked.push(format!("coalesce({age}, -1) > {max_age}"));
        if policy.block_unrated {
            blocked.push(format!("{age} IS NULL"));
        }
    }
    if blocked.is_empty() {
        "1 = 1".into()
    } else {
        format!("NOT ({})", blocked.join(" OR "))
    }
}

/// Policy for `profile_id`, or None when it may see everything.
pub fn policy_for_profile(db: &Database, profile_id: i64) -> Option<Policy> {
    if !feature_flags::is_enabled(db, "parental_ctrl") {
        return None;
    }
    let restricted = user_data::get_profile(db, profile_id)
        .ok()
        .flatten()
        .map(|profile| profile.restricted)
        .unwrap_or(false);
    restricted.then(|| Policy::from_config(&feature_flags::config(db, SWITCH)))
}

/// Visibility predicate (unaliased columns) for whoever is watching now.
pub fn active_filter(db: &Database) -> Option<String> {
    policy_for_profile(db, user_data::active_profile_id(db))
        .map(|policy| visibility_sql(&policy, ""))
}

/// Same, for queries that alias media_items as `m`.
pub fn filter_for_profile(db: &Database, profile_id: i64) -> Option<String> {
    policy_for_profile(db, profile_id).map(|policy| visibility_sql(&policy, "m."))
}

// ------------------------------------------------------------------ PIN ----

pub fn ensure_tables(db: &Database) -> rusqlite::Result<()> {
    db.conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS parental_pin (
            id INTEGER PRIMARY KEY CHECK (id = 1),
            pin_hash TEXT NOT NULL,
            updated_at TEXT NOT NULL
        );",
    )
}

fn stored_pin_hash(db: &Database) -> Option<String> {
    ensure_tables(db).ok()?;
    db.conn
        .query_row(
            "SELECT pin_hash FROM parental_pin WHERE id = 1",
            [],
            |row| row.get::<_, String>(0),
        )
        .optional()
        .ok()
        .flatten()
}

pub fn has_pin(db: &Database) -> bool {
    stored_pin_hash(db).is_some()
}

fn validate_pin(pin: &str) -> Result<(), String> {
    if (4..=8).contains(&pin.len()) && pin.chars().all(|c| c.is_ascii_digit()) {
        Ok(())
    } else {
        Err("The PIN must be 4 to 8 digits".into())
    }
}

fn hash_pin(pin: &str) -> Result<String, String> {
    let salt = SaltString::encode_b64(uuid::Uuid::new_v4().as_bytes())
        .map_err(|error| format!("Unable to salt the PIN: {error}"))?;
    Argon2::default()
        .hash_password(pin.as_bytes(), &salt)
        .map(|hash| hash.to_string())
        .map_err(|error| format!("Unable to hash the PIN: {error}"))
}

fn pin_matches(pin: &str, hash: &str) -> bool {
    PasswordHash::new(hash)
        .ok()
        .map(|parsed| {
            Argon2::default()
                .verify_password(pin.as_bytes(), &parsed)
                .is_ok()
        })
        .unwrap_or(false)
}

/// Unlock window and failed-attempt lockout.
#[derive(Debug, Default)]
pub struct PinGate {
    unlocked_until: Option<Instant>,
    failures: u32,
    locked_out_until: Option<Instant>,
}

impl PinGate {
    pub fn is_unlocked(&self, now: Instant) -> bool {
        self.unlocked_until.is_some_and(|until| now < until)
    }

    pub fn lock(&mut self) {
        self.unlocked_until = None;
    }

    /// Checks `pin`; a match opens the unlock window, misses count towards a lockout.
    pub fn verify(&mut self, db: &Database, pin: &str, now: Instant) -> Result<(), String> {
        if let Some(until) = self.locked_out_until {
            if now < until {
                let wait = until.duration_since(now).as_secs().max(1);
                return Err(format!("Too many wrong PINs. Try again in {wait} seconds"));
            }
            self.locked_out_until = None;
        }
        let Some(hash) = stored_pin_hash(db) else {
            return Ok(());
        };
        if pin_matches(pin.trim(), &hash) {
            self.failures = 0;
            self.unlocked_until = Some(now + UNLOCK_WINDOW);
            Ok(())
        } else {
            self.failures += 1;
            if self.failures >= MAX_FAILURES {
                self.failures = 0;
                self.locked_out_until = Some(now + LOCKOUT);
            }
            Err("Wrong PIN".into())
        }
    }
}

fn gate() -> &'static Mutex<PinGate> {
    static GATE: OnceLock<Mutex<PinGate>> = OnceLock::new();
    GATE.get_or_init(|| Mutex::new(PinGate::default()))
}

/// Whether loosening the rules needs the PIN right now.
pub fn pin_required(db: &Database) -> bool {
    feature_flags::is_enabled(db, SWITCH) && has_pin(db)
}

/// Ok when the PIN is not needed, the gate is unlocked, or `pin` matches.
fn require_pin_with(
    db: &Database,
    gate: &mut PinGate,
    pin: Option<&str>,
    action: &str,
) -> Result<(), String> {
    if !pin_required(db) || gate.is_unlocked(Instant::now()) {
        return Ok(());
    }
    match pin {
        Some(pin) if !pin.trim().is_empty() => {
            gate.verify(db, pin, Instant::now()).inspect_err(|_| {
                user_data::emit(db, "parental.pin_failed", action, json!({}));
            })
        }
        _ => Err(format!("PIN required: enter the parental PIN to {action}")),
    }
}

pub fn require_pin(db: &Database, pin: Option<&str>, action: &str) -> Result<(), String> {
    let mut gate = gate().lock().map_err(|error| error.to_string())?;
    require_pin_with(db, &mut gate, pin, action)
}

fn active_is_restricted(db: &Database) -> bool {
    user_data::active_profile(db)
        .map(|profile| profile.restricted)
        .unwrap_or(false)
}

/// Called before any Feature Matrix switch is saved.
pub fn guard_switch_change(
    db: &Database,
    key: &str,
    enabled: bool,
    config: &str,
) -> Result<(), String> {
    match key {
        SWITCH if feature_flags::is_enabled(db, SWITCH) => {
            let current = Value::Object(feature_flags::config(db, SWITCH));
            let next = serde_json::from_str::<Value>(config).unwrap_or(Value::Null);
            if !enabled {
                require_pin(db, None, "turn off parental controls")
            } else if !same_rules(&current, &next) {
                require_pin(db, None, "change parental control rules")
            } else {
                Ok(())
            }
        }
        "user_profiles"
            if !enabled
                && feature_flags::is_enabled(db, key)
                && feature_flags::is_enabled(db, SWITCH)
                && active_is_restricted(db) =>
        {
            require_pin(
                db,
                None,
                "turn off profiles while a restricted profile is active",
            )
        }
        _ => Ok(()),
    }
}

/// Configs are equal for guard purposes when every rule field matches.
fn same_rules(current: &Value, next: &Value) -> bool {
    [
        "maxRating",
        "blockedGenres",
        "blockAdult",
        "blockUnrated",
        "region",
    ]
    .iter()
    .all(|field| current.get(field) == next.get(field))
}

/// Switching from a restricted profile to an unrestricted one needs the PIN.
pub fn guard_profile_switch(
    db: &Database,
    target_id: i64,
    pin: Option<&str>,
) -> Result<(), String> {
    let target_restricted = user_data::get_profile(db, target_id)
        .ok()
        .flatten()
        .map(|profile| profile.restricted)
        .unwrap_or(false);
    if feature_flags::is_enabled(db, SWITCH) && active_is_restricted(db) && !target_restricted {
        require_pin(db, pin, "leave a restricted profile")
    } else {
        Ok(())
    }
}

/// Lifting a profile's restriction, or deleting a restricted profile, needs the PIN.
pub fn guard_profile_change(
    db: &Database,
    id: i64,
    restricted_after: Option<bool>,
) -> Result<(), String> {
    let restricted_now = user_data::get_profile(db, id)
        .ok()
        .flatten()
        .map(|profile| profile.restricted)
        .unwrap_or(false);
    if restricted_now && restricted_after != Some(true) {
        require_pin(db, None, "change a restricted profile")
    } else {
        Ok(())
    }
}

/// Sets the PIN. Replacing an existing PIN needs the current one.
pub fn set_pin(db: &Database, current: Option<&str>, new_pin: &str) -> Result<(), String> {
    validate_pin(new_pin.trim())?;
    ensure_tables(db).map_err(|error| error.to_string())?;
    if let Some(hash) = stored_pin_hash(db) {
        let gate = gate().lock().map_err(|error| error.to_string())?;
        let unlocked = gate.is_unlocked(Instant::now());
        let current_ok = current.is_some_and(|pin| pin_matches(pin.trim(), &hash));
        if !unlocked && !current_ok {
            return Err("Enter the current PIN to change it".into());
        }
    }
    let hash = hash_pin(new_pin.trim())?;
    db.conn
        .execute(
            "INSERT INTO parental_pin (id, pin_hash, updated_at) VALUES (1, ?1, ?2)
             ON CONFLICT(id) DO UPDATE SET pin_hash = excluded.pin_hash, updated_at = excluded.updated_at",
            params![hash, chrono::Utc::now().to_rfc3339()],
        )
        .map_err(|error| error.to_string())?;
    Ok(())
}

// ------------------------------------------------------------- commands ----

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ParentalStatus {
    pub enabled: bool,
    pub has_pin: bool,
    pub unlocked: bool,
    pub active_restricted: bool,
    pub rated_items: i64,
    pub unrated_items: i64,
    pub max_rating_choices: Vec<&'static str>,
}

fn status_of(db: &Database) -> Result<ParentalStatus, String> {
    let count = |sql: &str| {
        db.conn
            .query_row(sql, [], |row| row.get::<_, i64>(0))
            .map_err(|error| error.to_string())
    };
    Ok(ParentalStatus {
        enabled: feature_flags::is_enabled(db, SWITCH),
        has_pin: has_pin(db),
        unlocked: gate()
            .lock()
            .map(|gate| gate.is_unlocked(Instant::now()))
            .unwrap_or(false),
        active_restricted: active_is_restricted(db),
        rated_items: count(
            "SELECT COUNT(*) FROM media_items WHERE coalesce(content_rating, '') <> ''",
        )?,
        unrated_items: count(
            "SELECT COUNT(*) FROM media_items WHERE coalesce(content_rating, '') = '' AND media_type <> 'photo'",
        )?,
        max_rating_choices: MAX_RATING_CHOICES.to_vec(),
    })
}

fn with_db<T>(
    state: &State<AppState>,
    f: impl FnOnce(&Database) -> Result<T, String>,
) -> Result<T, String> {
    let db = state.db.lock().map_err(|error| error.to_string())?;
    user_data::ensure_tables(&db).map_err(|error| error.to_string())?;
    ensure_tables(&db).map_err(|error| error.to_string())?;
    f(&db)
}

#[tauri::command]
pub fn parental_status(state: State<AppState>) -> Result<ParentalStatus, String> {
    with_db(&state, status_of)
}

#[tauri::command]
pub fn parental_set_pin(
    state: State<AppState>,
    current_pin: Option<String>,
    new_pin: String,
) -> Result<ParentalStatus, String> {
    with_db(&state, |db| {
        set_pin(db, current_pin.as_deref(), &new_pin)?;
        user_data::emit(db, "parental.pin_set", "Parental PIN updated", json!({}));
        status_of(db)
    })
}

#[tauri::command]
pub fn parental_unlock(state: State<AppState>, pin: String) -> Result<ParentalStatus, String> {
    with_db(&state, |db| {
        let result =
            gate()
                .lock()
                .map_err(|error| error.to_string())?
                .verify(db, &pin, Instant::now());
        match result {
            Ok(()) => {
                user_data::emit(
                    db,
                    "parental.unlocked",
                    "Parental controls unlocked",
                    json!({}),
                );
                status_of(db)
            }
            Err(error) => {
                user_data::emit(db, "parental.pin_failed", "unlock", json!({}));
                Err(error)
            }
        }
    })
}

#[tauri::command]
pub fn parental_lock(state: State<AppState>) -> Result<ParentalStatus, String> {
    with_db(&state, |db| {
        gate().lock().map_err(|error| error.to_string())?.lock();
        status_of(db)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_db() -> (Database, std::path::PathBuf) {
        let path =
            std::env::temp_dir().join(format!("cinavault-parental-{}.db", uuid::Uuid::new_v4()));
        let db = Database::new(path.to_str().unwrap()).unwrap();
        user_data::ensure_tables(&db).unwrap();
        ensure_tables(&db).unwrap();
        (db, path)
    }

    fn add(db: &Database, title: &str, media_type: &str, genre: &str, rating: Option<&str>) {
        db.conn
            .execute(
                "INSERT INTO media_items (title, file_path, media_type, genre, content_rating, date_added)
                 VALUES (?1, ?2, ?3, ?4, ?5, '2026-01-01')",
                params![title, format!("C:/m/{title}.mkv"), media_type, genre, rating],
            )
            .unwrap();
    }

    /// Whether the media item may be shown to the active profile.
    fn item_visible(db: &Database, media_id: i64) -> bool {
        let Some(filter) = active_filter(db) else {
            return true;
        };
        db.conn
            .query_row(
                &format!("SELECT COUNT(*) FROM media_items WHERE id = ?1 AND {filter}"),
                params![media_id],
                |row| row.get::<_, i64>(0),
            )
            .map(|count| count > 0)
            .unwrap_or(false)
    }

    fn visible_titles(db: &Database, policy: &Policy) -> Vec<String> {
        let sql = format!(
            "SELECT title FROM media_items WHERE {} ORDER BY title",
            visibility_sql(policy, "")
        );
        let mut stmt = db.conn.prepare(&sql).unwrap();
        let rows = stmt.query_map([], |row| row.get::<_, String>(0)).unwrap();
        rows.map(Result::unwrap).collect()
    }

    #[test]
    fn certifications_normalise_from_nfo_and_provider_forms() {
        assert_eq!(
            normalize_certification("Rated PG-13").as_deref(),
            Some("PG-13")
        );
        assert_eq!(normalize_certification("US:R").as_deref(), Some("R"));
        assert_eq!(normalize_certification("gb:12a").as_deref(), Some("12A"));
        assert_eq!(normalize_certification("DE:FSK 16").as_deref(), Some("16"));
        assert_eq!(normalize_certification("tv-ma").as_deref(), Some("TV-MA"));
        assert_eq!(
            normalize_certification("PG-13 for violence").as_deref(),
            Some("PG-13")
        );
        assert_eq!(normalize_certification("NR"), None);
        assert_eq!(normalize_certification("Unrated"), None);
        assert_eq!(normalize_certification(""), None);
        assert_eq!(rating_age("PG-13"), Some(13));
        assert_eq!(rating_age("18+"), Some(18));
        assert_eq!(rating_age("99"), None);
    }

    #[test]
    fn sql_age_matches_rust_age_for_every_label() {
        let (db, path) = temp_db();
        let mut labels: Vec<String> = RATING_AGES.iter().map(|(l, _)| l.to_string()).collect();
        labels.extend(["12", "16", "18+", "12A", "7", "NR", "ABC", "99", "1X"].map(String::from));
        for label in labels {
            let sql_age: Option<i64> = db
                .conn
                .query_row(
                    &format!("SELECT {} FROM (SELECT ?1 AS content_rating)", age_sql("")),
                    params![label],
                    |row| row.get(0),
                )
                .unwrap();
            assert_eq!(
                sql_age,
                rating_age(&label).map(i64::from),
                "SQL and Rust disagree on {label}"
            );
        }
        std::fs::remove_file(path).ok();
    }

    #[test]
    fn policy_hides_adult_blocked_genres_and_titles_over_the_limit() {
        let (db, path) = temp_db();
        add(&db, "Cartoon", "movie", "Animation, Family", Some("G"));
        add(&db, "Teen Drama", "tv", "Drama", Some("TV-14"));
        add(&db, "Scary", "movie", "Horror, Thriller", Some("PG-13"));
        add(&db, "Gritty", "movie", "Crime", Some("R"));
        add(&db, "Mystery Box", "movie", "Drama", None);
        add(&db, "Late Night", "adult", "", Some("G"));
        add(&db, "100% Fun_Run", "movie", "Comedy", Some("PG"));

        let config: Map<String, Value> = serde_json::from_value(json!({
            "maxRating": "PG-13",
            "blockedGenres": ["horror"],
            "blockAdult": true,
            "blockUnrated": true
        }))
        .unwrap();
        let policy = Policy::from_config(&config);
        assert_eq!(policy.max_age, Some(13));
        assert_eq!(
            visible_titles(&db, &policy),
            vec!["100% Fun_Run", "Cartoon"]
        );

        let lenient = Policy {
            max_age: Some(14),
            blocked_genres: vec!["it's".into(), "%".into()],
            block_adult: false,
            block_unrated: false,
        };
        // Quotes and LIKE wildcards in a genre are literal text, never SQL.
        assert_eq!(
            visible_titles(&db, &lenient),
            vec![
                "100% Fun_Run",
                "Cartoon",
                "Late Night",
                "Mystery Box",
                "Scary",
                "Teen Drama"
            ]
        );
        let open = Policy {
            max_age: None,
            blocked_genres: vec![],
            block_adult: false,
            block_unrated: true,
        };
        assert_eq!(visibility_sql(&open, "m."), "1 = 1");
        assert_eq!(visible_titles(&db, &open).len(), 7);
        std::fs::remove_file(path).ok();
    }

    #[test]
    fn only_restricted_profiles_with_the_switch_on_are_filtered() {
        let (db, path) = temp_db();
        add(&db, "Gritty", "movie", "Crime", Some("R"));
        let kid = user_data::create_profile(&db, "Kid", "#f00", true).unwrap();
        db.set_feature_setting_data("user_profiles", true, "{}")
            .unwrap();
        user_data::set_active_profile(&db, kid.id).unwrap();
        assert!(active_filter(&db).is_none(), "switch defaults off");
        db.set_feature_setting_data(SWITCH, true, r#"{"maxRating":"PG"}"#)
            .unwrap();
        let filter = active_filter(&db).expect("restricted profile is filtered");
        assert!(filter.contains("> 8"));
        assert!(filter_for_profile(&db, user_data::DEFAULT_PROFILE_ID).is_none());
        let id: i64 = db
            .conn
            .query_row("SELECT id FROM media_items", [], |row| row.get(0))
            .unwrap();
        assert!(!item_visible(&db, id));
        user_data::set_active_profile(&db, user_data::DEFAULT_PROFILE_ID).unwrap();
        assert!(item_visible(&db, id));
        std::fs::remove_file(path).ok();
    }

    #[test]
    fn pin_is_hashed_and_wrong_pins_lock_out() {
        let (db, path) = temp_db();
        assert!(validate_pin("12a4").is_err());
        assert!(validate_pin("123").is_err());
        set_pin(&db, None, "4321").unwrap();
        let stored = stored_pin_hash(&db).unwrap();
        assert!(stored.starts_with("$argon2"));
        assert!(!stored.contains("4321"));

        let mut gate = PinGate::default();
        let start = Instant::now();
        assert!(!gate.is_unlocked(start));
        assert_eq!(gate.verify(&db, "0000", start), Err("Wrong PIN".into()));
        gate.verify(&db, "4321", start).unwrap();
        assert!(gate.is_unlocked(start + Duration::from_secs(60)));
        assert!(!gate.is_unlocked(start + UNLOCK_WINDOW + Duration::from_secs(1)));
        gate.lock();
        for _ in 0..MAX_FAILURES {
            assert!(gate.verify(&db, "1111", start).is_err());
        }
        let locked = gate.verify(&db, "4321", start + Duration::from_secs(5));
        assert!(locked.unwrap_err().starts_with("Too many wrong PINs"));
        gate.verify(&db, "4321", start + LOCKOUT + Duration::from_secs(1))
            .unwrap();
        std::fs::remove_file(path).ok();
    }

    #[test]
    fn leaving_a_restricted_profile_and_loosening_rules_need_the_pin() {
        let (db, path) = temp_db();
        let kid = user_data::create_profile(&db, "Kid", "#f00", true).unwrap();
        db.set_feature_setting_data("user_profiles", true, "{}")
            .unwrap();
        db.set_feature_setting_data(SWITCH, true, r#"{"maxRating":"PG"}"#)
            .unwrap();
        user_data::set_active_profile(&db, kid.id).unwrap();
        // Without a PIN nothing is guarded yet.
        assert!(guard_profile_switch(&db, user_data::DEFAULT_PROFILE_ID, None).is_ok());
        set_pin(&db, None, "2468").unwrap();

        let mut gate = PinGate::default();
        let err = require_pin_with(&db, &mut gate, None, "leave a restricted profile").unwrap_err();
        assert!(err.starts_with("PIN required"));
        assert!(require_pin_with(&db, &mut gate, Some("1357"), "x").is_err());
        assert!(require_pin_with(&db, &mut gate, Some("2468"), "x").is_ok());
        assert!(gate.is_unlocked(Instant::now()));

        assert!(same_rules(
            &json!({"maxRating": "PG", "note": 1}),
            &json!({"maxRating": "PG", "note": 2})
        ));
        assert!(!same_rules(
            &json!({"maxRating": "PG"}),
            &json!({"maxRating": "R"})
        ));
        // Re-saving the same rules (e.g. toggling a panel open) never needs the PIN.
        assert!(guard_switch_change(&db, SWITCH, true, r#"{"maxRating":"PG"}"#).is_ok());
        // Switching between two restricted profiles is fine.
        let kid2 = user_data::create_profile(&db, "Kid 2", "#0f0", true).unwrap();
        assert!(guard_profile_switch(&db, kid2.id, None).is_ok());
        // Unrelated switches are never guarded.
        assert!(guard_switch_change(&db, "cdn_cache", false, "{}").is_ok());
        std::fs::remove_file(path).ok();
    }
}
