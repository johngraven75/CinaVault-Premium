//! Build edition gate. The Microsoft Store edition is compiled with the
//! `store-safe` Cargo feature, which keeps adult sources, providers and
//! commands out of the back end as well as the UI.

pub const STORE_SAFE: bool = cfg!(feature = "store-safe");

pub const STORE_SAFE_REFUSAL: &str =
    "Adult content is not available in the Microsoft Store edition";

/// Err when this build is the Store edition; call at the top of adult-only commands.
pub fn ensure_adult_allowed() -> Result<(), String> {
    if STORE_SAFE {
        Err(STORE_SAFE_REFUSAL.into())
    } else {
        Ok(())
    }
}

/// True when a source of this type must be refused or skipped in this build.
pub fn source_type_blocked(source_type: &str) -> bool {
    STORE_SAFE && source_type.trim().eq_ignore_ascii_case("adult")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn edition_gate_matches_feature() {
        assert_eq!(ensure_adult_allowed().is_err(), STORE_SAFE);
        assert_eq!(source_type_blocked(" Adult "), STORE_SAFE);
        assert!(!source_type_blocked("folder"));
    }
}
