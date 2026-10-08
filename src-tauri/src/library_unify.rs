//! Unified library view: every source's copies of the same work collapse into
//! one entry with a single "primary" copy and the list of other copies.
//!
//! Identity rules (in order of strength):
//! 1. `tmdb:<id>` when any copy carries a TMDb id,
//! 2. `imdb:<id>` when any copy carries an IMDb id,
//! 3. `title:<normalized title>|<year or ''>|<media type group>` otherwise.
//!
//! Episodes always carry their season/episode in the key so two different
//! episodes of one show never merge, and the media type group keeps a movie and
//! an adult scene with the same title apart.

use crate::db::MediaItem;
use crate::AppState;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeSet, HashMap};
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;
use tauri::State;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CopyInfo {
    pub id: i64,
    pub file_path: String,
    pub source_id: Option<i64>,
    pub file_size: Option<i64>,
    pub resolution: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnifiedEntry {
    pub work_key: String,
    pub primary: MediaItem,
    pub copies: Vec<CopyInfo>,
    pub copy_count: usize,
    pub source_ids: Vec<i64>,
}

/// A release name reduced to the parts that identify the work.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NormalizedTitle {
    /// Lower-case, accent-free, punctuation-free words separated by one space.
    pub title: String,
    pub year: Option<i32>,
    /// (season, episode) when the name carries an SxxExx / NxNN marker.
    pub episode: Option<(u32, u32)>,
}

const VIDEO_EXTENSIONS: &[&str] = &[
    "mp4", "mkv", "avi", "mov", "wmv", "flv", "webm", "m4v", "mpg", "mpeg", "ts", "m2ts", "vob",
    "ogv", "3gp", "divx", "rm", "rmvb", "asf", "iso", "mp3", "flac", "m4a",
];

/// Tokens that mark the start of release noise. Everything from the first one
/// onwards is dropped (release groups and episode titles usually follow).
const NOISE_TOKENS: &[&str] = &[
    // resolution
    "480p",
    "480i",
    "540p",
    "576p",
    "576i",
    "720p",
    "720i",
    "900p",
    "1080p",
    "1080i",
    "1440p",
    "2160p",
    "4320p",
    "4k",
    "8k",
    "uhd",
    "fhd",
    "qhd", // codecs / bit depth / HDR
    "x264",
    "x265",
    "h264",
    "h265",
    "hevc",
    "av1",
    "avc",
    "xvid",
    "divx",
    "vp9",
    "10bit",
    "8bit",
    "12bit",
    "hdr",
    "hdr10",
    "hdr10plus",
    "dovi",
    "sdr", // sources
    "bluray",
    "bdrip",
    "brrip",
    "bdremux",
    "remux",
    "webdl",
    "webrip",
    "hdtv",
    "pdtv",
    "dvdrip",
    "dvdscr",
    "dvd",
    "hdrip",
    "hdcam",
    "camrip",
    "telesync",
    "amzn",
    "dsnp",
    "hmax",
    // audio
    "aac",
    "aac2",
    "ac3",
    "eac3",
    "dts",
    "dtshd",
    "dtsx",
    "truehd",
    "atmos",
    "flac",
    "mp3",
    "ddp",
    "ddp5",
    "ddp2",
    "dd5",
    "dd2",
    "opus",
    "2ch",
    "6ch",
    "8ch", // release flags
    "proper",
    "repack",
    "rerip",
    "internal",
    "limited",
    "multi",
    "subbed",
    "dubbed",
    "hardsub",
    "nfofix",
    "readnfo",
];

/// Two-word noise markers that only count as noise as a pair ("web dl", "blu ray").
const NOISE_PAIRS: &[(&str, &str)] = &[
    ("web", "dl"),
    ("web", "rip"),
    ("blu", "ray"),
    ("dd", "5"),
    ("ddp", "5"),
    ("dts", "hd"),
    ("h", "264"),
    ("h", "265"),
    ("x", "264"),
    ("x", "265"),
];

fn fold_char(c: char) -> &'static str {
    match c {
        'à' | 'á' | 'â' | 'ã' | 'ä' | 'å' | 'ā' | 'ă' | 'ą' | 'À' | 'Á' | 'Â' | 'Ã' | 'Ä' | 'Å' => {
            "a"
        }
        'æ' | 'Æ' => "ae",
        'ç' | 'ć' | 'č' | 'Ç' | 'Ć' | 'Č' => "c",
        'ď' | 'đ' | 'Ď' | 'Đ' | 'ð' | 'Ð' => "d",
        'è' | 'é' | 'ê' | 'ë' | 'ē' | 'ė' | 'ę' | 'ě' | 'È' | 'É' | 'Ê' | 'Ë' | 'Ě' | 'Ę' => {
            "e"
        }
        'ğ' | 'Ğ' => "g",
        'ì' | 'í' | 'î' | 'ï' | 'ī' | 'ı' | 'Ì' | 'Í' | 'Î' | 'Ï' | 'İ' => "i",
        'ł' | 'ľ' | 'Ł' | 'Ľ' => "l",
        'ñ' | 'ń' | 'ň' | 'Ñ' | 'Ń' | 'Ň' => "n",
        'ò' | 'ó' | 'ô' | 'õ' | 'ö' | 'ø' | 'ō' | 'ő' | 'Ò' | 'Ó' | 'Ô' | 'Õ' | 'Ö' | 'Ø' | 'Ő' => {
            "o"
        }
        'œ' | 'Œ' => "oe",
        'ř' | 'Ř' => "r",
        'ś' | 'š' | 'ş' | 'Ś' | 'Š' | 'Ş' => "s",
        'ß' => "ss",
        'ť' | 'ţ' | 'Ť' | 'Ţ' => "t",
        'þ' | 'Þ' => "th",
        'ù' | 'ú' | 'û' | 'ü' | 'ū' | 'ů' | 'ű' | 'Ù' | 'Ú' | 'Û' | 'Ü' | 'Ů' | 'Ű' => {
            "u"
        }
        'ý' | 'ÿ' | 'Ý' | 'Ÿ' => "y",
        'ž' | 'ź' | 'ż' | 'Ž' | 'Ź' | 'Ż' => "z",
        _ => "",
    }
}

