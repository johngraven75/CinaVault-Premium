//! Caching for the embedded server ("cdn_cache" switch): an in-memory LRU of
//! artwork bytes, strong ETags, and If-None-Match matching.

use axum::body::Bytes;
use sha2::{Digest, Sha256};
use std::collections::HashMap;

pub const DEFAULT_ARTWORK_CACHE_MB: u64 = 64;
pub const DEFAULT_LIBRARY_MAX_AGE: u32 = 30;
pub const ARTWORK_CACHE_CONTROL: &str = "public, max-age=86400, immutable";

/// Strong ETag for a response body.
pub fn etag_for(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let hex: String = digest[..16].iter().map(|b| format!("{b:02x}")).collect();
    format!("\"{hex}\"")
}

/// Whether an If-None-Match header value matches `etag` (weak comparison, as
/// RFC 9110 requires for If-None-Match).
pub fn if_none_match(header: Option<&str>, etag: &str) -> bool {
    let Some(header) = header else {
        return false;
    };
    let bare = |tag: &str| tag.trim().trim_start_matches("W/").to_string();
    header
        .split(',')
        .map(str::trim)
        .any(|candidate| candidate == "*" || bare(candidate) == bare(etag))
}

#[derive(Clone, Debug)]
pub struct CachedArtwork {
    pub bytes: Bytes,
    pub mime: String,
    pub etag: String,
}

/// Least-recently-used cache bounded by total bytes.
#[derive(Debug)]
pub struct ArtworkLru {
    capacity: usize,
    used: usize,
    tick: u64,
    entries: HashMap<String, (CachedArtwork, u64)>,
}

impl ArtworkLru {
    pub fn new(capacity_bytes: usize) -> Self {
        Self {
            capacity: capacity_bytes,
            used: 0,
            tick: 0,
            entries: HashMap::new(),
        }
    }

    /// Changes the byte budget, evicting as needed.
    pub fn set_capacity(&mut self, capacity_bytes: usize) {
        self.capacity = capacity_bytes;
        self.evict_to(self.capacity);
    }

    pub fn get(&mut self, key: &str) -> Option<CachedArtwork> {
        self.tick += 1;
        let tick = self.tick;
        self.entries.get_mut(key).map(|(entry, used)| {
            *used = tick;
            entry.clone()
        })
    }

    /// Stores artwork unless it alone exceeds the budget.
    pub fn insert(&mut self, key: String, artwork: CachedArtwork) {
        let size = artwork.bytes.len();
        if size > self.capacity {
            return;
        }
        if let Some((old, _)) = self.entries.remove(&key) {
            self.used -= old.bytes.len();
        }
        self.evict_to(self.capacity - size);
        self.tick += 1;
        self.used += size;
        self.entries.insert(key, (artwork, self.tick));
    }

    fn evict_to(&mut self, budget: usize) {
        while self.used > budget {
            let Some(oldest) = self
                .entries
                .iter()
                .min_by_key(|(_, (_, used))| *used)
                .map(|(key, _)| key.clone())
            else {
                break;
            };
            if let Some((entry, _)) = self.entries.remove(&oldest) {
                self.used -= entry.bytes.len();
            }
        }
    }
}

#[cfg(test)]
impl ArtworkLru {
    fn used_bytes(&self) -> usize {
        self.used
    }

    fn len(&self) -> usize {
        self.entries.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn art(size: usize) -> CachedArtwork {
        let bytes = vec![1u8; size];
        CachedArtwork {
            etag: etag_for(&bytes),
            bytes: Bytes::from(bytes),
            mime: "image/jpeg".into(),
        }
    }

    #[test]
    fn etags_are_stable_and_match_if_none_match_lists() {
        let tag = etag_for(b"poster");
        assert_eq!(tag, etag_for(b"poster"));
        assert_ne!(tag, etag_for(b"poster2"));
        assert_eq!(tag.len(), 34);
        assert!(if_none_match(Some(&tag), &tag));
        assert!(if_none_match(Some(&format!("\"x\", W/{tag}")), &tag));
        assert!(if_none_match(Some("*"), &tag));
        assert!(!if_none_match(Some("\"other\""), &tag));
        assert!(!if_none_match(None, &tag));
    }

    #[test]
    fn lru_evicts_least_recently_used_within_the_byte_budget() {
        let mut cache = ArtworkLru::new(100);
        cache.insert("a".into(), art(40));
        cache.insert("b".into(), art(40));
        assert!(cache.get("a").is_some()); // a is now fresher than b
        cache.insert("c".into(), art(40));
        assert!(cache.get("b").is_none(), "b was least recently used");
        assert!(cache.get("a").is_some());
        assert!(cache.get("c").is_some());
        assert_eq!(cache.used_bytes(), 80);
        cache.insert("huge".into(), art(101));
        assert!(cache.get("huge").is_none(), "larger than the whole budget");
        cache.insert("a".into(), art(10));
        assert_eq!(cache.used_bytes(), 50);
        cache.set_capacity(20);
        assert_eq!(cache.len(), 1);
        assert!(cache.get("a").is_some());
        assert!(cache.used_bytes() <= 20);
    }
}
