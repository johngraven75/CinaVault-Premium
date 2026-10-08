//! Release-name cleaning for the "smart_match" switch.
//!
//! Turns names like `The.Matrix.1999.1080p.BluRay.x264-GROUP` or
//! `Breaking_Bad_S01E02_720p_WEB-DL` into a title, a year and an episode
//! marker. The scanner uses it for new titles and the metadata lookups use it
//! for their search query. Pure string work: no I/O, no database.

use regex::Regex;
use std::path::Path;
use std::sync::OnceLock;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ParsedRelease {
    /// Movie title, or the show name for an episode.
    pub title: String,
    pub year: Option<i32>,
    pub season: Option<u32>,
    pub episode: Option<u32>,
    /// Words between the episode marker and the release tags, if any.
    pub episode_title: Option<String>,
}

impl ParsedRelease {
    pub fn is_episode(&self) -> bool {
        self.episode.is_some()
    }

    /// Library title: `Show - S01E02 - Episode Title`, or the movie title.
    pub fn display_title(&self) -> String {
        match (self.season, self.episode) {
            (season, Some(episode)) => {
                let marker = format!("S{:02}E{:02}", season.unwrap_or(1), episode);
                let mut parts = Vec::new();
                if !self.title.is_empty() {
                    parts.push(self.title.clone());
                }
                parts.push(marker);
                if let Some(name) = self.episode_title.as_ref().filter(|n| !n.is_empty()) {
                    parts.push(name.clone());
                }
                parts.join(" - ")
            }
            _ => self.title.clone(),
        }
    }
}

/// Words that always mark the end of the title in a release name.
const RELEASE_TAGS: &[&str] = &[
    // resolution
    "480p",
    "480i",
    "540p",
    "576p",
    "576i",
    "720p",
    "1080p",
    "1080i",
    "1440p",
    "2160p",
    "4320p",
    "4k",
    "8k",
    "uhd", // source
    "webdl",
    "web-dl",
    "webrip",
    "web-rip",
    "hdtv",
    "pdtv",
    "sdtv",
    "dvdrip",
    "dvdscr",
    "dvd5",
    "dvd9",
    "bluray",
    "blu-ray",
    "bdrip",
    "brrip",
    "bdremux",
    "remux",
    "hdrip",
    "hdcam",
    "camrip",
    "telesync",
    "amzn",
    "dsnp",
    "hmax",
    "atvp", // video codec
    "x264",
    "x265",
    "h264",
    "h265",
    "h.264",
    "h.265",
    "hevc",
    "avc",
    "xvid",
    "divx",
    "av1",
    "vp9",
    "10bit",
    "8bit",
    "hi10p", // dynamic range
    "hdr",
    "hdr10",
    "hdr10+",
    "hdr10plus",
    "dovi",
    "sdr", // audio
    "dts",
    "dts-hd",
    "dtshd",
    "dts-x",
    "truehd",
    "atmos",
    "aac",
    "aac2",
    "ac3",
    "eac3",
    "ddp",
    "dd2",
    "dd5",
    "ddp2",
    "ddp5",
    "dd+",
    "flac",
    "lpcm",
    "5.1",
    "7.1", // release flags
    "repack",
    "rerip",
    "multisubs",
    "readnfo",
];

/// Tags that are also ordinary words ("Charlotte's Web", "The Complete
/// Works"): they end the title only next to another tag.
const WEAK_TAGS: &[&str] = &[
    "web",
    "dvd",
    "dv",
    "nf",
    "dd",
    "mp3",
    "opus",
    "2.0",
    "proper",
    "extended",
    "unrated",
    "uncut",
    "remastered",
    "imax",
    "internal",
    "limited",
    "multi",
    "subbed",
    "dubbed",
    "dual",
    "complete",
    "fhd",
];

/// Generic file names that say nothing about the title.
const GENERIC_STEMS: &[&str] = &[
    "movie", "video", "film", "feature", "sample", "main", "title",
];

fn bracket_regex() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"\[[^\]]*\]|\{[^}]*\}").expect("bracket regex"))
}

fn paren_regex() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"\(([^)]*)\)").expect("paren regex"))
}

fn sxxeyy_regex() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"(?i)^s(\d{1,2})[ ._-]?e(\d{1,3})(?:[-e]+\d{1,3})*$").expect("SxxEyy regex")
    })
}

fn nxnn_regex() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"^(\d{1,2})x(\d{2,3})$").expect("NxNN regex"))
}

fn year_regex() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"^(?:19|20)\d{2}$").expect("year regex"))
}

fn is_weak_tag(token: &str) -> bool {
    WEAK_TAGS.contains(&token.to_ascii_lowercase().as_str())
}

