//! Programme guide (EPG) for Live TV.
//!
//! Guides come from XMLTV sources: URLs the user adds, the `url-tvg` header of
//! an M3U playlist, or an Xtream Codes provider's `xmltv.php`. A sync downloads
//! the file (plain or gzipped) to a temp file, parses it off the async runtime,
//! keeps only programmes inside a rolling window, and replaces that source's
//! rows in one transaction. Channels match a guide by their `tvg-id` /
//! `epg_channel_id` first, then by a normalised display name.

use crate::db::Database;
use crate::AppState;
use flate2::read::GzDecoder;
use quick_xml::events::{BytesStart, Event};
use quick_xml::Reader;
use rusqlite::{params, OptionalExtension};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::io::{BufRead, BufReader, Read, Seek, SeekFrom, Write};
use std::time::Duration;
use tauri::State;

/// Programmes that ended more than this long ago are not kept.
const KEEP_PAST_SECS: i64 = 6 * 3600;
/// Programmes starting further ahead than this are not kept.
const KEEP_AHEAD_SECS: i64 = 4 * 86_400;
/// Sources older than this are refreshed when the Live TV tab opens.
const STALE_AFTER_SECS: i64 = 12 * 3600;
/// Largest guide file accepted (compressed size on the wire).
const MAX_DOWNLOAD_BYTES: u64 = 512 * 1024 * 1024;
/// Largest guide accepted after decompression, so a gzip bomb cannot run away.
const MAX_GUIDE_XML_BYTES: u64 = 4 * 1024 * 1024 * 1024;
/// Most programmes kept in memory from one guide (inside the rolling window).
const MAX_PROGRAMMES: usize = 2_000_000;
/// Most channels kept from one guide.
const MAX_CHANNELS: usize = 200_000;

/// Reader that fails, rather than truncating, once `remaining` bytes are used.
struct CappedReader<R> {
    inner: R,
    remaining: u64,
}

impl<R: Read> Read for CappedReader<R> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        if self.remaining == 0 {
            return Err(std::io::Error::other(
                "The guide is larger than 4 GB once unpacked",
            ));
        }
        let max = buf
            .len()
            .min(usize::try_from(self.remaining).unwrap_or(usize::MAX));
        let read = self.inner.read(&mut buf[..max])?;
        self.remaining -= read as u64;
        Ok(read)
    }
}

pub fn ensure_tables(db: &Database) -> Result<(), String> {
    db.conn
        .execute_batch(
            "CREATE TABLE IF NOT EXISTS epg_sources (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                name TEXT NOT NULL,
                url TEXT NOT NULL UNIQUE,
                profile_id INTEGER,
                enabled INTEGER NOT NULL DEFAULT 1,
                last_synced TEXT,
                last_error TEXT,
                channel_count INTEGER NOT NULL DEFAULT 0,
                programme_count INTEGER NOT NULL DEFAULT 0
            );
            CREATE TABLE IF NOT EXISTS epg_source_profiles (
                source_id INTEGER NOT NULL,
                profile_id INTEGER NOT NULL,
                PRIMARY KEY (source_id, profile_id)
            );
            CREATE TABLE IF NOT EXISTS epg_channels (
                source_id INTEGER NOT NULL,
                channel_id TEXT NOT NULL,
                display_name TEXT NOT NULL,
                name_key TEXT NOT NULL,
                PRIMARY KEY (source_id, channel_id)
            );
            CREATE INDEX IF NOT EXISTS idx_epg_channels_name ON epg_channels(name_key);
            CREATE TABLE IF NOT EXISTS epg_programmes (
                source_id INTEGER NOT NULL,
                channel_key TEXT NOT NULL,
                start_ts INTEGER NOT NULL,
                stop_ts INTEGER NOT NULL,
                title TEXT NOT NULL,
                sub_title TEXT,
                description TEXT,
                category TEXT,
                episode TEXT
            );
            CREATE INDEX IF NOT EXISTS idx_epg_programmes_channel
                ON epg_programmes(channel_key, stop_ts);",
        )
        .map_err(|error| error.to_string())
}

// ── XMLTV parsing ────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub struct XmltvChannel {
    pub id: String,
    pub display_name: String,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct XmltvProgramme {
    pub channel: String,
    pub start: i64,
    pub stop: i64,
    pub title: String,
    pub sub_title: Option<String>,
    pub description: Option<String>,
    pub category: Option<String>,
    pub episode: Option<String>,
}

/// Parses XMLTV times such as `20261011140000 +0100`, `202610111400` or
/// `20261011140000` (no offset means UTC) into Unix seconds.
pub fn parse_xmltv_time(value: &str) -> Option<i64> {
    let value = value.trim();
    let (stamp, offset) = match value.split_once(char::is_whitespace) {
        Some((stamp, offset)) => (stamp, offset.trim()),
        None => (value, ""),
    };
    let digits: String = stamp.chars().take_while(|c| c.is_ascii_digit()).collect();
    if digits.len() < 12 {
        return None;
    }
    let field = |range: std::ops::Range<usize>| digits.get(range)?.parse::<u32>().ok();
    let date = chrono::NaiveDate::from_ymd_opt(field(0..4)? as i32, field(4..6)?, field(6..8)?)?;
    let seconds = if digits.len() >= 14 {
        field(12..14)?
    } else {
        0
    };
    let time = chrono::NaiveTime::from_hms_opt(field(8..10)?, field(10..12)?, seconds)?;
    let local = date.and_time(time).and_utc().timestamp();

    let offset_secs = if offset.len() >= 5 && (offset.starts_with('+') || offset.starts_with('-')) {
        let hours: i64 = offset.get(1..3)?.parse().ok()?;
        let minutes: i64 = offset.get(3..5)?.parse().ok()?;
        let total = hours * 3600 + minutes * 60;
        if offset.starts_with('-') {
            -total
        } else {
            total
        }
    } else {
        0
    };
    Some(local - offset_secs)
}