/// Lower-case, fold accents, and turn every non-alphanumeric run into one space.
fn fold_text(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for c in input.chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c.to_ascii_lowercase());
        } else if c == '\'' || c == '’' {
            // "Schindler's" and "Schindlers" are the same title.
        } else {
            let folded = fold_char(c);
            if !folded.is_empty() {
                out.push_str(folded);
            } else if c.is_alphanumeric() {
                out.extend(c.to_lowercase());
            } else {
                out.push(' ');
            }
        }
    }
    out
}

fn strip_extension(name: &str) -> &str {
    if let Some((stem, ext)) = name.rsplit_once('.') {
        if !stem.is_empty() && VIDEO_EXTENSIONS.contains(&ext.to_ascii_lowercase().as_str()) {
            return stem;
        }
    }
    name
}

/// Remove `[...]` and `{...}` groups entirely; parentheses keep their content
/// (it is often the year) but lose the brackets.
fn strip_bracketed(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut depth = 0usize;
    for c in input.chars() {
        match c {
            '[' | '{' => depth += 1,
            ']' | '}' => {
                depth = depth.saturating_sub(1);
                out.push(' ');
            }
            '(' | ')' if depth == 0 => out.push(' '),
            _ if depth == 0 => out.push(c),
            _ => {}
        }
    }
    out
}

fn parse_episode_token(token: &str) -> Option<(u32, u32)> {
    // s01e02, s1e2, s01e02e03 (first episode wins)
    if let Some(rest) = token.strip_prefix('s') {
        let (season, after) = rest.split_once('e')?;
        let episode: String = after.chars().take_while(|c| c.is_ascii_digit()).collect();
        if !season.is_empty()
            && season.len() <= 3
            && season.chars().all(|c| c.is_ascii_digit())
            && !episode.is_empty()
            && episode.len() <= 4
            && after[episode.len()..]
                .chars()
                .all(|c| c == 'e' || c.is_ascii_digit())
        {
            return Some((season.parse().ok()?, episode.parse().ok()?));
        }
    }
    // 1x02
    if let Some((season, episode)) = token.split_once('x') {
        if !season.is_empty()
            && season.len() <= 2
            && episode.len() >= 2
            && episode.len() <= 3
            && season.chars().all(|c| c.is_ascii_digit())
            && episode.chars().all(|c| c.is_ascii_digit())
        {
            return Some((season.parse().ok()?, episode.parse().ok()?));
        }
    }
    None
}

fn year_token(token: &str) -> Option<i32> {
    if token.len() != 4 || !token.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    let year: i32 = token.parse().ok()?;
    let max_year = chrono::Datelike::year(&chrono::Utc::now()) + 2;
    (1888..=max_year).contains(&year).then_some(year)
}

/// Normalize a title or release file name.
///
/// Strips the file extension, bracketed release groups, resolution, codec,
/// source and audio tags; pulls out the year (kept separately) and any
/// SxxExx marker; folds case, accents and punctuation.
pub fn normalize_title(raw: &str) -> NormalizedTitle {
    let base = raw
        .rsplit(|c| c == '/' || c == '\\')
        .next()
        .unwrap_or(raw)
        .trim();
    let base = strip_extension(base);
    let folded = fold_text(&strip_bracketed(base));
    let tokens: Vec<&str> = folded.split_whitespace().collect();

    // Truncate at the first episode marker or release-noise token.
    let mut cut = tokens.len();
    let mut episode = None;
    for (index, token) in tokens.iter().enumerate() {
        if let Some(marker) = parse_episode_token(token) {
            episode = Some(marker);
            cut = index;
            break;
        }
        let pair_noise = tokens
            .get(index + 1)
            .map(|next| NOISE_PAIRS.contains(&(*token, *next)))
            .unwrap_or(false);
        if (index > 0 && NOISE_TOKENS.contains(token)) || (index > 0 && pair_noise) {
            cut = index;
            break;
        }
    }
    // A "Season 1 Episode 2" spelled out form.
    if episode.is_none() {
        for index in 0..tokens.len().saturating_sub(3) {
            if tokens[index] == "season" && tokens[index + 2] == "episode" {
                if let (Ok(season), Ok(ep)) = (tokens[index + 1].parse(), tokens[index + 3].parse())
                {
                    episode = Some((season, ep));
                    cut = cut.min(index);
                    break;
                }
            }
        }
    }

    let kept = &tokens[..cut];
    // The year is the last year-like token that is not the first word
    // ("2001 A Space Odyssey" keeps 2001 as title; "Blade Runner 2049 2017"
    // takes 2017). Anything after the year is edition/release noise.
    let year_index = kept
        .iter()
        .enumerate()
        .skip(1)
        .filter(|(_, token)| year_token(token).is_some())
        .map(|(index, _)| index)
        .last();
    let (title_tokens, year) = match year_index {
        Some(index) => (&kept[..index], year_token(kept[index])),
        None => (kept, None),
    };

    NormalizedTitle {
        title: title_tokens.join(" "),
        year,
        episode,
    }
}

