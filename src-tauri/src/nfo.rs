//! Kodi-style .nfo import for the "nfo_import" switch.
//!
//! During a scan the scanner asks for the NFO that belongs to a video
//! (`<name>.nfo`, then `movie.nfo` in the same folder, and `tvshow.nfo` in the
//! folder or the one above for episodes), parses it, and applies it to the
//! library row. NFO values replace guessed values; on a row the user verified
//! they only fill fields that are still empty.

use crate::db::Database;
use crate::media_extras::{self, MediaExtras};
use quick_xml::events::Event;
use quick_xml::Reader;
use regex::Regex;
use rusqlite::{params, OptionalExtension};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

/// Largest NFO read; real ones are a few KB, anything bigger is not an NFO.
const MAX_NFO_BYTES: u64 = 2 * 1024 * 1024;

#[derive(Debug, Clone, Default, PartialEq)]
pub struct NfoData {
    /// Root element: movie, tvshow, episodedetails, musicvideo.
    pub kind: String,
    pub title: Option<String>,
    pub show_title: Option<String>,
    pub year: Option<i32>,
    pub plot: Option<String>,
    pub rating: Option<f64>,
    pub genres: Vec<String>,
    pub tmdb_id: Option<String>,
    pub imdb_id: Option<String>,
    pub mpaa: Option<String>,
    pub poster: Option<String>,
    pub fanart: Option<String>,
    pub set_name: Option<String>,
    pub season: Option<u32>,
    pub episode: Option<u32>,
}

impl NfoData {
    pub fn is_empty(&self) -> bool {
        self.title.is_none()
            && self.year.is_none()
            && self.plot.is_none()
            && self.rating.is_none()
            && self.genres.is_empty()
            && self.tmdb_id.is_none()
            && self.imdb_id.is_none()
            && self.mpaa.is_none()
            && self.poster.is_none()
            && self.fanart.is_none()
    }

    pub fn genre_text(&self) -> Option<String> {
        (!self.genres.is_empty()).then(|| self.genres.join(", "))
    }
}

fn clean(value: &str) -> Option<String> {
    let trimmed = value.trim();
    (!trimmed.is_empty()).then(|| trimmed.to_string())
}

fn imdb_regex() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"\btt\d{6,9}\b").expect("imdb regex"))
}

fn tmdb_url_regex() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"themoviedb\.org/(?:movie|tv)/(\d+)").expect("tmdb url regex"))
}

fn parse_year(value: &str) -> Option<i32> {
    let digits: String = value.trim().chars().take(4).collect();
    digits
        .parse::<i32>()
        .ok()
        .filter(|year| (1880..=2100).contains(year))
}

fn parse_rating(value: &str) -> Option<f64> {
    value
        .trim()
        .replace(',', ".")
        .parse::<f64>()
        .ok()
        .filter(|rating| rating.is_finite() && *rating > 0.0 && *rating <= 10.0)
}

#[derive(Default)]
struct RatingCandidate {
    default: bool,
    max: Option<f64>,
    value: Option<f64>,
}