fn is_release_tag(token: &str) -> bool {
    let lower = token.to_ascii_lowercase();
    if RELEASE_TAGS.contains(&lower.as_str()) {
        return true;
    }
    // "x264-GROUP", "WEB-DL", "DDP5.1": judge by the part before the dash.
    if let Some((head, _)) = lower.split_once('-') {
        if !head.is_empty() && RELEASE_TAGS.contains(&head) {
            return true;
        }
    }
    // Audio layouts glued to a codec: "DDP5.1", "AAC2.0", "DTS-HD.MA".
    let alpha: String = lower
        .chars()
        .take_while(|c| c.is_ascii_alphabetic())
        .collect();
    alpha.len() >= 2
        && alpha.len() < lower.len()
        && ["ddp", "dd", "aac", "ac", "eac", "dts", "truehd", "flac"].contains(&alpha.as_str())
        && lower[alpha.len()..]
            .chars()
            .all(|c| c.is_ascii_digit() || c == '.' || c == '+')
}

fn episode_marker(token: &str) -> Option<(u32, u32)> {
    let captures = sxxeyy_regex()
        .captures(token)
        .or_else(|| nxnn_regex().captures(token))?;
    let season = captures.get(1)?.as_str().parse().ok()?;
    let episode = captures.get(2)?.as_str().parse().ok()?;
    Some((season, episode))
}

fn tidy(tokens: &[String]) -> String {
    let words: Vec<&str> = tokens
        .iter()
        .map(|token| token.trim_matches(|c: char| c == '-' || c == ',' || c == ':'))
        .filter(|token| !token.is_empty())
        .collect();
    let joined = words.join(" ");
    if joined.chars().any(|c| c.is_ascii_uppercase()) {
        joined
    } else {
        title_case(&joined)
    }
}