/// Coarse media type bucket used in title keys. Movies, generic videos and
/// adult scenes are separate buckets; every TV spelling is "episode".
pub fn media_type_group(media_type: &str) -> String {
    let lower = media_type.trim().to_ascii_lowercase();
    match lower.as_str() {
        "movie" | "movies" | "film" | "films" => "movie".into(),
        "adult" | "xxx" | "scene" => "adult".into(),
        "video" | "videos" | "clip" | "other" | "" => "video".into(),
        "tv" | "episode" | "series" | "show" | "shows" | "tvshow" | "tv_show" | "season" => {
            "episode".into()
        }
        other => other.into(),
    }
}

fn non_empty(value: &Option<String>) -> Option<&str> {
    value
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
}

fn file_stem(path: &str) -> &str {
    let name = path
        .rsplit(|c| c == '/' || c == '\\')
        .next()
        .unwrap_or(path);
    strip_extension(name)
}

/// The title used for keying: the stored title, falling back to the file name
/// when the title is empty. The episode marker is also read from the path.
fn item_title(item: &MediaItem) -> NormalizedTitle {
    let mut normalized = normalize_title(&item.title);
    if normalized.title.is_empty() {
        let from_file = normalize_title(file_stem(&item.file_path));
        normalized.title = from_file.title;
        normalized.year = normalized.year.or(from_file.year);
        normalized.episode = normalized.episode.or(from_file.episode);
    }
    if normalized.episode.is_none() {
        normalized.episode = normalize_title(file_stem(&item.file_path)).episode;
    }
    normalized
}

fn item_group(item: &MediaItem, normalized: &NormalizedTitle) -> String {
    let group = media_type_group(&item.media_type);
    if normalized.episode.is_some() && (group == "movie" || group == "video") {
        // A scanner that labels everything "movie" still files SxxExx as episodes.
        "episode".into()
    } else {
        group
    }
}

fn episode_suffix(normalized: &NormalizedTitle) -> String {
    match normalized.episode {
        Some((season, episode)) => format!("s{season:02}e{episode:02}"),
        None => String::new(),
    }
}

fn title_key(item: &MediaItem) -> String {
    let normalized = item_title(item);
    let group = item_group(item, &normalized);
    let year = item
        .year
        .or(normalized.year)
        .map(|year| year.to_string())
        .unwrap_or_default();
    if normalized.title.is_empty() {
        // Nothing to identify the work by: keep the copy on its own.
        return format!("path:{}", item.file_path);
    }
    if group == "episode" {
        format!(
            "title:{}|{}|episode:{}",
            normalized.title,
            year,
            episode_suffix(&normalized)
        )
    } else {
        format!("title:{}|{}|{}", normalized.title, year, group)
    }
}

/// Provider-id keys (TMDb, IMDb) of one copy. Episodes append their
/// season/episode because scanners often store the show's id on every episode;
/// an episode without a known number gets no id key at all.
fn id_keys(item: &MediaItem) -> (Option<String>, Option<String>) {
    let normalized = item_title(item);
    let suffix = if item_group(item, &normalized) == "episode" {
        let suffix = episode_suffix(&normalized);
        if suffix.is_empty() {
            return (None, None);
        }
        format!("|{suffix}")
    } else {
        String::new()
    };
    let key = |prefix: &str, value: &Option<String>| {
        non_empty(value).map(|id| format!("{prefix}:{}{}", id.to_ascii_lowercase(), suffix))
    };
    (key("tmdb", &item.tmdb_id), key("imdb", &item.imdb_id))
}

fn strong_key(item: &MediaItem) -> Option<String> {
    let (tmdb, imdb) = id_keys(item);
    tmdb.or(imdb)
}

/// Identity key of a single copy (before cross-copy grouping).
#[allow(dead_code)] // Part of the module's tested API; group_items computes keys in bulk.
pub fn work_key(item: &MediaItem) -> String {
    strong_key(item).unwrap_or_else(|| title_key(item))
}

/// Vertical resolution of a copy, from its resolution field or its file name.
pub fn resolution_rank(item: &MediaItem) -> u32 {
    fn parse(value: &str) -> u32 {
        let lower = value.to_ascii_lowercase();
        if let Some((width, height)) = lower.split_once('x') {
            let width: u32 = width.trim().parse().unwrap_or(0);
            let height: u32 = height.trim().parse().unwrap_or(0);
            if width > 0 && height > 0 {
                // Scope/ultrawide encodes: 3840x1608 is still a 4K copy.
                return height.max(width * 9 / 16);
            }
        }
        if lower.contains("4320") || lower.contains("8k") {
            return 4320;
        }
        if lower.contains("2160") || lower.contains("4k") || lower.contains("uhd") {
            return 2160;
        }
        for (marker, rank) in [
            ("1440", 1440),
            ("1080", 1080),
            ("900p", 900),
            ("720", 720),
            ("576", 576),
            ("540", 540),
            ("480", 480),
            ("360", 360),
        ] {
            if lower.contains(marker) {
                return rank;
            }
        }
        lower
            .trim_end_matches('p')
            .trim()
            .parse::<u32>()
            .unwrap_or(0)
    }
    let from_field = item.resolution.as_deref().map(parse).unwrap_or(0);
    if from_field > 0 {
        return from_field;
    }
    let name = file_stem(&item.file_path).to_ascii_lowercase();
    for token in name.split(|c: char| !c.is_ascii_alphanumeric()) {
        let rank = match token {
            "4320p" | "8k" => 4320,
            "2160p" | "4k" | "uhd" => 2160,
            "1440p" => 1440,
            "1080p" | "1080i" => 1080,
            "720p" => 720,
            "576p" => 576,
            "480p" => 480,
            _ => 0,
        };
        if rank > 0 {
            return rank;
        }
    }
    0
}