/// Parses NFO text. Tolerates mismatched tags, entities, CDATA, and the
/// "URL only" NFO style (a bare IMDb or TMDB link).
pub fn parse_nfo(text: &str) -> NfoData {
    let mut data = NfoData::default();
    let mut reader = Reader::from_str(text);
    reader.config_mut().check_end_names = false;

    let mut stack: Vec<String> = Vec::new();
    let mut attrs: Vec<Vec<(String, String)>> = Vec::new();
    let mut buffer = String::new();
    let mut ratings: Vec<RatingCandidate> = Vec::new();
    let mut plain_rating = None;
    let mut outline = None;
    let mut premiered_year = None;

    loop {
        match reader.read_event() {
            Ok(Event::Start(start)) => {
                let name =
                    String::from_utf8_lossy(start.local_name().as_ref()).to_ascii_lowercase();
                let element_attrs = start
                    .attributes()
                    .flatten()
                    .map(|attr| {
                        (
                            String::from_utf8_lossy(attr.key.local_name().as_ref())
                                .to_ascii_lowercase(),
                            String::from_utf8_lossy(&attr.value).to_string(),
                        )
                    })
                    .collect::<Vec<_>>();
                if name == "rating" && stack.last().map(String::as_str) == Some("ratings") {
                    ratings.push(RatingCandidate {
                        default: element_attrs
                            .iter()
                            .any(|(key, value)| key == "default" && value == "true"),
                        max: element_attrs
                            .iter()
                            .find(|(key, _)| key == "max")
                            .and_then(|(_, value)| value.parse().ok()),
                        value: None,
                    });
                }
                stack.push(name);
                attrs.push(element_attrs);
                buffer.clear();
            }
            Ok(Event::Text(text)) => {
                if let Ok(decoded) = text.decode() {
                    buffer.push_str(&decoded);
                }
            }
            Ok(Event::CData(cdata)) => {
                if let Ok(decoded) = cdata.decode() {
                    buffer.push_str(&decoded);
                }
            }
            Ok(Event::GeneralRef(reference)) => {
                if let Ok(Some(character)) = reference.resolve_char_ref() {
                    buffer.push(character);
                } else if let Ok(name) = reference.decode() {
                    let entity = format!("&{name};");
                    match quick_xml::escape::unescape(&entity) {
                        Ok(resolved) => buffer.push_str(&resolved),
                        Err(_) => buffer.push_str(&entity),
                    }
                }
            }
            Ok(Event::End(_)) => {
                let Some(name) = stack.pop() else { continue };
                let element_attrs = attrs.pop().unwrap_or_default();
                let value = std::mem::take(&mut buffer);
                let parent = stack.last().map(String::as_str).unwrap_or_default();
                let depth = stack.len();
                if depth == 0 {
                    data.kind = name;
                    continue;
                }
                let attr = |key: &str| {
                    element_attrs
                        .iter()
                        .find(|(name, _)| name == key)
                        .map(|(_, value)| value.as_str())
                };
                match (parent, name.as_str()) {
                    (_, "title") if depth == 1 => data.title = clean(&value),
                    (_, "originaltitle") if depth == 1 && data.title.is_none() => {
                        data.title = clean(&value)
                    }
                    (_, "showtitle") if depth == 1 => data.show_title = clean(&value),
                    (_, "year") if depth == 1 => data.year = parse_year(&value).or(data.year),
                    (_, "premiered" | "aired" | "releasedate") if depth == 1 => {
                        premiered_year = premiered_year.or_else(|| parse_year(&value))
                    }
                    (_, "plot") if depth == 1 => data.plot = clean(&value),
                    (_, "outline") if depth == 1 => outline = clean(&value),
                    (_, "rating") if depth == 1 => plain_rating = parse_rating(&value),
                    ("rating", "value") => {
                        if let Some(candidate) = ratings.last_mut() {
                            candidate.value = value.trim().replace(',', ".").parse().ok();
                        }
                    }
                    (_, "genre") if depth == 1 => {
                        for genre in value.split(['/', '|']) {
                            if let Some(genre) = clean(genre) {
                                if !data.genres.contains(&genre) {
                                    data.genres.push(genre);
                                }
                            }
                        }
                    }
                    (_, "uniqueid") if depth == 1 => {
                        let kind = attr("type").unwrap_or_default().to_ascii_lowercase();
                        let id = clean(&value);
                        match kind.as_str() {
                            "tmdb" => data.tmdb_id = id.or(data.tmdb_id.take()),
                            "imdb" => data.imdb_id = id.or(data.imdb_id.take()),
                            _ if id.as_deref().is_some_and(|id| imdb_regex().is_match(id)) => {
                                data.imdb_id = data.imdb_id.take().or(id)
                            }
                            _ => {}
                        }
                    }
                    (_, "tmdbid") if depth == 1 => {
                        data.tmdb_id = data.tmdb_id.take().or(clean(&value))
                    }
                    (_, "imdbid" | "imdb_id") if depth == 1 => {
                        data.imdb_id = data.imdb_id.take().or(clean(&value))
                    }
                    (_, "id") if depth == 1 => {
                        let id = clean(&value);
                        if id.as_deref().is_some_and(|id| imdb_regex().is_match(id)) {
                            data.imdb_id = data.imdb_id.take().or(id);
                        }
                    }
                    (_, "mpaa" | "certification") if depth == 1 => {
                        data.mpaa = data.mpaa.take().or(clean(&value))
                    }
                    ("fanart", "thumb") => data.fanart = data.fanart.take().or(clean(&value)),
                    (_, "fanart") if depth == 1 => {
                        data.fanart = data.fanart.take().or(clean(&value))
                    }
                    (_, "thumb") if depth == 1 => {
                        let aspect = attr("aspect").unwrap_or("poster").to_ascii_lowercase();
                        let url = clean(&value);
                        match aspect.as_str() {
                            "poster" => data.poster = data.poster.take().or(url),
                            "fanart" | "landscape" => data.fanart = data.fanart.take().or(url),
                            _ => {}
                        }
                    }
                    ("set", "name") => data.set_name = clean(&value),
                    (_, "set") if depth == 1 && data.set_name.is_none() => {
                        data.set_name = clean(&value)
                    }
                    (_, "season") if depth == 1 => data.season = value.trim().parse().ok(),
                    (_, "episode") if depth == 1 => data.episode = value.trim().parse().ok(),
                    _ => {}
                }
            }
            Ok(Event::Eof) => break,
            Ok(_) => {}
            // Keep whatever was read before the broken part.
            Err(_) => break,
        }
    }

    data.plot = data.plot.or(outline);
    data.year = data.year.or(premiered_year);
    let preferred = ratings
        .iter()
        .find(|rating| rating.default && rating.value.is_some())
        .or_else(|| ratings.iter().find(|rating| rating.value.is_some()));
    data.rating = preferred
        .and_then(|rating| {
            let value = rating.value?;
            let max = rating.max.filter(|max| *max > 0.0).unwrap_or(10.0);
            parse_rating(&format!("{}", value * 10.0 / max))
        })
        .or(plain_rating);
    data.tmdb_id = data
        .tmdb_id
        .filter(|id| id.chars().all(|c| c.is_ascii_digit()));

    // URL-only NFO, or a link line after the XML.
    if data.imdb_id.is_none() {
        data.imdb_id = imdb_regex().find(text).map(|m| m.as_str().to_string());
    }
    if data.tmdb_id.is_none() {
        data.tmdb_id = tmdb_url_regex()
            .captures(text)
            .and_then(|caps| caps.get(1))
            .map(|m| m.as_str().to_string());
    }
    data
}

