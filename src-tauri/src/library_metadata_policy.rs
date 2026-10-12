//! Which metadata providers the library may call.
//!
//! The full edition currently looks up library metadata from the adult
//! providers only (ThePornDB and StashDB, plus the keyless adult bridges).
//! The regular movie/TV providers (TMDB, OMDb and the keyless Cinemeta and
//! TVMaze lookups) are never called by scans, the post-scan lookup, bulk
//! enrichment or a single-title metadata check, even when a key is saved.
//!
//! The Microsoft Store edition cannot use adult providers at all, so it keeps
//! the regular providers; otherwise its library would get no metadata.

/// Regular (non-adult) metadata providers, by normalized provider key.
pub const STANDARD_PROVIDERS: &[&str] = &[
    "tmdb",
    "tmdb_images",
    "omdb",
    "cinemeta",
    "tvmaze",
    "tvdb",
    "trakt",
    "fanart",
    "fanarttv",
];

/// Shown wherever a regular provider is listed but not used.
pub const NOT_IN_USE_MESSAGE: &str =
    "Not in use: library metadata comes from adult providers (ThePornDB, StashDB) only";

/// True when the regular movie/TV providers may be called for the library.
pub fn standard_providers_enabled() -> bool {
    crate::edition::STORE_SAFE
}

fn normalize(provider: &str) -> String {
    match provider.trim().to_ascii_lowercase().as_str() {
        "themoviedb" | "themoviedb_images" => "tmdb".to_string(),
        "open_movie_db" | "openmoviedb" => "omdb".to_string(),
        "theporndb" => "tpdb".to_string(),
        other => other.to_string(),
    }
}

/// True for a regular (non-adult) metadata provider key or alias.
pub fn is_standard_provider(provider: &str) -> bool {
    STANDARD_PROVIDERS.contains(&normalize(provider).as_str())
}

/// True when the library may call this provider in the current build.
pub fn provider_allowed(provider: &str) -> bool {
    standard_providers_enabled() || !is_standard_provider(provider)
}

/// Err with a readable reason when the library may not call this provider.
pub fn ensure_provider_allowed(provider: &str) -> Result<(), String> {
    if provider_allowed(provider) {
        Ok(())
    } else {
        Err(format!("{}: {NOT_IN_USE_MESSAGE}", provider.trim()))
    }
}

/// Drops saved keys for providers the library may not call, so a stored
/// TMDB or OMDb key can never drive a lookup.
pub fn retain_allowed_keys<V>(keys: &mut std::collections::HashMap<String, V>) {
    keys.retain(|provider, _| provider_allowed(provider));
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn adult_providers_are_always_allowed() {
        for provider in ["tpdb", "ThePornDB", "stashdb", "pgma", "iafd"] {
            assert!(provider_allowed(provider), "{provider} must stay active");
        }
    }

    #[test]
    fn regular_providers_follow_the_edition() {
        for provider in [
            "tmdb",
            "TheMovieDB",
            "tmdb_images",
            "omdb",
            "OpenMovieDB",
            "cinemeta",
            "tvmaze",
        ] {
            assert!(
                is_standard_provider(provider),
                "{provider} is a regular provider"
            );
            assert_eq!(
                provider_allowed(provider),
                crate::edition::STORE_SAFE,
                "{provider}"
            );
            assert_eq!(
                ensure_provider_allowed(provider).is_ok(),
                crate::edition::STORE_SAFE
            );
        }
    }

    #[test]
    fn saved_regular_keys_are_dropped_in_the_full_edition() {
        let mut keys: HashMap<String, String> = [
            ("tmdb", "a"),
            ("themoviedb", "b"),
            ("omdb", "c"),
            ("tpdb", "d"),
            ("stashdb", "e"),
        ]
        .into_iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect();
        retain_allowed_keys(&mut keys);
        let mut left = keys.keys().cloned().collect::<Vec<_>>();
        left.sort();
        if crate::edition::STORE_SAFE {
            assert_eq!(left.len(), 5);
        } else {
            assert_eq!(left, ["stashdb", "tpdb"]);
        }
    }
}