/// Lowercases a channel name and drops country tags, quality tags and
/// punctuation, so "UK: BBC One HD" and "BBC One" share a key.
pub fn channel_name_key(name: &str) -> String {
    let lower = name.to_lowercase();
    let mut text = lower.trim();
    // Leading "US:", "UK |", "[DE]", "FR -" style tags.
    if let Some(rest) = text.strip_prefix('[').and_then(|rest| rest.split_once(']')) {
        text = rest.1.trim();
    } else if let Some(position) = text.find([':', '|']) {
        if position <= 4 {
            text = text[position + 1..].trim();
        }
    } else if let Some((tag, rest)) = text.split_once(" - ") {
        if tag.len() <= 3 && tag.chars().all(|c| c.is_ascii_alphabetic()) {
            text = rest.trim();
        }
    }
    const QUALITY: [&str; 9] = [
        "hd", "fhd", "uhd", "sd", "4k", "hevc", "h265", "1080p", "720p",
    ];
    text.split(|c: char| !c.is_alphanumeric())
        .filter(|word| !word.is_empty() && !QUALITY.contains(word))
        .collect::<Vec<_>>()
        .concat()
}

fn attribute(start: &BytesStart, key: &[u8]) -> Option<String> {
    start
        .attributes()
        .flatten()
        .find(|attr| attr.key.local_name().as_ref() == key)
        .and_then(|attr| {
            attr.normalized_value(quick_xml::XmlVersion::Implicit1_0)
                .ok()
                .map(|value| value.trim().to_string())
        })
        .filter(|value| !value.is_empty())
}

fn non_empty(value: String) -> Option<String> {
    let trimmed = value.trim();
    (!trimmed.is_empty()).then(|| trimmed.to_string())
}

/// Streams an XMLTV document, calling `on_channel` and `on_programme` as each
/// element closes. Programmes whose times cannot be read are skipped.
pub fn parse_xmltv<R: BufRead>(
    input: R,
    mut on_channel: impl FnMut(XmltvChannel),
    mut on_programme: impl FnMut(XmltvProgramme),
) -> Result<(), String> {
    let mut reader = Reader::from_reader(input);
    reader.config_mut().check_end_names = false;

    let mut buf = Vec::new();
    let mut text = String::new();
    let mut channel: Option<(String, Option<String>)> = None;
    let mut programme: Option<XmltvProgramme> = None;
    let mut episode_system: Option<String> = None;

    loop {
        let event = reader
            .read_event_into(&mut buf)
            .map_err(|error| format!("Guide is not valid XMLTV: {error}"))?;
        match event {
            Event::Start(start) => {
                text.clear();
                match start.local_name().as_ref() {
                    b"channel" => {
                        channel = attribute(&start, b"id").map(|id| (id, None));
                    }
                    b"programme" => {
                        let times = (
                            attribute(&start, b"start")
                                .as_deref()
                                .and_then(parse_xmltv_time),
                            attribute(&start, b"stop")
                                .as_deref()
                                .and_then(parse_xmltv_time),
                            attribute(&start, b"channel"),
                        );
                        programme = match times {
                            (Some(start), Some(stop), Some(channel)) if stop > start => {
                                Some(XmltvProgramme {
                                    channel,
                                    start,
                                    stop,
                                    title: String::new(),
                                    sub_title: None,
                                    description: None,
                                    category: None,
                                    episode: None,
                                })
                            }
                            _ => None,
                        };
                    }
                    b"episode-num" => episode_system = attribute(&start, b"system"),
                    _ => {}
                }
            }
            Event::Text(chunk) => {
                if let Ok(decoded) = chunk.decode() {
                    text.push_str(&decoded);
                }
            }
            Event::CData(chunk) => {
                if let Ok(decoded) = chunk.decode() {
                    text.push_str(&decoded);
                }
            }
            Event::GeneralRef(reference) => {
                if let Ok(Some(character)) = reference.resolve_char_ref() {
                    text.push(character);
                } else if let Ok(name) = reference.decode() {
                    let entity = format!("&{name};");
                    match quick_xml::escape::unescape(&entity) {
                        Ok(resolved) => text.push_str(&resolved),
                        Err(_) => text.push_str(&entity),
                    }
                }
            }
            Event::End(end) => {
                let value = std::mem::take(&mut text);
                match end.local_name().as_ref() {
                    b"display-name" => {
                        if let Some((_, name)) = channel.as_mut() {
                            if name.is_none() {
                                *name = non_empty(value);
                            }
                        }
                    }
                    b"channel" => {
                        if let Some((id, name)) = channel.take() {
                            let display_name = name.unwrap_or_else(|| id.clone());
                            on_channel(XmltvChannel { id, display_name });
                        }
                    }
                    b"title" => {
                        if let Some(item) = programme.as_mut() {
                            if item.title.is_empty() {
                                item.title = value.trim().to_string();
                            }
                        }
                    }
                    b"sub-title" => {
                        if let Some(item) = programme.as_mut() {
                            item.sub_title = item.sub_title.take().or_else(|| non_empty(value));
                        }
                    }
                    b"desc" => {
                        if let Some(item) = programme.as_mut() {
                            item.description = item.description.take().or_else(|| non_empty(value));
                        }
                    }
                    b"category" => {
                        if let Some(item) = programme.as_mut() {
                            item.category = item.category.take().or_else(|| non_empty(value));
                        }
                    }
                    b"episode-num" => {
                        if let Some(item) = programme.as_mut() {
                            let system = episode_system.take();
                            let episode = match system.as_deref() {
                                Some("xmltv_ns") => format_xmltv_ns(&value),
                                _ => non_empty(value),
                            };
                            // Prefer the readable onscreen form over xmltv_ns.
                            if item.episode.is_none() || system.as_deref() == Some("onscreen") {
                                item.episode = episode.or(item.episode.take());
                            }
                        }
                    }
                    b"programme" => {
                        if let Some(mut item) = programme.take() {
                            if item.title.is_empty() {
                                item.title = "Untitled".into();
                            }
                            on_programme(item);
                        }
                    }
                    _ => {}
                }
            }
            Event::Eof => break,
            _ => {}
        }
        buf.clear();
    }
    Ok(())
}