fn read_nfo(path: &Path) -> Option<NfoData> {
    let metadata = std::fs::metadata(path).ok()?;
    if !metadata.is_file() || metadata.len() > MAX_NFO_BYTES {
        return None;
    }
    let bytes = std::fs::read(path).ok()?;
    let text = String::from_utf8_lossy(&bytes);
    let text = text.trim_start_matches('\u{feff}');
    let parsed = parse_nfo(text);
    (!parsed.is_empty()).then_some(parsed)
}

/// The item NFO (`<name>.nfo` or `movie.nfo`) and the show NFO (`tvshow.nfo`
/// in the folder or its parent) that belong to `video`.
pub fn nfo_paths_for(video: &Path) -> (Option<PathBuf>, Option<PathBuf>) {
    let Some(parent) = video.parent() else {
        return (None, None);
    };
    let stem = video.file_stem().map(|s| s.to_string_lossy().to_string());
    let item = stem
        .map(|stem| parent.join(format!("{stem}.nfo")))
        .filter(|path| path.is_file())
        .or_else(|| Some(parent.join("movie.nfo")).filter(|path| path.is_file()));
    let show = [Some(parent), parent.parent()]
        .into_iter()
        .flatten()
        .map(|dir| dir.join("tvshow.nfo"))
        .find(|path| path.is_file());
    (item, show)
}

/// Resolves an NFO artwork reference: an https URL as is, a local path
/// relative to the NFO folder when the file exists.
fn resolve_artwork(reference: Option<&str>, nfo_dir: &Path) -> Option<String> {
    let reference = reference?.trim();
    if reference.starts_with("https://") {
        return Some(reference.to_string());
    }
    if reference.starts_with("http://") || reference.is_empty() {
        return None;
    }
    let candidate = Path::new(reference);
    let path = if candidate.is_absolute() {
        candidate.to_path_buf()
    } else {
        nfo_dir.join(candidate)
    };
    // Only images in the media folder (or the one above, for show art): an
    // NFO must not be able to point the library at arbitrary local files.
    let is_image = path
        .extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| {
            ["jpg", "jpeg", "png", "webp", "gif"].contains(&ext.to_ascii_lowercase().as_str())
        });
    let resolved = path.canonicalize().ok()?;
    let root = nfo_dir.parent().unwrap_or(nfo_dir).canonicalize().ok()?;
    (is_image && resolved.is_file() && resolved.starts_with(&root))
        .then(|| path.to_string_lossy().to_string())
}