/// Index of the best copy: highest resolution, then largest file, then one
/// with a poster, then the lowest id (stable).
pub fn choose_primary(items: &[MediaItem]) -> usize {
    let mut best = 0usize;
    for index in 1..items.len() {
        let candidate = &items[index];
        let current = &items[best];
        let candidate_rank = (
            resolution_rank(candidate),
            candidate.file_size.unwrap_or(0),
            non_empty(&candidate.poster_path).is_some(),
            std::cmp::Reverse(candidate.id.unwrap_or(i64::MAX)),
        );
        let current_rank = (
            resolution_rank(current),
            current.file_size.unwrap_or(0),
            non_empty(&current.poster_path).is_some(),
            std::cmp::Reverse(current.id.unwrap_or(i64::MAX)),
        );
        if candidate_rank > current_rank {
            best = index;
        }
    }
    best
}

/// Fill the primary copy's missing metadata from the other copies, in order.
fn fill_metadata(primary: &mut MediaItem, others: &[&MediaItem]) {
    fn fill_text(target: &mut Option<String>, source: &Option<String>) {
        if non_empty(target).is_none() {
            if let Some(value) = non_empty(source) {
                *target = Some(value.to_string());
            }
        }
    }
    for other in others {
        fill_text(&mut primary.poster_path, &other.poster_path);
        fill_text(&mut primary.backdrop_path, &other.backdrop_path);
        fill_text(&mut primary.overview, &other.overview);
        fill_text(&mut primary.genre, &other.genre);
        if primary.rating.is_none() {
            primary.rating = other.rating;
        }
        if primary.year.is_none() {
            primary.year = other.year;
        }
    }
}

/// Union-find over copies that also tracks, per set, the TMDb and IMDb id the
/// set carries. A union that would put two different TMDb ids (or two
/// different IMDb ids) into one set is refused, so a copy with mismatched ids
/// (e.g. TMDb 1 + IMDb tt1 next to TMDb 2 + IMDb tt1) cannot glue unrelated
/// works together transitively.
struct DisjointSet {
    parent: Vec<usize>,
    tmdb: Vec<Option<String>>,
    imdb: Vec<Option<String>>,
}

impl DisjointSet {
    fn new(tmdb: Vec<Option<String>>, imdb: Vec<Option<String>>) -> Self {
        DisjointSet {
            parent: (0..tmdb.len()).collect(),
            tmdb,
            imdb,
        }
    }

    fn find(&mut self, index: usize) -> usize {
        let mut root = index;
        while self.parent[root] != root {
            root = self.parent[root];
        }
        let mut cursor = index;
        while self.parent[cursor] != root {
            let next = self.parent[cursor];
            self.parent[cursor] = root;
            cursor = next;
        }
        root
    }

    /// Merge the sets of `a` and `b` unless their ids conflict. Returns
    /// whether the two copies now share a set.
    fn union(&mut self, a: usize, b: usize) -> bool {
        fn conflicts(left: &Option<String>, right: &Option<String>) -> bool {
            matches!((left, right), (Some(left), Some(right)) if left != right)
        }
        let (a, b) = (self.find(a), self.find(b));
        if a == b {
            return true;
        }
        if conflicts(&self.tmdb[a], &self.tmdb[b]) || conflicts(&self.imdb[a], &self.imdb[b]) {
            return false;
        }
        let (root, child) = (a.min(b), a.max(b));
        self.parent[child] = root;
        if self.tmdb[root].is_none() {
            self.tmdb[root] = self.tmdb[child].take();
        }
        if self.imdb[root].is_none() {
            self.imdb[root] = self.imdb[child].take();
        }
        true
    }
}

/// `title:<t>|<year>|<group>` with the year blanked; None for non-title keys.
fn yearless_title_key(key: &str) -> Option<String> {
    let rest = key.strip_prefix("title:")?;
    let mut parts = rest.splitn(3, '|');
    let title = parts.next()?;
    let _year = parts.next()?;
    let group = parts.next()?;
    Some(format!("title:{title}||{group}"))
}