/// Turns xmltv_ns "1.4.0/1" (zero-based) into "S2 E5".
fn format_xmltv_ns(value: &str) -> Option<String> {
    let mut parts = value.split('.');
    let number = |part: Option<&str>| {
        part.and_then(|part| part.split('/').next())
            .map(str::trim)
            .filter(|part| !part.is_empty())
            .and_then(|part| part.parse::<u32>().ok())
            .map(|n| n + 1)
    };
    match (number(parts.next()), number(parts.next())) {
        (Some(season), Some(episode)) => Some(format!("S{season} E{episode}")),
        (None, Some(episode)) => Some(format!("E{episode}")),
        (Some(season), None) => Some(format!("S{season}")),
        (None, None) => None,
    }
}

// ── Storage ──────────────────────────────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct EpgSource {
    pub id: i64,
    pub name: String,
    /// Shown to the user; credentials in provider URLs are masked.
    pub url: String,
    pub profile_id: Option<i64>,
    pub enabled: bool,
    pub last_synced: Option<String>,
    pub last_error: Option<String>,
    pub channel_count: i64,
    pub programme_count: i64,
}

/// Hides `password=` / `username=` values so provider guide URLs can be listed.
pub fn mask_credentials(url: &str) -> String {
    let Ok(mut parsed) = reqwest::Url::parse(url) else {
        return url.to_string();
    };
    let pairs: Vec<(String, String)> = parsed
        .query_pairs()
        .map(|(key, value)| {
            let hidden = matches!(
                key.to_ascii_lowercase().as_str(),
                "password" | "username" | "pass" | "user" | "token"
            );
            (
                key.into_owned(),
                if hidden {
                    "•••".to_string()
                } else {
                    value.into_owned()
                },
            )
        })
        .collect();
    if !pairs.is_empty() {
        parsed.query_pairs_mut().clear().extend_pairs(pairs);
    }
    if parsed.password().is_some() {
        let _ = parsed.set_password(Some("•••"));
    }
    if !parsed.username().is_empty() {
        let _ = parsed.set_username("•••");
    }
    parsed.to_string()
}

pub fn validate_source_url(url: &str) -> Result<String, String> {
    let trimmed = url.trim();
    let parsed = reqwest::Url::parse(trimmed)
        .map_err(|_| "Enter a full guide URL starting with http:// or https://".to_string())?;
    match parsed.scheme() {
        "http" | "https" if parsed.host_str().is_some() => Ok(trimmed.to_string()),
        _ => Err("Guide URLs must start with http:// or https://".into()),
    }
}

/// Adds a source or returns the id of the existing one with the same URL.
pub fn upsert_source(
    db: &Database,
    name: &str,
    url: &str,
    profile_id: Option<i64>,
) -> Result<i64, String> {
    let url = validate_source_url(url)?;
    let name = if name.trim().is_empty() {
        reqwest::Url::parse(&url)
            .ok()
            .and_then(|parsed| parsed.host_str().map(str::to_string))
            .unwrap_or_else(|| "Guide".into())
    } else {
        name.trim().to_string()
    };
    if let Some(id) = db
        .conn
        .query_row(
            "SELECT id FROM epg_sources WHERE url = ?1",
            params![url],
            |row| row.get(0),
        )
        .optional()
        .map_err(|error| error.to_string())?
    {
        // A guide added by hand first, or named by several playlists, links to
        // every playlist that names it.
        if let Some(profile_id) = profile_id {
            db.conn
                .execute(
                    "UPDATE epg_sources SET profile_id = COALESCE(profile_id, ?2) WHERE id = ?1",
                    params![id, profile_id],
                )
                .map_err(|error| error.to_string())?;
            link_profile(db, id, profile_id)?;
        }
        return Ok(id);
    }
    db.conn
        .execute(
            "INSERT INTO epg_sources (name, url, profile_id) VALUES (?1, ?2, ?3)",
            params![name, url, profile_id],
        )
        .map_err(|error| error.to_string())?;
    let id = db.conn.last_insert_rowid();
    if let Some(profile_id) = profile_id {
        link_profile(db, id, profile_id)?;
    }
    Ok(id)
}

fn link_profile(db: &Database, source_id: i64, profile_id: i64) -> Result<(), String> {
    db.conn
        .execute(
            "INSERT OR IGNORE INTO epg_source_profiles (source_id, profile_id) VALUES (?1, ?2)",
            params![source_id, profile_id],
        )
        .map(|_| ())
        .map_err(|error| error.to_string())
}

