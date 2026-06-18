use crate::AppState;
use regex::Regex;
use rusqlite::params;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    fs,
    path::{Path, PathBuf},
};
use tauri::{AppHandle, Manager, State};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PgmaRefreshConfig {
    pub overwrite_existing_metadata: Option<bool>,
    pub download_artwork: Option<bool>,
    pub metadata_sources: Option<Vec<String>>,
    pub limit: Option<usize>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PgmaRefreshResult {
    pub success: bool,
    pub scanned: usize,
    pub matched: usize,
    pub updated: usize,
    pub artwork_downloaded: usize,
    pub skipped: usize,
    pub errors: Vec<String>,
    pub toolchain: String,
    pub message: String,
}

#[derive(Debug, Clone)]
struct LibraryItem {
    id: i64,
    file_path: String,
    title: Option<String>,
    overview: Option<String>,
    poster_path: Option<String>,
    year: Option<i32>,
    rating: Option<f64>,
    genre: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq)]
struct MetadataPatch {
    title: Option<String>,
    overview: Option<String>,
    poster_path: Option<String>,
    year: Option<i32>,
    rating: Option<f64>,
    genre: Option<String>,
}

#[tauri::command]
pub async fn refresh_pgma_library(
    app: AppHandle,
    state: State<'_, AppState>,
    config: Option<String>,
) -> Result<PgmaRefreshResult, String> {
    let config = parse_config(config.as_deref());
    let overwrite = config.overwrite_existing_metadata.unwrap_or(false);
    let download_artwork = config.download_artwork.unwrap_or(true);
    let sources = config
        .metadata_sources
        .unwrap_or_else(|| vec!["nfo".to_string(), "localArtwork".to_string()]);
    let use_nfo = sources.iter().any(|source| source.eq_ignore_ascii_case("nfo"));
    let use_local_artwork = sources.iter().any(|source| {
        source.eq_ignore_ascii_case("localArtwork") || source.eq_ignore_ascii_case("local_artwork")
    });
    let limit = config.limit.unwrap_or(5000).max(1);
    let app_data_dir = app.path().app_data_dir().map_err(|error| error.to_string())?;

    let items = {
        let db = state.db.lock().map_err(|error| error.to_string())?;
        let mut stmt = db
            .conn
            .prepare(
                "SELECT id, file_path, title, overview, poster_path, year, rating, genre
                 FROM media_items
                 WHERE media_type IN ('movie', 'episode', 'video', 'adult')
                 ORDER BY date_added DESC
                 LIMIT ?1",
            )
            .map_err(|error| error.to_string())?;
        let rows = stmt
            .query_map(params![limit as i64], |row| {
                Ok(LibraryItem {
                    id: row.get(0)?,
                    file_path: row.get(1)?,
                    title: row.get(2)?,
                    overview: row.get(3)?,
                    poster_path: row.get(4)?,
                    year: row.get(5)?,
                    rating: row.get(6)?,
                    genre: row.get(7)?,
                })
            })
            .map_err(|error| error.to_string())?;
        let collected_items = rows
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| error.to_string())?;
        collected_items
    };

    let client = reqwest::Client::new();
    let mut scanned = 0usize;
    let mut matched = 0usize;
    let mut updated = 0usize;
    let mut artwork_downloaded = 0usize;
    let mut errors = Vec::new();

    for item in items {
        scanned += 1;
        let media_path = PathBuf::from(&item.file_path);
        let mut patch = MetadataPatch::default();

        if use_nfo {
            if let Some(sidecar_patch) = read_sidecar_metadata(&media_path) {
                patch = merge_patches(patch, sidecar_patch);
            }
        }

        if use_local_artwork && patch.poster_path.is_none() {
            patch.poster_path = find_local_artwork(&media_path);
        }

        if patch == MetadataPatch::default() {
            continue;
        }
        matched += 1;

        if let Some(poster) = patch.poster_path.clone() {
            match resolve_artwork_path(&client, &app_data_dir, &poster, download_artwork).await {
                Ok((resolved, downloaded)) => {
                    patch.poster_path = resolved;
                    if downloaded {
                        artwork_downloaded += 1;
                    }
                }
                Err(error) => errors.push(format!("{}: {}", display_item_name(&item), error)),
            }
        }

        let next_title = choose_text(&item.title, &patch.title, overwrite);
        let next_overview = choose_text(&item.overview, &patch.overview, overwrite);
        let next_poster = choose_text(&item.poster_path, &patch.poster_path, overwrite);
        let next_genre = choose_text(&item.genre, &patch.genre, overwrite);
        let next_year = choose_copy(item.year, patch.year, overwrite);
        let next_rating = choose_copy(item.rating, patch.rating, overwrite);

        let changed = next_title != item.title
            || next_overview != item.overview
            || next_poster != item.poster_path
            || next_genre != item.genre
            || next_year != item.year
            || next_rating != item.rating;

        if !changed {
            continue;
        }

        let write_result = {
            let db = state.db.lock().map_err(|error| error.to_string())?;
            db.conn.execute(
                "UPDATE media_items
                 SET title = ?2,
                     overview = ?3,
                     poster_path = ?4,
                     year = ?5,
                     rating = ?6,
                     genre = ?7
                 WHERE id = ?1",
                params![
                    item.id,
                    next_title,
                    next_overview,
                    next_poster,
                    next_year,
                    next_rating,
                    next_genre,
                ],
            )
        };

        match write_result {
            Ok(rows) if rows > 0 => updated += 1,
            Ok(_) => errors.push(format!("{}: no database row updated", display_item_name(&item))),
            Err(error) => errors.push(format!("{}: {}", display_item_name(&item), error)),
        }
    }

    Ok(PgmaRefreshResult {
        success: errors.is_empty(),
        scanned,
        matched,
        updated,
        artwork_downloaded,
        skipped: scanned.saturating_sub(matched),
        errors,
        toolchain: "native-rust-pgma-bridge".to_string(),
        message: "PGMA bridge refresh completed. CinaVault read sidecar/local metadata and wrote matching fields into the library database.".to_string(),
    })
}