/// Group copies into works. Copies sharing a TMDb or IMDb id merge; copies
/// without ids join the id-carrying work with the same title key when exactly
/// one such work exists, otherwise they group by title key among themselves.
pub fn group_items(items: Vec<MediaItem>) -> Vec<UnifiedEntry> {
    let count = items.len();
    let strong: Vec<Option<String>> = items.iter().map(strong_key).collect();
    let titles: Vec<String> = items.iter().map(title_key).collect();
    let ids: Vec<(Option<String>, Option<String>)> = items.iter().map(id_keys).collect();
    let tmdb: Vec<Option<String>> = ids.iter().map(|(tmdb, _)| tmdb.clone()).collect();
    let imdb: Vec<Option<String>> = ids.iter().map(|(_, imdb)| imdb.clone()).collect();
    let mut sets = DisjointSet::new(tmdb.clone(), imdb.clone());

    let mut first_by_key: HashMap<&str, usize> = HashMap::new();
    for index in 0..count {
        for key in [tmdb[index].as_deref(), imdb[index].as_deref()]
            .into_iter()
            .flatten()
        {
            match first_by_key.get(key) {
                Some(&first) => {
                    // Refused when the ids conflict: the copies stay apart.
                    sets.union(first, index);
                }
                None => {
                    first_by_key.insert(key, index);
                }
            }
        }
    }

    // Title keys of id-carrying works.
    let mut strong_roots_by_title: HashMap<&str, BTreeSet<usize>> = HashMap::new();
    for index in 0..count {
        if strong[index].is_some() {
            let root = sets.find(index);
            strong_roots_by_title
                .entry(titles[index].as_str())
                .or_default()
                .insert(root);
        }
    }
    let mut weak_first_by_title: HashMap<&str, usize> = HashMap::new();
    for index in 0..count {
        if strong[index].is_some() {
            continue;
        }
        let title = titles[index].as_str();
        match strong_roots_by_title.get(title) {
            Some(roots) if roots.len() == 1 => {
                let root = *roots.iter().next().expect("one root");
                sets.union(root, index);
            }
            Some(_) => {
                // Ambiguous between several identified works: keep apart.
            }
            None => match weak_first_by_title.get(title) {
                Some(&first) => {
                    sets.union(first, index);
                }
                None => {
                    weak_first_by_title.insert(title, index);
                }
            },
        }
    }

    // A copy whose title key has no year joins the single same-title work that
    // does have a year ("Heat" + "Heat (1995)"), but not when several exist.
    let mut yeared_roots: HashMap<String, BTreeSet<usize>> = HashMap::new();
    for index in 0..count {
        if let Some(yearless) = yearless_title_key(&titles[index]) {
            if yearless != titles[index] {
                let root = sets.find(index);
                yeared_roots.entry(yearless).or_default().insert(root);
            }
        }
    }
    for index in 0..count {
        if strong[index].is_some() {
            continue;
        }
        let Some(yearless) = yearless_title_key(&titles[index]) else {
            continue;
        };
        if yearless != titles[index] {
            continue;
        }
        if let Some(roots) = yeared_roots.get(&yearless) {
            if roots.len() == 1 {
                let root = *roots.iter().next().expect("one root");
                sets.union(root, index);
            }
        }
    }

    let mut clusters: Vec<(usize, Vec<usize>)> = Vec::new();
    let mut cluster_of_root: HashMap<usize, usize> = HashMap::new();
    for index in 0..count {
        let root = sets.find(index);
        let slot = *cluster_of_root.entry(root).or_insert_with(|| {
            clusters.push((root, Vec::new()));
            clusters.len() - 1
        });
        clusters[slot].1.push(index);
    }

    let mut entries = Vec::with_capacity(clusters.len());
    for (_, members) in clusters {
        let owned: Vec<MediaItem> = members.iter().map(|&index| items[index].clone()).collect();
        let primary_offset = choose_primary(&owned);
        let primary_index = members[primary_offset];

        let key = strong[primary_index]
            .clone()
            .or_else(|| members.iter().find_map(|&index| tmdb[index].clone()))
            .or_else(|| members.iter().find_map(|&index| imdb[index].clone()))
            .unwrap_or_else(|| {
                let has_year =
                    |key: &String| yearless_title_key(key).is_some_and(|yearless| &yearless != key);
                std::iter::once(primary_index)
                    .chain(members.iter().copied())
                    .map(|index| &titles[index])
                    .find(|key| has_year(key))
                    .unwrap_or(&titles[primary_index])
                    .clone()
            });

        let mut primary = items[primary_index].clone();
        let others: Vec<&MediaItem> = members
            .iter()
            .filter(|&&index| index != primary_index)
            .map(|&index| &items[index])
            .collect();
        fill_metadata(&mut primary, &others);

        let mut ordered = vec![primary_index];
        ordered.extend(members.iter().copied().filter(|&i| i != primary_index));
        let copies: Vec<CopyInfo> = ordered
            .iter()
            .map(|&index| {
                let item = &items[index];
                CopyInfo {
                    id: item.id.unwrap_or_default(),
                    file_path: item.file_path.clone(),
                    source_id: item.source_id,
                    file_size: item.file_size,
                    resolution: item.resolution.clone(),
                }
            })
            .collect();
        let source_ids: Vec<i64> = members
            .iter()
            .filter_map(|&index| items[index].source_id)
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect();
        entries.push(UnifiedEntry {
            work_key: key,
            copy_count: copies.len(),
            copies,
            source_ids,
            primary,
        });
    }
    entries
}

const FINGERPRINT_CHUNK: u64 = 4 * 1024 * 1024;

/// Fast content fingerprint: SHA-256 of the file size plus the first, middle
/// and last 4 MiB. Files up to 12 MiB are hashed whole.
pub fn content_fingerprint(path: &Path) -> Result<String, String> {
    let mut file =
        File::open(path).map_err(|error| format!("Cannot open {}: {error}", path.display()))?;
    let size = file
        .metadata()
        .map_err(|error| format!("Cannot stat {}: {error}", path.display()))?
        .len();
    let mut hasher = Sha256::new();
    hasher.update(size.to_le_bytes());

    let mut read_range = |offset: u64, length: u64, hasher: &mut Sha256| -> Result<(), String> {
        file.seek(SeekFrom::Start(offset))
            .map_err(|error| error.to_string())?;
        let mut remaining = length;
        let mut buffer = vec![0u8; 64 * 1024];
        while remaining > 0 {
            let want = remaining.min(buffer.len() as u64) as usize;
            let read = file
                .read(&mut buffer[..want])
                .map_err(|error| error.to_string())?;
            if read == 0 {
                break;
            }
            hasher.update(&buffer[..read]);
            remaining -= read as u64;
        }
        Ok(())
    };

    if size <= FINGERPRINT_CHUNK * 3 {
        read_range(0, size, &mut hasher)?;
    } else {
        read_range(0, FINGERPRINT_CHUNK, &mut hasher)?;
        read_range(
            size / 2 - FINGERPRINT_CHUNK / 2,
            FINGERPRINT_CHUNK,
            &mut hasher,
        )?;
        read_range(size - FINGERPRINT_CHUNK, FINGERPRINT_CHUNK, &mut hasher)?;
    }
    Ok(hasher
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect())
}

fn sort_entries(entries: &mut [UnifiedEntry]) {
    entries.sort_by(|a, b| {
        b.primary
            .date_added
            .cmp(&a.primary.date_added)
            .then_with(|| a.primary.title.cmp(&b.primary.title))
            .then_with(|| a.work_key.cmp(&b.work_key))
    });
}