/// Guide sources named by a playlist or Xtream profile.
pub fn sources_for_profile(db: &Database, profile_id: i64) -> Result<Vec<i64>, String> {
    let mut stmt = db
        .conn
        .prepare(
            "SELECT source_id FROM epg_source_profiles WHERE profile_id = ?1 ORDER BY source_id",
        )
        .map_err(|error| error.to_string())?;
    let rows = stmt
        .query_map(params![profile_id], |row| row.get(0))
        .map_err(|error| error.to_string())?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|error| error.to_string())
}

/// Drops a removed profile's links, deleting guides no other profile names.
pub fn unlink_profile(db: &Database, profile_id: i64) -> Result<(), String> {
    let linked = sources_for_profile(db, profile_id)?;
    db.conn
        .execute(
            "DELETE FROM epg_source_profiles WHERE profile_id = ?1",
            params![profile_id],
        )
        .map_err(|error| error.to_string())?;
    for source in linked {
        let still_owned: bool = db
            .conn
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM epg_source_profiles WHERE source_id = ?1)",
                params![source],
                |row| row.get(0),
            )
            .map_err(|error| error.to_string())?;
        if still_owned {
            db.conn
                .execute(
                    "UPDATE epg_sources SET profile_id = (SELECT MIN(profile_id) FROM epg_source_profiles WHERE source_id = ?1) WHERE id = ?1",
                    params![source],
                )
                .map_err(|error| error.to_string())?;
        } else {
            delete_source(db, source)?;
        }
    }
    Ok(())
}

pub fn list_sources(db: &Database) -> Result<Vec<EpgSource>, String> {
    let mut stmt = db
        .conn
        .prepare(
            "SELECT id, name, url, profile_id, enabled, last_synced, last_error,
                    channel_count, programme_count
             FROM epg_sources ORDER BY name COLLATE NOCASE",
        )
        .map_err(|error| error.to_string())?;
    let rows = stmt
        .query_map([], |row| {
            Ok(EpgSource {
                id: row.get(0)?,
                name: row.get(1)?,
                url: mask_credentials(&row.get::<_, String>(2)?),
                profile_id: row.get(3)?,
                enabled: row.get(4)?,
                last_synced: row.get(5)?,
                last_error: row.get(6)?,
                channel_count: row.get(7)?,
                programme_count: row.get(8)?,
            })
        })
        .map_err(|error| error.to_string())?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|error| error.to_string())
}

pub fn delete_source(db: &Database, id: i64) -> Result<(), String> {
    for sql in [
        "DELETE FROM epg_programmes WHERE source_id = ?1",
        "DELETE FROM epg_channels WHERE source_id = ?1",
        "DELETE FROM epg_source_profiles WHERE source_id = ?1",
        "DELETE FROM epg_sources WHERE id = ?1",
    ] {
        db.conn
            .execute(sql, params![id])
            .map_err(|error| error.to_string())?;
    }
    Ok(())
}

/// Parsed guide content ready to store.
#[derive(Debug, Default)]
pub struct GuideData {
    pub channels: Vec<XmltvChannel>,
    pub programmes: Vec<XmltvProgramme>,
}

/// Parses a guide file (gzip detected by its magic bytes) and keeps only the
/// programmes around `now`.
pub fn read_guide_file(file: &mut std::fs::File, now: i64) -> Result<GuideData, String> {
    let mut magic = [0u8; 2];
    let read = file.read(&mut magic).map_err(|error| error.to_string())?;
    file.seek(SeekFrom::Start(0))
        .map_err(|error| error.to_string())?;
    let input: Box<dyn Read + '_> = if read == 2 && magic == [0x1f, 0x8b] {
        Box::new(GzDecoder::new(&mut *file))
    } else {
        Box::new(&mut *file)
    };
    let capped = CappedReader {
        inner: input,
        remaining: MAX_GUIDE_XML_BYTES,
    };
    read_guide(BufReader::new(capped), now)
}

pub fn read_guide<R: BufRead>(input: R, now: i64) -> Result<GuideData, String> {
    let mut data = GuideData::default();
    let earliest_stop = now - KEEP_PAST_SECS;
    let latest_start = now + KEEP_AHEAD_SECS;
    let overflow = std::cell::Cell::new(false);
    parse_xmltv(
        input,
        |channel| {
            if data.channels.len() < MAX_CHANNELS {
                data.channels.push(channel);
            } else {
                overflow.set(true);
            }
        },
        |programme| {
            if programme.stop >= earliest_stop && programme.start <= latest_start {
                if data.programmes.len() < MAX_PROGRAMMES {
                    data.programmes.push(programme);
                } else {
                    overflow.set(true);
                }
            }
        },
    )?;
    if overflow.get() {
        return Err(format!(
            "The guide is too large (over {MAX_CHANNELS} channels or {MAX_PROGRAMMES} programmes in the next few days)."
        ));
    }
    if data.channels.is_empty() && data.programmes.is_empty() {
        return Err(
            "The guide had no channels or programmes. Check that the URL points to an XMLTV file."
                .into(),
        );
    }
    Ok(data)
}