fn parse_config(raw: Option<&str>) -> PgmaRefreshConfig {
    raw.and_then(|value| serde_json::from_str::<PgmaRefreshConfig>(value).ok())
        .unwrap_or(PgmaRefreshConfig {
            overwrite_existing_metadata: Some(false),
            download_artwork: Some(true),
            metadata_sources: Some(vec!["nfo".to_string(), "localArtwork".to_string()]),
            limit: Some(5000),
        })
}

fn read_sidecar_metadata(media_path: &Path) -> Option<MetadataPatch> {
    let sidecar_path = find_sidecar_nfo(media_path)?;
    let xml = fs::read_to_string(&sidecar_path).ok()?;
    let base_dir = sidecar_path.parent().unwrap_or_else(|| Path::new("."));
    Some(parse_nfo_metadata(&xml, base_dir))
}

fn find_sidecar_nfo(media_path: &Path) -> Option<PathBuf> {
    let parent = media_path.parent()?;
    let candidates = vec![
        media_path.with_extension("nfo"),
        parent.join("movie.nfo"),
        parent.join("metadata.nfo"),
    ];

    let found = candidates.into_iter().find(|candidate| candidate.is_file());
    found
}

fn parse_nfo_metadata(xml: &str, base_dir: &Path) -> MetadataPatch {
    let title = tag_text(xml, "title").or_else(|| tag_text(xml, "originaltitle"));
    let overview = tag_text(xml, "plot").or_else(|| tag_text(xml, "outline"));
    let year = tag_text(xml, "year")
        .or_else(|| tag_text(xml, "premiered"))
        .or_else(|| tag_text(xml, "releasedate"))
        .and_then(|value| first_year(&value));
    let rating = tag_text(xml, "rating").and_then(|value| value.parse::<f64>().ok());
    let genre_values = tag_values(xml, "genre");
    let genre = if genre_values.is_empty() {
        None
    } else {
        Some(genre_values.join(", "))
    };
    let poster_path = tag_text(xml, "thumb")
        .or_else(|| tag_text(xml, "poster"))
        .or_else(|| tag_text(xml, "cover"))
        .and_then(|value| normalize_artwork_reference(&value, base_dir));

    MetadataPatch {
        title,
        overview,
        poster_path,
        year,
        rating,
        genre,
    }
}

fn tag_text(xml: &str, tag: &str) -> Option<String> {
    tag_values(xml, tag).into_iter().next()
}

fn tag_values(xml: &str, tag: &str) -> Vec<String> {
    let pattern = format!(r"(?is)<{}\b[^>]*>(.*?)</{}>", regex::escape(tag), regex::escape(tag));
    let Ok(re) = Regex::new(&pattern) else {
        return Vec::new();
    };
    re.captures_iter(xml)
        .filter_map(|captures| captures.get(1).map(|match_| clean_xml_text(match_.as_str())))
        .filter(|value| !value.trim().is_empty())
        .collect()
}

fn clean_xml_text(raw: &str) -> String {
    let without_cdata = raw.replace("<![CDATA[", "").replace("]]>", "");
    let without_tags = Regex::new(r"(?is)<[^>]+>")
        .map(|re| re.replace_all(&without_cdata, " ").to_string())
        .unwrap_or(without_cdata);
    xml_unescape(without_tags.trim())
}

fn xml_unescape(value: &str) -> String {
    value
        .replace("&amp;", "&")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
}

fn first_year(value: &str) -> Option<i32> {
    let re = Regex::new(r"\b(19|20)\d{2}\b").ok()?;
    re.find(value)
        .and_then(|match_| match_.as_str().parse::<i32>().ok())
}

fn normalize_artwork_reference(value: &str, base_dir: &Path) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return None;
    }
    if is_http_url(trimmed) {
        return Some(trimmed.to_string());
    }
    let path = PathBuf::from(trimmed);
    let resolved = if path.is_absolute() { path } else { base_dir.join(path) };
    resolved.is_file().then(|| resolved.display().to_string())
}