/// Every library copy grouped into one entry per work.
#[tauri::command]
pub fn get_unified_library(
    state: State<'_, AppState>,
    media_type: Option<String>,
) -> Result<Vec<UnifiedEntry>, String> {
    let items = {
        let db = state.db.lock().map_err(|error| error.to_string())?;
        db.get_media_items_visible(
            None,
            None,
            None,
            crate::parental::active_filter(&db).as_deref(),
        )
        .map_err(|error| error.to_string())?
    };
    let wanted = media_type
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty() && !value.eq_ignore_ascii_case("all"))
        .map(media_type_group);
    let items: Vec<MediaItem> = items
        .into_iter()
        .filter(|item| {
            !(crate::edition::STORE_SAFE && media_type_group(&item.media_type) == "adult")
        })
        .filter(|item| {
            wanted
                .as_deref()
                .map(|group| media_type_group(&item.media_type) == group)
                .unwrap_or(true)
        })
        .collect();
    let mut entries = group_items(items);
    sort_entries(&mut entries);
    Ok(entries)
}

#[cfg(test)]
pub(crate) fn test_item(id: i64, title: &str, file_path: &str, media_type: &str) -> MediaItem {
    MediaItem {
        id: Some(id),
        title: title.into(),
        file_path: file_path.into(),
        media_type: media_type.into(),
        year: None,
        rating: None,
        overview: None,
        poster_path: None,
        backdrop_path: None,
        genre: None,
        duration: None,
        file_size: None,
        resolution: None,
        codec: None,
        verified: false,
        watched: false,
        favorite: false,
        date_added: "2026-01-01T00:00:00Z".into(),
        last_played: None,
        tmdb_id: None,
        imdb_id: None,
        source_id: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(id: i64, title: &str, path: &str, media_type: &str) -> MediaItem {
        test_item(id, title, path, media_type)
    }

    #[test]
    fn normalize_strips_release_noise_and_keeps_year() {
        let n = normalize_title("The.Matrix.1999.1080p.BluRay.x264.DTS-GROUP.mkv");
        assert_eq!(n.title, "the matrix");
        assert_eq!(n.year, Some(1999));
        assert_eq!(n.episode, None);

        let n = normalize_title("[YTS] The Matrix (1999) [2160p] [WEB-DL] HEVC AAC");
        assert_eq!(n.title, "the matrix");
        assert_eq!(n.year, Some(1999));

        let n = normalize_title("the_matrix_1999_720p_web-dl_h264");
        assert_eq!(n.title, "the matrix");
        assert_eq!(n.year, Some(1999));

        let n = normalize_title("The Matrix 4K Remux Atmos");
        assert_eq!(n.title, "the matrix");
        assert_eq!(n.year, None);
    }

    #[test]
    fn normalize_folds_case_accents_and_punctuation() {
        assert_eq!(normalize_title("Amélie (2001)").title, "amelie");
        assert_eq!(normalize_title("AMELIE.2001").title, "amelie");
        assert_eq!(normalize_title("Schindler's List").title, "schindlers list");
        assert_eq!(
            normalize_title("Spider-Man: No Way Home").title,
            "spider man no way home"
        );
        assert_eq!(
            normalize_title("Léon: The Professional").title,
            "leon the professional"
        );
    }

    #[test]
    fn normalize_edge_cases() {
        let empty = normalize_title("");
        assert_eq!(empty.title, "");
        assert_eq!(empty.year, None);
        assert_eq!(normalize_title("   ...   ").title, "");

        let no_year = normalize_title("Heat");
        assert_eq!(no_year.title, "heat");
        assert_eq!(no_year.year, None);

        // A leading year is the title, not a release year.
        let odyssey = normalize_title("2001 A Space Odyssey 1968");
        assert_eq!(odyssey.title, "2001 a space odyssey");
        assert_eq!(odyssey.year, Some(1968));

        // Charlotte's Web keeps "web" because it is not followed by dl/rip.
        assert_eq!(
            normalize_title("Charlottes.Web.2006.DVDRip").title,
            "charlottes web"
        );

        // Unknown extensions are not stripped (and the dot becomes a space).
        assert_eq!(normalize_title("Mr. Nobody").title, "mr nobody");
    }

    #[test]
    fn normalize_parses_episode_markers() {
        let n = normalize_title("Breaking.Bad.S01E02.Cats.in.the.Bag.720p.HDTV.x264.mkv");
        assert_eq!(n.title, "breaking bad");
        assert_eq!(n.episode, Some((1, 2)));
        assert_eq!(normalize_title("Breaking Bad 1x03").episode, Some((1, 3)));
        assert_eq!(
            normalize_title("Breaking Bad Season 2 Episode 4").episode,
            Some((2, 4))
        );
        assert_eq!(normalize_title("Sexy Beast").episode, None);
    }

    #[test]
    fn work_key_forms() {
        let mut movie = item(
            1,
            "The.Matrix.1999.1080p",
            "/m/The.Matrix.1999.1080p.mkv",
            "movie",
        );
        assert_eq!(work_key(&movie), "title:the matrix|1999|movie");
        movie.imdb_id = Some("tt0133093".into());
        assert_eq!(work_key(&movie), "imdb:tt0133093");
        movie.tmdb_id = Some("603".into());
        assert_eq!(work_key(&movie), "tmdb:603");

        let no_year = item(2, "Heat", "/m/Heat.mkv", "movie");
        assert_eq!(work_key(&no_year), "title:heat||movie");

        let adult = item(3, "Heat", "/a/Heat.mp4", "adult");
        assert_eq!(work_key(&adult), "title:heat||adult");

        let episode = item(4, "Show S01E02", "/tv/Show.S01E02.mkv", "tv");
        assert_eq!(work_key(&episode), "title:show||episode:s01e02");

        // Empty title falls back to the file name.
        let untitled = item(5, "", "/m/Heat.1995.mkv", "movie");
        assert_eq!(work_key(&untitled), "title:heat|1995|movie");
        // Nothing usable at all keeps the copy apart.
        let nothing = item(6, "", "/m/[x].mkv", "movie");
        assert_eq!(work_key(&nothing), "path:/m/[x].mkv");
    }

    #[test]
    fn group_merges_copies_of_one_movie_across_sources() {
        let mut a = item(1, "The Matrix", "/a/The.Matrix.1999.1080p.mkv", "movie");
        a.year = Some(1999);
        a.source_id = Some(1);
        let mut b = item(
            2,
            "The.Matrix.1999.2160p.UHD.mkv",
            "/b/The.Matrix.1999.2160p.UHD.mkv",
            "movie",
        );
        b.source_id = Some(2);
        let c = item(3, "Heat", "/a/Heat.mkv", "movie");
        let entries = group_items(vec![a, b, c]);
        assert_eq!(entries.len(), 2);
        let matrix = entries
            .iter()
            .find(|entry| entry.work_key.contains("matrix"))
            .unwrap();
        assert_eq!(matrix.copy_count, 2);
        assert_eq!(matrix.source_ids, vec![1, 2]);
        assert_eq!(matrix.copies[0].id, matrix.primary.id.unwrap());
    }

    #[test]
    fn movie_and_adult_scene_with_same_title_do_not_merge() {
        let movie = item(1, "Heat", "/m/Heat.mkv", "movie");
        let adult = item(2, "Heat", "/x/Heat.mp4", "adult");
        assert_eq!(group_items(vec![movie, adult]).len(), 2);
    }

    #[test]
    fn different_episodes_never_merge() {
        let mut e1 = item(1, "Show", "/tv/Show.S01E01.mkv", "tv");
        let mut e2 = item(2, "Show", "/tv/Show.S01E02.mkv", "tv");
        // Even when both carry the show's TMDb id.
        e1.tmdb_id = Some("1396".into());
        e2.tmdb_id = Some("1396".into());
        let e2_copy = item(3, "Show.S01E02.720p", "/nas/Show.S01E02.720p.mkv", "tv");
        let entries = group_items(vec![e1, e2, e2_copy]);
        assert_eq!(entries.len(), 2);
        let keys: BTreeSet<_> = entries.iter().map(|e| e.work_key.clone()).collect();
        assert!(keys.contains("tmdb:1396|s01e01"));
        assert!(keys.contains("tmdb:1396|s01e02"));
        let ep2 = entries
            .iter()
            .find(|e| e.work_key == "tmdb:1396|s01e02")
            .unwrap();
        assert_eq!(ep2.copy_count, 2);
    }

    #[test]
    fn tmdb_id_beats_title() {
        // Same title and year, different TMDb ids: two works.
        let mut a = item(1, "Dune", "/a/Dune.mkv", "movie");
        a.tmdb_id = Some("438631".into());
        let mut b = item(2, "Dune", "/b/Dune.mkv", "movie");
        b.tmdb_id = Some("841".into());
        assert_eq!(group_items(vec![a.clone(), b.clone()]).len(), 2);

        // Different titles, same TMDb id: one work.
        let mut c = item(3, "Dune Part One", "/c/Dune.Part.One.mkv", "movie");
        c.tmdb_id = Some("438631".into());
        let entries = group_items(vec![a.clone(), c]);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].work_key, "tmdb:438631");

        // An id-less copy whose title matches exactly one identified work joins it.
        let d = item(4, "Dune", "/d/Dune.mkv", "movie");
        let entries = group_items(vec![a.clone(), d.clone()]);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].copy_count, 2);
        // ...but stays apart when the title is ambiguous between two works.
        assert_eq!(group_items(vec![a, b, d]).len(), 3);
    }

    #[test]
    fn conflicting_provider_ids_never_merge_transitively() {
        // Same IMDb id but different TMDb ids: the ids disagree, keep apart.
        let mut a = item(1, "Alpha", "/a/Alpha.mkv", "movie");
        a.tmdb_id = Some("1".into());
        a.imdb_id = Some("tt1".into());
        let mut b = item(2, "Beta", "/b/Beta.mkv", "movie");
        b.tmdb_id = Some("2".into());
        b.imdb_id = Some("tt1".into());
        let entries = group_items(vec![a, b]);
        assert_eq!(entries.len(), 2);
        assert!(entries.iter().all(|entry| entry.copy_count == 1));

        // A bridge copy with consistent ids still joins TMDb-only and
        // IMDb-only copies of the same work into one card.
        let mut tmdb_only = item(3, "Gamma", "/c/Gamma.mkv", "movie");
        tmdb_only.tmdb_id = Some("1".into());
        let mut bridge = item(4, "Gamma Remux", "/d/Gamma.Remux.mkv", "movie");
        bridge.tmdb_id = Some("1".into());
        bridge.imdb_id = Some("tt9".into());
        let mut imdb_only = item(5, "Gamma Alt", "/e/Gamma.Alt.mkv", "movie");
        imdb_only.imdb_id = Some("tt9".into());
        let entries = group_items(vec![tmdb_only, bridge, imdb_only]);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].copy_count, 3);

        // A bridge must not glue two different TMDb works together.
        let mut x = item(6, "Xray", "/f/Xray.mkv", "movie");
        x.tmdb_id = Some("10".into());
        x.imdb_id = Some("tt10".into());
        let mut y = item(7, "Yankee", "/g/Yankee.mkv", "movie");
        y.tmdb_id = Some("20".into());
        let mut glue = item(8, "Zulu", "/h/Zulu.mkv", "movie");
        glue.tmdb_id = Some("20".into());
        glue.imdb_id = Some("tt10".into());
        let entries = group_items(vec![x, y, glue]);
        assert_eq!(entries.len(), 2);
        let tmdb_10 = entries
            .iter()
            .find(|entry| entry.work_key == "tmdb:10")
            .unwrap();
        assert_eq!(tmdb_10.copy_count, 1);
    }

    #[test]
    fn primary_prefers_4k_over_1080p_then_size_then_poster() {
        let mut hd = item(1, "Movie", "/a/Movie.1080p.mkv", "movie");
        hd.file_size = Some(30_000_000_000);
        let mut uhd = item(2, "Movie", "/b/Movie.mkv", "movie");
        uhd.resolution = Some("3840x2160".into());
        uhd.file_size = Some(20_000_000_000);
        assert_eq!(choose_primary(&[hd.clone(), uhd.clone()]), 1);

        // Resolution inferred from the file name when the field is empty.
        let uhd_name = item(3, "Movie", "/c/Movie.2160p.mkv", "movie");
        assert_eq!(choose_primary(&[hd.clone(), uhd_name]), 1);

        let mut small = item(4, "Movie", "/d/Movie.1080p.mkv", "movie");
        small.file_size = Some(1_000);
        assert_eq!(choose_primary(&[small.clone(), hd.clone()]), 1);

        let mut with_poster = small.clone();
        with_poster.id = Some(5);
        with_poster.poster_path = Some("/p.jpg".into());
        assert_eq!(choose_primary(&[small, with_poster]), 1);
        assert_eq!(choose_primary(&[]), 0);
    }

    #[test]
    fn metadata_filled_from_secondary_copy() {
        let mut uhd = item(1, "Movie", "/a/Movie.2160p.mkv", "movie");
        uhd.overview = Some("  ".into());
        let mut hd = item(2, "Movie", "/b/Movie.1080p.mkv", "movie");
        hd.poster_path = Some("/posters/movie.jpg".into());
        hd.backdrop_path = Some("/posters/back.jpg".into());
        hd.overview = Some("A film.".into());
        hd.rating = Some(7.5);
        hd.genre = Some("Drama".into());
        hd.year = Some(2020);
        let entries = group_items(vec![uhd, hd]);
        // The year-less copy joins the only same-title work that has a year.
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].primary.year, Some(2020));
        assert_eq!(entries[0].work_key, "title:movie|2020|movie");

        // Ambiguous between two years: the year-less copy stays apart.
        let mut y1 = item(7, "Heat", "/a/Heat.mkv", "movie");
        y1.year = Some(1995);
        let mut y2 = item(8, "Heat", "/b/Heat.mkv", "movie");
        y2.year = Some(1986);
        let bare = item(9, "Heat", "/c/Heat.mkv", "movie");
        assert_eq!(group_items(vec![y1, y2, bare]).len(), 3);

        let mut uhd = item(1, "Movie", "/a/Movie.2160p.mkv", "movie");
        uhd.year = Some(2020);
        let mut hd = item(2, "Movie", "/b/Movie.1080p.mkv", "movie");
        hd.year = Some(2020);
        hd.poster_path = Some("/posters/movie.jpg".into());
        hd.overview = Some("A film.".into());
        hd.rating = Some(7.5);
        hd.genre = Some("Drama".into());
        hd.backdrop_path = Some("/posters/back.jpg".into());
        let entries = group_items(vec![uhd, hd]);
        assert_eq!(entries.len(), 1);
        let primary = &entries[0].primary;
        assert_eq!(primary.id, Some(1));
        assert_eq!(primary.poster_path.as_deref(), Some("/posters/movie.jpg"));
        assert_eq!(primary.backdrop_path.as_deref(), Some("/posters/back.jpg"));
        assert_eq!(primary.overview.as_deref(), Some("A film."));
        assert_eq!(primary.rating, Some(7.5));
        assert_eq!(primary.genre.as_deref(), Some("Drama"));
        assert_eq!(primary.year, Some(2020));
        assert_eq!(primary.file_path, "/a/Movie.2160p.mkv");
    }

    #[test]
    fn media_type_groups() {
        assert_eq!(media_type_group("Movie"), "movie");
        assert_eq!(media_type_group("adult"), "adult");
        assert_eq!(media_type_group("video"), "video");
        assert_eq!(media_type_group("tv"), "episode");
        assert_eq!(media_type_group("music"), "music");
    }

    #[test]
    fn content_fingerprint_matches_identical_content_only() {
        let dir = std::env::temp_dir().join(format!("cinavault-fp-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let small_a = dir.join("a.bin");
        let small_b = dir.join("b.bin");
        let small_c = dir.join("c.bin");
        std::fs::write(&small_a, b"same bytes").unwrap();
        std::fs::write(&small_b, b"same bytes").unwrap();
        std::fs::write(&small_c, b"diff bytes").unwrap();
        let fa = content_fingerprint(&small_a).unwrap();
        assert_eq!(fa, content_fingerprint(&small_b).unwrap());
        assert_ne!(fa, content_fingerprint(&small_c).unwrap());

        // Large file: a change inside a sampled region changes the fingerprint.
        let size = (FINGERPRINT_CHUNK * 4) as usize;
        let mut big = vec![7u8; size];
        let big_a = dir.join("big_a.bin");
        std::fs::write(&big_a, &big).unwrap();
        let before = content_fingerprint(&big_a).unwrap();
        big[size - 1] = 8;
        let big_b = dir.join("big_b.bin");
        std::fs::write(&big_b, &big).unwrap();
        assert_ne!(before, content_fingerprint(&big_b).unwrap());
        assert!(content_fingerprint(&dir.join("missing.bin")).is_err());
        std::fs::remove_dir_all(&dir).ok();
    }
}