/// Replaces a source's channels and programmes in one transaction.
pub fn store_guide(db: &Database, source_id: i64, data: &GuideData) -> Result<(), String> {
    let tx = db
        .conn
        .unchecked_transaction()
        .map_err(|error| error.to_string())?;
    // The source may have been removed while its download was running.
    let exists: bool = tx
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM epg_sources WHERE id = ?1)",
            params![source_id],
            |row| row.get(0),
        )
        .map_err(|error| error.to_string())?;
    if !exists {
        return Err("The guide source was removed while it was syncing.".into());
    }
    tx.execute(
        "DELETE FROM epg_programmes WHERE source_id = ?1",
        params![source_id],
    )
    .map_err(|error| error.to_string())?;
    tx.execute(
        "DELETE FROM epg_channels WHERE source_id = ?1",
        params![source_id],
    )
    .map_err(|error| error.to_string())?;
    {
        let mut insert_channel = tx
            .prepare(
                "INSERT OR REPLACE INTO epg_channels (source_id, channel_id, display_name, name_key)
                 VALUES (?1, ?2, ?3, ?4)",
            )
            .map_err(|error| error.to_string())?;
        for channel in &data.channels {
            insert_channel
                .execute(params![
                    source_id,
                    channel.id.to_lowercase(),
                    channel.display_name,
                    channel_name_key(&channel.display_name)
                ])
                .map_err(|error| error.to_string())?;
        }
        let mut insert_programme = tx
            .prepare(
                "INSERT INTO epg_programmes
                    (source_id, channel_key, start_ts, stop_ts, title, sub_title, description, category, episode)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            )
            .map_err(|error| error.to_string())?;
        for item in &data.programmes {
            insert_programme
                .execute(params![
                    source_id,
                    item.channel.to_lowercase(),
                    item.start,
                    item.stop,
                    item.title,
                    item.sub_title,
                    item.description,
                    item.category,
                    item.episode
                ])
                .map_err(|error| error.to_string())?;
        }
    }
    tx.execute(
        "UPDATE epg_sources SET last_synced = ?1, last_error = NULL,
             channel_count = ?2, programme_count = ?3 WHERE id = ?4",
        params![
            chrono::Utc::now().to_rfc3339(),
            data.channels.len() as i64,
            data.programmes.len() as i64,
            source_id
        ],
    )
    .map_err(|error| error.to_string())?;
    tx.commit().map_err(|error| error.to_string())
}

fn record_error(db: &Database, source_id: i64, error: &str) {
    let _ = db.conn.execute(
        "UPDATE epg_sources SET last_error = ?1 WHERE id = ?2",
        params![error, source_id],
    );
}

// ── Lookup ───────────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize, Clone)]
pub struct GuideChannelRef {
    /// Caller's key for the result map (usually the channel row id).
    pub key: String,
    pub epg_id: Option<String>,
    pub name: String,
}

#[derive(Debug, Serialize, Clone, PartialEq)]
pub struct GuideProgramme {
    pub start: i64,
    pub stop: i64,
    pub title: String,
    pub sub_title: Option<String>,
    pub description: Option<String>,
    pub category: Option<String>,
    pub episode: Option<String>,
}

/// Guide channel ids for a live channel: its own EPG id when the guide knows
/// it, otherwise every guide channel whose display name matches.
fn resolve_channel_keys(
    db: &Database,
    epg_id: Option<&str>,
    name: &str,
) -> Result<Vec<String>, String> {
    if let Some(id) = epg_id.map(str::trim).filter(|id| !id.is_empty()) {
        let id = id.to_lowercase();
        let known: bool = db
            .conn
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM epg_programmes WHERE channel_key = ?1 LIMIT 1)",
                params![id],
                |row| row.get(0),
            )
            .map_err(|error| error.to_string())?;
        if known {
            return Ok(vec![id]);
        }
    }
    let key = channel_name_key(name);
    if key.is_empty() {
        return Ok(Vec::new());
    }
    let mut stmt = db
        .conn
        .prepare("SELECT DISTINCT channel_id FROM epg_channels WHERE name_key = ?1")
        .map_err(|error| error.to_string())?;
    let rows = stmt
        .query_map(params![key], |row| row.get::<_, String>(0))
        .map_err(|error| error.to_string())?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|error| error.to_string())
}

fn programmes_for_keys(
    db: &Database,
    keys: &[String],
    now: i64,
    limit: i64,
) -> Result<Vec<GuideProgramme>, String> {
    let mut found: Vec<GuideProgramme> = Vec::new();
    let mut stmt = db
        .conn
        .prepare(
            "SELECT start_ts, stop_ts, title, sub_title, description, category, episode
             FROM epg_programmes WHERE channel_key = ?1 AND stop_ts > ?2
             ORDER BY start_ts LIMIT ?3",
        )
        .map_err(|error| error.to_string())?;
    for key in keys {
        let rows = stmt
            .query_map(params![key, now, limit], |row| {
                Ok(GuideProgramme {
                    start: row.get(0)?,
                    stop: row.get(1)?,
                    title: row.get(2)?,
                    sub_title: row.get(3)?,
                    description: row.get(4)?,
                    category: row.get(5)?,
                    episode: row.get(6)?,
                })
            })
            .map_err(|error| error.to_string())?;
        for row in rows {
            found.push(row.map_err(|error| error.to_string())?);
        }
    }
    found.sort_by_key(|item| item.start);
    // Two sources can carry the same channel; keep one entry per slot.
    let mut seen = HashSet::new();
    found.retain(|item| seen.insert((item.start, item.title.clone())));
    found.truncate(limit.max(0) as usize);
    Ok(found)
}

pub fn channel_guide(
    db: &Database,
    epg_id: Option<&str>,
    name: &str,
    now: i64,
    limit: i64,
) -> Result<Vec<GuideProgramme>, String> {
    let keys = resolve_channel_keys(db, epg_id, name)?;
    programmes_for_keys(db, &keys, now, limit)
}

// ── Download ─────────────────────────────────────────────────────────────────

/// A downloaded guide on disk, removed when dropped.
struct TempGuide {
    file: Option<std::fs::File>,
    path: std::path::PathBuf,
}

impl Drop for TempGuide {
    fn drop(&mut self) {
        // Close first: Windows will not delete a file that is still open.
        drop(self.file.take());
        let _ = std::fs::remove_file(&self.path);
    }
}