fn find_local_artwork(media_path: &Path) -> Option<String> {
    let parent = media_path.parent()?;
    let stem = media_path.file_stem()?.to_string_lossy();
    let candidates = vec![
        parent.join(format!("{stem}.jpg")),
        parent.join(format!("{stem}.jpeg")),
        parent.join(format!("{stem}.png")),
        parent.join(format!("{stem}.webp")),
        parent.join("poster.jpg"),
        parent.join("poster.png"),
        parent.join("folder.jpg"),
        parent.join("cover.jpg"),
    ];

    let found = candidates
        .into_iter()
        .find(|candidate| candidate.is_file())
        .map(|path| path.display().to_string());
    found
}

async fn resolve_artwork_path(
    client: &reqwest::Client,
    app_data_dir: &Path,
    poster: &str,
    download_artwork: bool,
) -> Result<(Option<String>, bool), String> {
    if !is_http_url(poster) || !download_artwork {
        return Ok((Some(poster.to_string()), false));
    }

    let response = client
        .get(poster)
        .send()
        .await
        .map_err(|error| format!("artwork download failed: {error}"))?;
    if !response.status().is_success() {
        return Err(format!("artwork download failed: HTTP {}", response.status()));
    }
    let bytes = response
        .bytes()
        .await
        .map_err(|error| format!("artwork read failed: {error}"))?;
    if bytes.is_empty() {
        return Err("artwork download returned an empty file".to_string());
    }

    let ext = extension_from_url(poster).unwrap_or("jpg");
    let mut hasher = Sha256::new();
    hasher.update(poster.as_bytes());
    let hash = format!("{:x}", hasher.finalize());
    let artwork_dir = app_data_dir.join("metadata").join("pgma-artwork");
    fs::create_dir_all(&artwork_dir).map_err(|error| error.to_string())?;
    let output_path = artwork_dir.join(format!("{}.{}", &hash[..24], ext));
    fs::write(&output_path, bytes).map_err(|error| error.to_string())?;
    Ok((Some(output_path.display().to_string()), true))
}

fn extension_from_url(url: &str) -> Option<&'static str> {
    let lower = url.split('?').next().unwrap_or(url).to_lowercase();
    if lower.ends_with(".png") {
        Some("png")
    } else if lower.ends_with(".webp") {
        Some("webp")
    } else if lower.ends_with(".jpeg") {
        Some("jpeg")
    } else if lower.ends_with(".jpg") {
        Some("jpg")
    } else {
        None
    }
}

fn is_http_url(value: &str) -> bool {
    value.starts_with("http://") || value.starts_with("https://")
}

fn merge_patches(current: MetadataPatch, incoming: MetadataPatch) -> MetadataPatch {
    MetadataPatch {
        title: current.title.or(incoming.title),
        overview: current.overview.or(incoming.overview),
        poster_path: current.poster_path.or(incoming.poster_path),
        year: current.year.or(incoming.year),
        rating: current.rating.or(incoming.rating),
        genre: current.genre.or(incoming.genre),
    }
}

fn choose_text(current: &Option<String>, incoming: &Option<String>, overwrite: bool) -> Option<String> {
    if let Some(value) = incoming.as_ref().filter(|value| !value.trim().is_empty()) {
        if overwrite || current.as_ref().map(|value| value.trim().is_empty()).unwrap_or(true) {
            return Some(value.trim().to_string());
        }
    }
    current.clone()
}

fn choose_copy<T: Copy>(current: Option<T>, incoming: Option<T>, overwrite: bool) -> Option<T> {
    if overwrite || current.is_none() {
        incoming.or(current)
    } else {
        current
    }
}

fn display_item_name(item: &LibraryItem) -> String {
    item.title
        .clone()
        .filter(|title| !title.trim().is_empty())
        .unwrap_or_else(|| item.file_path.clone())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_nfo_metadata_fields() {
        let xml = r#"
            <movie>
                <title>Example Title</title>
                <plot><![CDATA[Example &amp; overview]]></plot>
                <year>2024</year>
                <rating>7.5</rating>
                <genre>Drama</genre>
                <genre>Feature</genre>
            </movie>
        "#;
        let patch = parse_nfo_metadata(xml, Path::new("."));
        assert_eq!(patch.title.as_deref(), Some("Example Title"));
        assert_eq!(patch.overview.as_deref(), Some("Example & overview"));
        assert_eq!(patch.year, Some(2024));
        assert_eq!(patch.rating, Some(7.5));
        assert_eq!(patch.genre.as_deref(), Some("Drama, Feature"));
    }

    #[test]
    fn preserves_existing_values_without_overwrite() {
        let current = Some("Current".to_string());
        let incoming = Some("Incoming".to_string());
        assert_eq!(choose_text(&current, &incoming, false).as_deref(), Some("Current"));
        assert_eq!(choose_text(&current, &incoming, true).as_deref(), Some("Incoming"));
    }
}