/// Kodi artwork files next to the video: `<name>-fanart.jpg`, `fanart.jpg`.
fn sidecar_fanart(video: &Path) -> Option<String> {
    let parent = video.parent()?;
    let stem = video.file_stem()?.to_string_lossy().to_string();
    for name in [
        format!("{stem}-fanart"),
        "fanart".to_string(),
        "backdrop".to_string(),
    ] {
        for ext in ["jpg", "jpeg", "png", "webp"] {
            let path = parent.join(format!("{name}.{ext}"));
            if path.is_file() {
                return Some(path.to_string_lossy().to_string());
            }
        }
    }
    None
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct NfoApplyReport {
    pub nfo_found: bool,
    pub fields_written: usize,
}

struct RowState {
    verified: bool,
    title: String,
    year: Option<i32>,
    overview: Option<String>,
    rating: Option<f64>,
    genre: Option<String>,
    tmdb_id: Option<String>,
    imdb_id: Option<String>,
    poster_path: Option<String>,
    backdrop_path: Option<String>,
    media_type: String,
}

fn blank(value: &Option<String>) -> bool {
    value.as_deref().map(str::trim).is_none_or(str::is_empty)
}

/// Reads the NFOs for the item at `video` and applies them to row `id`.
pub fn import_for_item(db: &Database, id: i64, video: &Path) -> Result<NfoApplyReport, String> {
    let (item_nfo, show_nfo) = nfo_paths_for(video);
    let item = item_nfo.as_deref().and_then(read_nfo);
    let show = show_nfo.as_deref().and_then(read_nfo);
    if item.is_none() && show.is_none() {
        return Ok(NfoApplyReport::default());
    }
    let item_dir = item_nfo
        .as_deref()
        .and_then(Path::parent)
        .unwrap_or_else(|| video.parent().unwrap_or(Path::new(".")));
    let show_dir = show_nfo
        .as_deref()
        .and_then(Path::parent)
        .unwrap_or(item_dir);
    let fields = apply(
        db,
        id,
        item.as_ref(),
        show.as_ref(),
        item_dir,
        show_dir,
        sidecar_fanart(video),
    )?;
    Ok(NfoApplyReport {
        nfo_found: true,
        fields_written: fields,
    })
}

/// Applies parsed NFO data to row `id`; returns the number of fields written.
pub fn apply(
    db: &Database,
    id: i64,
    item: Option<&NfoData>,
    show: Option<&NfoData>,
    item_dir: &Path,
    show_dir: &Path,
    fallback_fanart: Option<String>,
) -> Result<usize, String> {
    let row = db
        .conn
        .query_row(
            "SELECT verified, title, year, overview, rating, genre, tmdb_id, imdb_id,
                    poster_path, backdrop_path, media_type
             FROM media_items WHERE id = ?1",
            params![id],
            |row| {
                Ok(RowState {
                    verified: row.get::<_, Option<bool>>(0)?.unwrap_or(false),
                    title: row.get(1)?,
                    year: row.get(2)?,
                    overview: row.get(3)?,
                    rating: row.get(4)?,
                    genre: row.get(5)?,
                    tmdb_id: row.get(6)?,
                    imdb_id: row.get(7)?,
                    poster_path: row.get(8)?,
                    backdrop_path: row.get(9)?,
                    media_type: row.get(10)?,
                })
            },
        )
        .optional()
        .map_err(|error| error.to_string())?
        .ok_or_else(|| format!("Media item {id} was not found"))?;

    let is_episode_file =
        item.is_some_and(|nfo| nfo.kind == "episodedetails") || (item.is_none() && show.is_some());
    // tvshow.nfo describes the show: it only supplies show-level fields.
    let show_level = show.filter(|_| is_episode_file || item.is_none());
    let primary = item;

    let series_title = show_level
        .and_then(|nfo| nfo.title.clone())
        .or_else(|| primary.and_then(|nfo| nfo.show_title.clone()));
    // An episode NFO title is the episode name; keep the show in front.
    let title = primary.and_then(|nfo| match (&nfo.kind[..], &series_title, nfo.episode) {
        ("episodedetails", Some(show), Some(episode)) => Some(
            crate::title_clean::ParsedRelease {
                title: show.clone(),
                season: nfo.season.or(Some(1)),
                episode: Some(episode),
                episode_title: nfo.title.clone(),
                year: None,
            }
            .display_title(),
        ),
        _ => nfo.title.clone(),
    });
    let year = primary.and_then(|nfo| nfo.year);
    let plot = primary.and_then(|nfo| nfo.plot.clone());
    let rating = primary.and_then(|nfo| nfo.rating);
    let genre = primary
        .and_then(NfoData::genre_text)
        .or_else(|| show_level.and_then(NfoData::genre_text));
    let tmdb = primary
        .and_then(|nfo| nfo.tmdb_id.clone())
        .filter(|_| !is_episode_file);
    let imdb = primary
        .and_then(|nfo| nfo.imdb_id.clone())
        .filter(|_| !is_episode_file);
    let poster = primary
        .and_then(|nfo| resolve_artwork(nfo.poster.as_deref(), item_dir))
        .or_else(|| show_level.and_then(|nfo| resolve_artwork(nfo.poster.as_deref(), show_dir)));
    let fanart = primary
        .and_then(|nfo| resolve_artwork(nfo.fanart.as_deref(), item_dir))
        .or_else(|| show_level.and_then(|nfo| resolve_artwork(nfo.fanart.as_deref(), show_dir)))
        .or(fallback_fanart);

    // NFO wins over guesses; a verified row only gets its gaps filled.
    let take_text = |incoming: Option<String>, current: &Option<String>| {
        incoming
            .filter(|_| !row.verified || blank(current))
            .filter(|value| current.as_deref() != Some(value.as_str()))
    };
    let new_title = title
        .filter(|_| !row.verified || row.title.trim().is_empty())
        .filter(|value| value != &row.title);
    let new_year =
        year.filter(|value| (!row.verified || row.year.is_none()) && row.year != Some(*value));
    let new_rating = rating
        .filter(|value| (!row.verified || row.rating.is_none()) && row.rating != Some(*value));
    let new_plot = take_text(plot, &row.overview);
    let new_genre = take_text(genre, &row.genre);
    let new_tmdb = take_text(tmdb, &row.tmdb_id);
    let new_imdb = take_text(imdb, &row.imdb_id);
    // Artwork: an NFO image replaces nothing the library already shows.
    let new_poster = poster.filter(|_| blank(&row.poster_path));
    let new_backdrop = fanart.filter(|_| blank(&row.backdrop_path));
    let new_type = (is_episode_file && row.media_type == "movie" && !row.verified)
        .then(|| "episode".to_string());

    let written = [
        new_title.is_some(),
        new_year.is_some(),
        new_rating.is_some(),
        new_plot.is_some(),
        new_genre.is_some(),
        new_tmdb.is_some(),
        new_imdb.is_some(),
        new_poster.is_some(),
        new_backdrop.is_some(),
        new_type.is_some(),
    ]
    .into_iter()
    .filter(|changed| *changed)
    .count();

    if written > 0 {
        db.conn
            .execute(
                "UPDATE media_items SET
                    title = COALESCE(?1, title),
                    year = COALESCE(?2, year),
                    rating = COALESCE(?3, rating),
                    overview = COALESCE(?4, overview),
                    genre = COALESCE(?5, genre),
                    tmdb_id = COALESCE(?6, tmdb_id),
                    imdb_id = COALESCE(?7, imdb_id),
                    poster_path = COALESCE(?8, poster_path),
                    backdrop_path = COALESCE(?9, backdrop_path),
                    media_type = COALESCE(?10, media_type)
                 WHERE id = ?11",
                params![
                    new_title,
                    new_year,
                    new_rating,
                    new_plot,
                    new_genre,
                    new_tmdb,
                    new_imdb,
                    new_poster,
                    new_backdrop,
                    new_type,
                    id
                ],
            )
            .map_err(|error| error.to_string())?;
    }

    let extras = MediaExtras {
        content_rating: primary
            .and_then(|nfo| nfo.mpaa.clone())
            .or_else(|| show_level.and_then(|nfo| nfo.mpaa.clone())),
        collection_name: primary.and_then(|nfo| nfo.set_name.clone()),
        series_title,
        season: primary.and_then(|nfo| nfo.season),
        episode: primary.and_then(|nfo| nfo.episode),
        ..MediaExtras::default()
    };
    media_extras::merge(db, id, &extras)?;
    Ok(written)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::Database;

    const MOVIE_NFO: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<movie>
    <title>Heat</title>
    <originaltitle>Heat (Original)</originaltitle>
    <ratings>
        <rating name="imdb" max="10"><value>8.3</value></rating>
        <rating name="themoviedb" max="100" default="true"><value>79</value></rating>
    </ratings>
    <year>1995</year>
    <plot><![CDATA[A group of <professional> bank robbers]]> &amp; a detective.</plot>
    <mpaa>Rated R</mpaa>
    <genre>Crime</genre>
    <genre>Drama / Thriller</genre>
    <uniqueid type="imdb" default="true">tt0113277</uniqueid>
    <uniqueid type="tmdb">949</uniqueid>
    <thumb aspect="poster">heat-poster.jpg</thumb>
    <fanart><thumb>https://image.tmdb.org/t/p/original/heat.jpg</thumb></fanart>
    <set><name>Michael Mann Crime</name></set>
    <actor><name>Al Pacino</name><thumb>pacino.jpg</thumb></actor>
</movie>
https://www.imdb.com/title/tt0113277/
"#;

    #[test]
    fn parses_a_full_kodi_movie_nfo() {
        let nfo = parse_nfo(MOVIE_NFO);
        assert_eq!(nfo.kind, "movie");
        assert_eq!(nfo.title.as_deref(), Some("Heat"));
        assert_eq!(nfo.year, Some(1995));
        assert_eq!(
            nfo.plot.as_deref(),
            Some("A group of <professional> bank robbers & a detective.")
        );
        assert_eq!(nfo.rating, Some(7.9));
        assert_eq!(nfo.genres, vec!["Crime", "Drama", "Thriller"]);
        assert_eq!(nfo.imdb_id.as_deref(), Some("tt0113277"));
        assert_eq!(nfo.tmdb_id.as_deref(), Some("949"));
        assert_eq!(nfo.mpaa.as_deref(), Some("Rated R"));
        assert_eq!(nfo.poster.as_deref(), Some("heat-poster.jpg"));
        assert_eq!(
            nfo.fanart.as_deref(),
            Some("https://image.tmdb.org/t/p/original/heat.jpg")
        );
        assert_eq!(nfo.set_name.as_deref(), Some("Michael Mann Crime"));
    }

    #[test]
    fn tolerates_broken_and_url_only_nfos() {
        let broken = parse_nfo("<movie><title>Alien</title><year>1979</yeer><plot>In space");
        assert_eq!(broken.title.as_deref(), Some("Alien"));
        assert_eq!(broken.year, Some(1979));

        let url_only = parse_nfo("https://www.themoviedb.org/movie/348-alien\n");
        assert_eq!(url_only.tmdb_id.as_deref(), Some("348"));
        assert_eq!(url_only.title, None);

        // CinaVault's own writer leaves empty tags; they must not blank fields.
        let sparse = parse_nfo(
            "<movie><title>Up</title><year></year><rating></rating><uniqueid type=\"tmdb\"></uniqueid></movie>",
        );
        assert_eq!(sparse.title.as_deref(), Some("Up"));
        assert_eq!(
            (sparse.year, sparse.rating, sparse.tmdb_id),
            (None, None, None)
        );
    }

    fn temp_root() -> PathBuf {
        let root = std::env::temp_dir().join(format!("cinavault-nfo-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&root).unwrap();
        root
    }

    fn add_row(db: &Database, path: &Path, title: &str, verified: bool) -> i64 {
        db.conn
            .execute(
                "INSERT INTO media_items (title, file_path, media_type, date_added, verified, overview)
                 VALUES (?1, ?2, 'movie', '2026-01-01', ?3, ?4)",
                params![
                    title,
                    path.to_string_lossy(),
                    verified,
                    verified.then_some("User written plot")
                ],
            )
            .unwrap();
        db.conn.last_insert_rowid()
    }

    #[test]
    fn nfo_values_replace_guesses_but_not_verified_fields() {
        let root = temp_root();
        let video = root.join("Heat.1995.1080p.mkv");
        std::fs::write(&video, b"x").unwrap();
        std::fs::write(root.join("Heat.1995.1080p.nfo"), MOVIE_NFO).unwrap();
        std::fs::write(root.join("heat-poster.jpg"), b"jpg").unwrap();
        let db = Database::new(":memory:").unwrap();
        media_extras::ensure_table(&db).unwrap();

        let guessed = add_row(&db, &video, "Heat 1995 1080p", false);
        let report = import_for_item(&db, guessed, &video).unwrap();
        assert!(report.nfo_found);
        let (title, year, poster, backdrop, tmdb): (String, i32, String, String, String) = db
            .conn
            .query_row(
                "SELECT title, year, poster_path, backdrop_path, tmdb_id FROM media_items WHERE id = ?1",
                params![guessed],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)),
            )
            .unwrap();
        assert_eq!(title, "Heat");
        assert_eq!(year, 1995);
        assert_eq!(poster, root.join("heat-poster.jpg").to_string_lossy());
        assert_eq!(backdrop, "https://image.tmdb.org/t/p/original/heat.jpg");
        assert_eq!(tmdb, "949");
        let extras = media_extras::get(&db, guessed).unwrap();
        assert_eq!(extras.content_rating.as_deref(), Some("Rated R"));
        assert_eq!(
            extras.collection_name.as_deref(),
            Some("Michael Mann Crime")
        );
        // Artwork must be an image inside the media folder tree.
        std::fs::write(root.join("notes.txt"), b"x").unwrap();
        assert_eq!(resolve_artwork(Some("notes.txt"), &root), None);
        assert_eq!(resolve_artwork(Some("/etc/hostname"), &root), None);
        assert_eq!(resolve_artwork(Some("http://insecure/p.jpg"), &root), None);
        assert!(resolve_artwork(Some("heat-poster.jpg"), &root).is_some());
        // Applying again changes nothing.
        assert_eq!(
            import_for_item(&db, guessed, &video)
                .unwrap()
                .fields_written,
            0
        );

        db.conn.execute("DELETE FROM media_items", []).unwrap();
        let verified = add_row(&db, &video, "My Heat", true);
        import_for_item(&db, verified, &video).unwrap();
        let (title, overview, year): (String, String, i32) = db
            .conn
            .query_row(
                "SELECT title, overview, year FROM media_items WHERE id = ?1",
                params![verified],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .unwrap();
        assert_eq!(title, "My Heat");
        assert_eq!(overview, "User written plot");
        assert_eq!(year, 1995, "empty fields are still filled");
        std::fs::remove_dir_all(root).ok();
    }

    #[test]
    fn tvshow_nfo_supplies_show_fields_to_episodes() {
        let root = temp_root();
        let season = root.join("Season 1");
        std::fs::create_dir_all(&season).unwrap();
        std::fs::write(
            root.join("tvshow.nfo"),
            "<tvshow><title>Severance</title><genre>Drama</genre><mpaa>TV-MA</mpaa><uniqueid type=\"tmdb\">95396</uniqueid></tvshow>",
        )
        .unwrap();
        let video = season.join("Severance.S01E01.mkv");
        std::fs::write(&video, b"x").unwrap();
        let db = Database::new(":memory:").unwrap();
        media_extras::ensure_table(&db).unwrap();
        let id = add_row(&db, &video, "Severance - S01E01", false);
        import_for_item(&db, id, &video).unwrap();
        let (title, genre, media_type, tmdb): (String, String, String, Option<String>) = db
            .conn
            .query_row(
                "SELECT title, genre, media_type, tmdb_id FROM media_items WHERE id = ?1",
                params![id],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
            )
            .unwrap();
        assert_eq!(
            title, "Severance - S01E01",
            "the show title is not the episode title"
        );
        assert_eq!(genre, "Drama");
        assert_eq!(media_type, "episode");
        assert_eq!(tmdb, None, "a show id is not the episode id");
        let extras = media_extras::get(&db, id).unwrap();
        assert_eq!(extras.series_title.as_deref(), Some("Severance"));
        assert_eq!(extras.content_rating.as_deref(), Some("TV-MA"));
        std::fs::remove_dir_all(root).ok();
    }
}