async fn download_to_temp(url: &str) -> Result<TempGuide, String> {
    let client = reqwest::Client::builder()
        .connect_timeout(Duration::from_secs(20))
        .timeout(Duration::from_secs(600))
        .user_agent(concat!(
            "CinaVault-Premium/",
            env!("CARGO_PKG_VERSION"),
            " (Live TV guide)"
        ))
        .build()
        .map_err(|error| error.to_string())?;
    let mut response = client
        .get(url)
        .send()
        .await
        .map_err(|error| format!("Could not reach the guide: {error}"))?;
    if !response.status().is_success() {
        return Err(format!("The guide server answered {}", response.status()));
    }
    let path = std::env::temp_dir().join(format!(
        "cinavault-epg-{}.xml",
        uuid::Uuid::new_v4().simple()
    ));
    let file = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create_new(true)
        .open(&path)
        .map_err(|error| format!("Could not create a temp file for the guide: {error}"))?;
    let mut guide = TempGuide {
        file: Some(file),
        path,
    };
    let file = guide.file.as_mut().expect("file is set above");
    let mut total: u64 = 0;
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|error| format!("Guide download was interrupted: {error}"))?
    {
        total += chunk.len() as u64;
        if total > MAX_DOWNLOAD_BYTES {
            return Err("The guide file is larger than 512 MB".into());
        }
        file.write_all(&chunk).map_err(|error| error.to_string())?;
    }
    file.flush().map_err(|error| error.to_string())?;
    file.seek(SeekFrom::Start(0))
        .map_err(|error| error.to_string())?;
    Ok(guide)
}

#[derive(Debug, Serialize)]
pub struct SyncResult {
    pub source_id: i64,
    pub channels: usize,
    pub programmes: usize,
}

pub async fn sync_source_by_id(state: &AppState, source_id: i64) -> Result<SyncResult, String> {
    let url: String = {
        let db = state.db.lock().map_err(|error| error.to_string())?;
        db.conn
            .query_row(
                "SELECT url FROM epg_sources WHERE id = ?1",
                params![source_id],
                |row| row.get(0),
            )
            .map_err(|_| "Guide source not found".to_string())?
    };
    let outcome = async {
        let mut guide = download_to_temp(&url).await?;
        let now = chrono::Utc::now().timestamp();
        tauri::async_runtime::spawn_blocking(move || {
            let file = guide.file.as_mut().ok_or("Guide download was lost")?;
            read_guide_file(file, now)
        })
        .await
        .map_err(|error| error.to_string())?
    }
    .await;
    let db = state.db.lock().map_err(|error| error.to_string())?;
    match outcome {
        Ok(data) => {
            store_guide(&db, source_id, &data)?;
            Ok(SyncResult {
                source_id,
                channels: data.channels.len(),
                programmes: data.programmes.len(),
            })
        }
        Err(error) => {
            record_error(&db, source_id, &error);
            Err(error)
        }
    }
}

// ── Commands ─────────────────────────────────────────────────────────────────

#[tauri::command]
pub fn list_epg_sources(state: State<AppState>) -> Result<Vec<EpgSource>, String> {
    let db = state.db.lock().map_err(|error| error.to_string())?;
    list_sources(&db)
}

#[tauri::command]
pub fn add_epg_source(state: State<AppState>, name: String, url: String) -> Result<i64, String> {
    let db = state.db.lock().map_err(|error| error.to_string())?;
    upsert_source(&db, &name, &url, None)
}

#[tauri::command]
pub fn remove_epg_source(state: State<AppState>, id: i64) -> Result<(), String> {
    let db = state.db.lock().map_err(|error| error.to_string())?;
    delete_source(&db, id)
}

#[tauri::command]
pub async fn sync_epg_source(state: State<'_, AppState>, id: i64) -> Result<SyncResult, String> {
    sync_source_by_id(&state, id).await
}

/// Refreshes enabled sources never synced or older than 12 hours.
#[tauri::command]
pub async fn sync_stale_epg_sources(
    state: State<'_, AppState>,
) -> Result<serde_json::Value, String> {
    let stale: Vec<i64> = {
        let db = state.db.lock().map_err(|error| error.to_string())?;
        let cutoff =
            (chrono::Utc::now() - chrono::Duration::seconds(STALE_AFTER_SECS)).to_rfc3339();
        let mut stmt = db
            .conn
            .prepare("SELECT id FROM epg_sources WHERE enabled = 1 AND (last_synced IS NULL OR last_synced < ?1)")
            .map_err(|error| error.to_string())?;
        let rows = stmt
            .query_map(params![cutoff], |row| row.get(0))
            .map_err(|error| error.to_string())?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|error| error.to_string())?
    };
    let mut synced = 0;
    let mut failed = Vec::new();
    for id in stale {
        match sync_source_by_id(&state, id).await {
            Ok(_) => synced += 1,
            Err(error) => failed.push(serde_json::json!({ "id": id, "error": error })),
        }
    }
    Ok(serde_json::json!({ "synced": synced, "failed": failed }))
}

#[tauri::command]
pub fn get_channel_guide(
    state: State<AppState>,
    epg_id: Option<String>,
    name: String,
    limit: Option<i64>,
) -> Result<Vec<GuideProgramme>, String> {
    let db = state.db.lock().map_err(|error| error.to_string())?;
    let now = chrono::Utc::now().timestamp();
    channel_guide(
        &db,
        epg_id.as_deref(),
        &name,
        now,
        limit.unwrap_or(24).clamp(1, 200),
    )
}