fn title_case(value: &str) -> String {
    value
        .split(' ')
        .map(|word| {
            let mut chars = word.chars();
            match chars.next() {
                Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// Splits a release name into title, year and episode marker.
pub fn parse_release_name(raw: &str) -> ParsedRelease {
    let without_brackets = bracket_regex().replace_all(raw, " ");
    // Keep a bare "(2019)" as a year token; drop parentheses full of tags.
    let without_parens = paren_regex().replace_all(&without_brackets, |caps: &regex::Captures| {
        let inner = caps.get(1).map(|m| m.as_str()).unwrap_or_default();
        let inner_tokens: Vec<&str> = inner.split([' ', '.', '_']).collect();
        if inner_tokens.iter().any(|token| is_release_tag(token)) {
            " ".to_string()
        } else {
            format!(" {inner} ")
        }
    });
    let spaced = without_parens.replace(['.', '_'], " ");
    let tokens: Vec<String> = spaced.split_whitespace().map(str::to_string).collect();

    let mut episode_at = None;
    let mut tag_at = None;
    for (index, token) in tokens.iter().enumerate() {
        if episode_at.is_none() {
            if let Some(marker) = episode_marker(token) {
                episode_at = Some((index, marker));
                continue;
            }
        }
        if tag_at.is_some() || index == 0 {
            continue;
        }
        let neighbour_is_tag = |at: usize| {
            tokens
                .get(at)
                .is_some_and(|next| is_release_tag(next) || is_weak_tag(next))
        };
        if is_release_tag(token) || (is_weak_tag(token) && neighbour_is_tag(index + 1)) {
            tag_at = Some(index);
        }
    }

    let hard_end = [episode_at.map(|(index, _)| index), tag_at]
        .into_iter()
        .flatten()
        .min()
        .unwrap_or(tokens.len());
    // The last year before the tags, never the first word ("2012", "1917").
    let year_at = (1..hard_end)
        .rev()
        .find(|index| year_regex().is_match(&tokens[*index]));
    let title_end = year_at.unwrap_or(hard_end);

    let mut parsed = ParsedRelease {
        title: tidy(&tokens[..title_end]),
        year: year_at.and_then(|index| tokens[index].parse().ok()),
        ..ParsedRelease::default()
    };

    if let Some((index, (season, episode))) = episode_at {
        parsed.season = Some(season);
        parsed.episode = Some(episode);
        let after_end = tag_at.filter(|tag| *tag > index).unwrap_or(tokens.len());
        let name = tidy(&tokens[index + 1..after_end]);
        if !name.is_empty() && !year_regex().is_match(&name) {
            parsed.episode_title = Some(name);
        }
    }
    parsed
}

/// Parses a media file path: the file name first, then the folder name when
/// the file name is generic ("movie.mkv") or only a marker ("S01E02.mkv").
pub fn parse_path(path: &Path) -> ParsedRelease {
    let stem = path
        .file_stem()
        .map(|stem| stem.to_string_lossy().to_string())
        .unwrap_or_default();
    let mut parsed = parse_release_name(&stem);
    let generic = parsed.title.is_empty()
        || GENERIC_STEMS.contains(&parsed.title.to_ascii_lowercase().as_str());
    if generic {
        let mut folders = path.ancestors().skip(1).filter_map(|dir| dir.file_name());
        // Episodes usually sit in "Show/Season 1/"; skip the season folder.
        for folder in folders.by_ref().take(2) {
            let folder = parse_release_name(&folder.to_string_lossy());
            let lower = folder.title.to_ascii_lowercase();
            if folder.title.is_empty() || lower.starts_with("season ") || lower == "specials" {
                continue;
            }
            parsed.title = folder.title;
            parsed.year = parsed.year.or(folder.year);
            break;
        }
    }
    if parsed.title.is_empty() {
        parsed.title = stem.replace(['.', '_'], " ").trim().to_string();
    }
    parsed
}

/// The metadata search query: the cleaned title, or the show name for an
/// episode. Falls back to the file name when the stored title is unusable.
pub fn search_query(title: &str, file_path: &str) -> String {
    let from_title = parse_release_name(title);
    if !from_title.title.is_empty() && !title.trim().eq_ignore_ascii_case("unknown") {
        return from_title.title;
    }
    let file_name = file_path
        .rsplit(['/', '\\'])
        .find(|segment| !segment.is_empty())
        .unwrap_or(file_path);
    parse_path(Path::new(file_name)).title
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_release_tags_and_finds_the_year() {
        let parsed = parse_release_name("The.Matrix.1999.1080p.BluRay.x264-SPARKS");
        assert_eq!(parsed.title, "The Matrix");
        assert_eq!(parsed.year, Some(1999));
        assert!(!parsed.is_episode());

        let parsed = parse_release_name("Dune Part Two (2024) [2160p HDR10 DTS-HD MA 7.1] REMUX");
        assert_eq!(parsed.title, "Dune Part Two");
        assert_eq!(parsed.year, Some(2024));

        let parsed = parse_release_name("Blade_Runner_2049_2017_2160p_WEB-DL_DDP5.1_HEVC");
        assert_eq!(parsed.title, "Blade Runner 2049");
        assert_eq!(parsed.year, Some(2017));

        let parsed = parse_release_name("[YTS.MX] Parasite.2019.720p.WEBRip.AAC");
        assert_eq!(parsed.title, "Parasite");
        assert_eq!(parsed.year, Some(2019));
    }

    #[test]
    fn a_leading_number_stays_in_the_title() {
        assert_eq!(parse_release_name("1917.2019.1080p.x265").title, "1917");
        assert_eq!(parse_release_name("1917.2019.1080p.x265").year, Some(2019));
        let parsed = parse_release_name("2012.720p.BluRay");
        assert_eq!(parsed.title, "2012");
        assert_eq!(parsed.year, None);
    }

    #[test]
    fn ordinary_words_that_look_like_tags_stay_in_the_title() {
        let parsed = parse_release_name("Charlotte's Web (1973)");
        assert_eq!(parsed.title, "Charlotte's Web");
        assert_eq!(parsed.year, Some(1973));
        assert_eq!(
            parse_release_name("The.Complete.Works.EXTENDED.1080p").title,
            "The Complete Works"
        );
        assert_eq!(
            parse_release_name("Severance.S02E01.WEB.h264-GROUP").episode_title,
            None
        );
    }

    #[test]
    fn hyphenated_titles_survive() {
        let parsed = parse_release_name("Spider-Man.No.Way.Home.2021.HDR.2160p");
        assert_eq!(parsed.title, "Spider-Man No Way Home");
        assert_eq!(parsed.year, Some(2021));
    }

    #[test]
    fn episode_markers_split_show_and_episode() {
        let parsed = parse_release_name("Breaking.Bad.S01E02.Cats.in.the.Bag.720p.HDTV.x264");
        assert_eq!(parsed.title, "Breaking Bad");
        assert_eq!(parsed.season, Some(1));
        assert_eq!(parsed.episode, Some(2));
        assert_eq!(parsed.episode_title.as_deref(), Some("Cats in the Bag"));
        assert_eq!(
            parsed.display_title(),
            "Breaking Bad - S01E02 - Cats in the Bag"
        );

        let parsed = parse_release_name("the_office_3x05_web-dl");
        assert_eq!(parsed.title, "The Office");
        assert_eq!((parsed.season, parsed.episode), (Some(3), Some(5)));
        assert_eq!(parsed.episode_title, None);
        assert_eq!(parsed.display_title(), "The Office - S03E05");

        let parsed = parse_release_name("Doctor Who (2005) - s10e01e02 - The Pilot");
        assert_eq!(parsed.title, "Doctor Who");
        assert_eq!(parsed.year, Some(2005));
        assert_eq!((parsed.season, parsed.episode), (Some(10), Some(1)));
        assert_eq!(parsed.episode_title.as_deref(), Some("The Pilot"));
    }

    #[test]
    fn generic_file_names_use_the_folder() {
        let parsed = parse_path(Path::new("/media/Movies/Heat (1995)/movie.mkv"));
        assert_eq!(parsed.title, "Heat");
        assert_eq!(parsed.year, Some(1995));

        let parsed = parse_path(Path::new("/tv/Severance/Season 1/S01E03.mkv"));
        assert_eq!(parsed.title, "Severance");
        assert_eq!((parsed.season, parsed.episode), (Some(1), Some(3)));
    }

    #[test]
    fn search_query_prefers_the_show_name() {
        assert_eq!(
            search_query("Breaking Bad - S01E02 - Cats in the Bag", "x.mkv"),
            "Breaking Bad"
        );
        assert_eq!(
            search_query("Unknown", "D:\\Films\\Alien.1979.Directors.Cut.1080p.mkv"),
            "Alien"
        );
        assert_eq!(search_query("Heat", "/m/heat.mkv"), "Heat");
    }
}