/// What is on now for many channels at once (the channel list).
#[tauri::command]
pub fn get_now_playing(
    state: State<AppState>,
    channels: Vec<GuideChannelRef>,
) -> Result<HashMap<String, GuideProgramme>, String> {
    let db = state.db.lock().map_err(|error| error.to_string())?;
    let now = chrono::Utc::now().timestamp();
    now_playing(&db, &channels, now)
}

pub fn now_playing(
    db: &Database,
    channels: &[GuideChannelRef],
    now: i64,
) -> Result<HashMap<String, GuideProgramme>, String> {
    let mut result = HashMap::new();
    for channel in channels.iter().take(5000) {
        let keys = resolve_channel_keys(db, channel.epg_id.as_deref(), &channel.name)?;
        if let Some(current) = programmes_for_keys(db, &keys, now, 1)?
            .into_iter()
            .find(|item| item.start <= now)
        {
            result.insert(channel.key.clone(), current);
        }
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE tv SYSTEM "xmltv.dtd">
<tv generator-info-name="test">
  <channel id="bbc1.uk">
    <display-name lang="en">BBC One HD</display-name>
    <display-name>BBC 1</display-name>
  </channel>
  <channel id="news.example"><display-name>World News</display-name></channel>
  <programme start="20261011120000 +0000" stop="20261011130000 +0000" channel="bbc1.uk">
    <title lang="en">Bargain Hunt</title>
    <desc lang="en">Teams &amp; experts compete.</desc>
    <category>Game show</category>
    <episode-num system="xmltv_ns">3.11.0/1</episode-num>
  </programme>
  <programme start="20261011130000 +0000" stop="20261011140000 +0000" channel="bbc1.uk">
    <title><![CDATA[News at One]]></title>
    <sub-title>Lunchtime edition</sub-title>
  </programme>
  <programme start="20261011150000 +0100" stop="20261011160000 +0100" channel="BBC1.UK">
    <title>Doctors</title>
  </programme>
  <programme start="garbage" stop="20261011160000" channel="bbc1.uk"><title>Broken</title></programme>
</tv>"#;

    fn ts(value: &str) -> i64 {
        parse_xmltv_time(value).unwrap()
    }

    #[test]
    fn xmltv_times_respect_offsets() {
        assert_eq!(ts("20261011140000 +0000"), 1_791_727_200);
        assert_eq!(ts("20261011150000 +0100"), 1_791_727_200);
        assert_eq!(ts("20261011090000 -0500"), 1_791_727_200);
        assert_eq!(ts("20261011140000"), 1_791_727_200);
        assert_eq!(ts("202610111400"), 1_791_727_200);
        assert_eq!(parse_xmltv_time("2026"), None);
        assert_eq!(parse_xmltv_time("20261399140000"), None);
    }

    #[test]
    fn channel_name_keys_ignore_tags_and_quality() {
        assert_eq!(channel_name_key("UK: BBC One HD"), "bbcone");
        assert_eq!(channel_name_key("BBC One"), "bbcone");
        assert_eq!(channel_name_key("[DE] ZDF FHD"), "zdf");
        assert_eq!(channel_name_key("US | ESPN 2"), "espn2");
        assert_eq!(channel_name_key("FR - TF1 4K"), "tf1");
        assert_eq!(
            channel_name_key("Sky Sports: Main Event"),
            "skysportsmainevent"
        );
    }

    #[test]
    fn parses_channels_programmes_and_details() {
        let data = read_guide(SAMPLE.as_bytes(), ts("20261011133000")).unwrap();
        assert_eq!(
            data.channels,
            vec![
                XmltvChannel {
                    id: "bbc1.uk".into(),
                    display_name: "BBC One HD".into()
                },
                XmltvChannel {
                    id: "news.example".into(),
                    display_name: "World News".into()
                },
            ]
        );
        assert_eq!(
            data.programmes.len(),
            3,
            "the programme with a bad start is skipped"
        );
        let first = &data.programmes[0];
        assert_eq!(first.title, "Bargain Hunt");
        assert_eq!(
            first.description.as_deref(),
            Some("Teams & experts compete.")
        );
        assert_eq!(first.category.as_deref(), Some("Game show"));
        assert_eq!(first.episode.as_deref(), Some("S4 E12"));
        assert_eq!(data.programmes[1].title, "News at One");
        assert_eq!(
            data.programmes[1].sub_title.as_deref(),
            Some("Lunchtime edition")
        );
    }

    #[test]
    fn keeps_only_the_rolling_window() {
        let far_future = ts("20261020000000");
        let data = read_guide(SAMPLE.as_bytes(), far_future).unwrap();
        assert!(data.programmes.is_empty());
        assert_eq!(data.channels.len(), 2);
    }

    #[test]
    fn rejects_files_that_are_not_guides() {
        assert!(read_guide("<html><body>Login</body></html>".as_bytes(), 0).is_err());
        assert!(read_guide("not xml at all <<<".as_bytes(), 0).is_err());
    }

    #[test]
    fn reads_gzipped_guides() {
        use flate2::write::GzEncoder;
        let mut encoder = GzEncoder::new(Vec::new(), flate2::Compression::default());
        encoder.write_all(SAMPLE.as_bytes()).unwrap();
        let compressed = encoder.finish().unwrap();
        let path =
            std::env::temp_dir().join(format!("epg-test-{}.gz", uuid::Uuid::new_v4().simple()));
        std::fs::write(&path, compressed).unwrap();
        let mut file = std::fs::File::open(&path).unwrap();
        let data = read_guide_file(&mut file, ts("20261011133000")).unwrap();
        drop(file);
        let _ = std::fs::remove_file(&path);
        assert_eq!(data.programmes.len(), 3);
    }

    #[test]
    fn credentials_are_masked_in_listed_urls() {
        let masked =
            mask_credentials("http://tv.example:8080/xmltv.php?username=me&password=s3cret");
        assert!(!masked.contains("s3cret"), "{masked}");
        assert!(!masked.contains("=me"), "{masked}");
        assert!(masked.starts_with("http://tv.example:8080/xmltv.php?"));
        assert_eq!(
            mask_credentials("https://epg.example/guide.xml.gz"),
            "https://epg.example/guide.xml.gz"
        );
    }

    #[test]
    fn existing_sources_link_to_the_playlist_that_names_them() {
        let db = Database::new(":memory:").unwrap();
        ensure_tables(&db).unwrap();
        let id = upsert_source(&db, "Manual", "https://epg.example/b.xml", None).unwrap();
        assert_eq!(
            upsert_source(&db, "", "https://epg.example/b.xml", Some(4)).unwrap(),
            id
        );
        let profile: Option<i64> = db
            .conn
            .query_row(
                "SELECT profile_id FROM epg_sources WHERE id = ?1",
                params![id],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(profile, Some(4));
    }

    #[test]
    fn a_guide_shared_by_two_playlists_survives_removing_one() {
        let db = Database::new(":memory:").unwrap();
        ensure_tables(&db).unwrap();
        let id = upsert_source(&db, "", "https://epg.example/c.xml", Some(1)).unwrap();
        assert_eq!(
            upsert_source(&db, "", "https://epg.example/c.xml", Some(2)).unwrap(),
            id
        );
        assert_eq!(sources_for_profile(&db, 2).unwrap(), vec![id]);
        unlink_profile(&db, 1).unwrap();
        assert_eq!(
            sources_for_profile(&db, 2).unwrap(),
            vec![id],
            "still owned"
        );
        assert_eq!(list_sources(&db).unwrap().len(), 1);
        unlink_profile(&db, 2).unwrap();
        assert!(
            list_sources(&db).unwrap().is_empty(),
            "last owner removes it"
        );
    }

    #[test]
    fn a_removed_source_stores_nothing() {
        let (db, source) = guide_db();
        delete_source(&db, source).unwrap();
        let data = read_guide(SAMPLE.as_bytes(), ts("20261011133000")).unwrap();
        assert!(store_guide(&db, source, &data).is_err());
        let rows: i64 = db
            .conn
            .query_row("SELECT COUNT(*) FROM epg_programmes", [], |row| row.get(0))
            .unwrap();
        assert_eq!(rows, 0, "no orphan programmes");
    }

    #[test]
    fn capped_reader_fails_instead_of_truncating() {
        let mut reader = CappedReader {
            inner: &b"0123456789"[..],
            remaining: 4,
        };
        let mut out = Vec::new();
        assert!(reader.read_to_end(&mut out).is_err());
        assert_eq!(out, b"0123");
    }

    #[test]
    fn source_urls_must_be_http() {
        assert!(validate_source_url("https://epg.example/guide.xml").is_ok());
        assert!(validate_source_url("file:///etc/passwd").is_err());
        assert!(validate_source_url("epg.example/guide.xml").is_err());
    }

    fn guide_db() -> (Database, i64) {
        let db = Database::new(":memory:").unwrap();
        ensure_tables(&db).unwrap();
        let source = upsert_source(&db, "Test", "https://epg.example/a.xml", None).unwrap();
        assert_eq!(
            upsert_source(&db, "Again", "https://epg.example/a.xml", None).unwrap(),
            source
        );
        let data = read_guide(SAMPLE.as_bytes(), ts("20261011133000")).unwrap();
        store_guide(&db, source, &data).unwrap();
        (db, source)
    }

    #[test]
    fn guide_matches_by_epg_id_then_by_name() {
        let (db, _) = guide_db();
        let now = ts("20261011133000");
        let by_id = channel_guide(&db, Some("BBC1.uk"), "anything", now, 10).unwrap();
        assert_eq!(
            by_id.iter().map(|p| p.title.as_str()).collect::<Vec<_>>(),
            vec!["News at One", "Doctors"],
            "finished programmes are left out"
        );
        let by_name = channel_guide(&db, Some("unknown.id"), "UK | BBC One", now, 10).unwrap();
        assert_eq!(by_name, by_id);
        assert!(channel_guide(&db, None, "Nothing Here", now, 10)
            .unwrap()
            .is_empty());
    }

    #[test]
    fn now_playing_maps_each_channel_key() {
        let (db, _) = guide_db();
        let now = ts("20261011133000");
        let refs = vec![
            GuideChannelRef {
                key: "7".into(),
                epg_id: Some("bbc1.uk".into()),
                name: "BBC One".into(),
            },
            GuideChannelRef {
                key: "8".into(),
                epg_id: None,
                name: "World News".into(),
            },
        ];
        let map = now_playing(&db, &refs, now).unwrap();
        assert_eq!(map.get("7").map(|p| p.title.as_str()), Some("News at One"));
        assert!(
            !map.contains_key("8"),
            "a channel with no programmes has nothing on now"
        );
    }

    #[test]
    fn listing_and_removing_sources() {
        let (db, source) = guide_db();
        let listed = list_sources(&db).unwrap();
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].programme_count, 3);
        assert_eq!(listed[0].channel_count, 2);
        assert!(listed[0].last_synced.is_some());
        delete_source(&db, source).unwrap();
        assert!(list_sources(&db).unwrap().is_empty());
        let left: i64 = db
            .conn
            .query_row("SELECT COUNT(*) FROM epg_programmes", [], |r| r.get(0))
            .unwrap();
        assert_eq!(left, 0);
    }
}
