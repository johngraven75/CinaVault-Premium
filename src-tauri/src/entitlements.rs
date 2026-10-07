//! CinaVault Plus entitlements (the paywall).
//!
//! Free: local drives, playback, LAN access to the embedded server.
//! CinaVault Plus ($9.99/month) unlocks: external libraries (NAS, cloud,
//! network shares), downloads, adult metadata, and remote access from another
//! network. Plus features are locked from first launch (strict free tier). Each
//! install can opt in once to a [`TRIAL_DAYS`]-day Plus trial with the
//! `start_trial` command; nothing starts it automatically (John, 2026-10-01).
//!
//! Licenses are offline tokens: `CVL1.<base64url payload>.<base64url signature>`.
//! The payload is JSON `{license_id, email, plan, issued_at, expires_at}` with
//! RFC 3339 timestamps and `plan == "cinavault_plus"`. The Ed25519 signature
//! covers the ASCII bytes `CVL1.<base64url payload>` (everything before the
//! second dot). The public key is baked in at build time from
//! `CINAVAULT_LICENSE_PUBLIC_KEY` (base64, 32 bytes). A build without it
//! verifies no license at all (fail closed); the trial still works.

use crate::db::Database;
use crate::AppState;
use base64::Engine;
use chrono::{DateTime, Duration, Utc};
use ed25519_dalek::{Signature, VerifyingKey};
use serde::{Deserialize, Serialize};
use tauri::State;

/// Length of the free trial. One constant: change it here only.
/// The trial is opt-in (`start_trial`), once per install; 0 disables it.
pub const TRIAL_DAYS: i64 = 30;
pub const PLAN_ID: &str = "cinavault_plus";
pub const PLAN_NAME: &str = "CinaVault Plus";
pub const PRICE_LABEL: &str = "$9.99/month";
pub const TOKEN_PREFIX: &str = "CVL1";

const SETTING_TRIAL_STARTED_AT: &str = "entitlements_trial_started_at";
const SETTING_LICENSE_TOKEN: &str = "entitlements_license_token";

const LICENSE_PUBLIC_KEY: Option<&str> = option_env!("CINAVAULT_LICENSE_PUBLIC_KEY");
const CHECKOUT_URL: Option<&str> = option_env!("CINAVAULT_CHECKOUT_URL");

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Feature {
    ExternalLibraries,
    Downloads,
    AdultMetadata,
    RemoteAccess,
}

impl Feature {
    pub fn id(self) -> &'static str {
        match self {
            Feature::ExternalLibraries => "external_libraries",
            Feature::Downloads => "downloads",
            Feature::AdultMetadata => "adult_metadata",
            Feature::RemoteAccess => "remote_access",
        }
    }

    fn label(self) -> &'static str {
        match self {
            Feature::ExternalLibraries => "NAS, cloud and network-share libraries",
            Feature::Downloads => "Downloads",
            Feature::AdultMetadata => "Adult metadata",
            Feature::RemoteAccess => "Remote access from outside your home network",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FeatureFlags {
    pub external_libraries: bool,
    pub downloads: bool,
    pub adult_metadata: bool,
    pub remote_access: bool,
}

impl FeatureFlags {
    fn all(enabled: bool) -> Self {
        FeatureFlags {
            external_libraries: enabled,
            downloads: enabled,
            adult_metadata: enabled,
            remote_access: enabled,
        }
    }

    pub fn has(&self, feature: Feature) -> bool {
        match feature {
            Feature::ExternalLibraries => self.external_libraries,
            Feature::Downloads => self.downloads,
            Feature::AdultMetadata => self.adult_metadata,
            Feature::RemoteAccess => self.remote_access,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Entitlements {
    /// "free" | "trial" | "plus"
    pub plan: String,
    pub trial_days_left: Option<i64>,
    pub trial_ends_at: Option<String>,
    pub license_email: Option<String>,
    pub license_expires_at: Option<String>,
    pub features: FeatureFlags,
    pub checkout_url: Option<String>,
    pub price_label: String,
    pub licensing_configured: bool,
    /// True when this install has never used its trial and no license is active.
    pub trial_available: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LicensePayload {
    pub license_id: String,
    pub email: String,
    pub plan: String,
    pub issued_at: String,
    pub expires_at: String,
}

/// A license that verified, with parsed expiry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifiedLicense {
    pub payload: LicensePayload,
    pub expires_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrialState {
    /// The user has never opted in to the trial.
    NotStarted,
    Active {
        days_left: i64,
        ends_at: DateTime<Utc>,
    },
    Expired {
        ends_at: Option<DateTime<Utc>>,
    },
}

fn decode_base64(value: &str) -> Result<Vec<u8>, String> {
    let trimmed = value.trim().trim_end_matches('=');
    base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(trimmed)
        .or_else(|_| base64::engine::general_purpose::STANDARD_NO_PAD.decode(trimmed))
        .map_err(|_| "is not valid base64".to_string())
}

fn decode_public_key(encoded: &str) -> Result<VerifyingKey, String> {
    let bytes = decode_base64(encoded).map_err(|e| format!("License public key {e}"))?;
    let bytes: [u8; 32] = bytes
        .try_into()
        .map_err(|_| "License public key must be 32 bytes".to_string())?;
    VerifyingKey::from_bytes(&bytes).map_err(|_| "License public key is invalid".to_string())
}

/// The build's public key, if one was configured and is well formed.
fn configured_public_key() -> Option<VerifyingKey> {
    LICENSE_PUBLIC_KEY
        .map(str::trim)
        .filter(|key| !key.is_empty())
        .and_then(|key| decode_public_key(key).ok())
}

fn checkout_url() -> Option<String> {
    CHECKOUT_URL
        .map(str::trim)
        .filter(|url| !url.is_empty())
        .map(str::to_string)
}

fn parse_time(value: &str, field: &str) -> Result<DateTime<Utc>, String> {
    DateTime::parse_from_rfc3339(value.trim())
        .map(|time| time.with_timezone(&Utc))
        .map_err(|_| format!("License {field} is not a valid RFC 3339 time"))
}

/// Verify a license token against `public_key` at time `now`.
pub fn verify_license_token_with_key(
    token: &str,
    public_key: &VerifyingKey,
    now: DateTime<Utc>,
) -> Result<VerifiedLicense, String> {
    let token = token.trim();
    let mut parts = token.split('.');
    let (Some(prefix), Some(payload_b64), Some(signature_b64), None) =
        (parts.next(), parts.next(), parts.next(), parts.next())
    else {
        return Err("License key is not in the CVL1.<payload>.<signature> format".into());
    };
    if prefix != TOKEN_PREFIX {
        return Err("License key version is not supported".into());
    }
    let signature_bytes =
        decode_base64(signature_b64).map_err(|e| format!("License signature {e}"))?;
    let signature = Signature::from_slice(&signature_bytes)
        .map_err(|_| "License signature has the wrong length".to_string())?;
    let signed = &token[..prefix.len() + 1 + payload_b64.len()];
    public_key
        .verify_strict(signed.as_bytes(), &signature)
        .map_err(|_| "License key signature is not valid".to_string())?;

    let payload_bytes = decode_base64(payload_b64).map_err(|e| format!("License payload {e}"))?;
    let payload: LicensePayload = serde_json::from_slice(&payload_bytes)
        .map_err(|_| "License payload is not valid JSON".to_string())?;
    if payload.plan != PLAN_ID {
        return Err(format!("License is not for {PLAN_NAME}"));
    }
    let issued_at = parse_time(&payload.issued_at, "issue time")?;
    let expires_at = parse_time(&payload.expires_at, "expiry time")?;
    if expires_at <= issued_at {
        return Err("License expiry is not after its issue time".into());
    }
    if issued_at > now + Duration::days(1) {
        return Err("License was issued in the future; check this computer's clock".into());
    }
    if now >= expires_at {
        return Err(format!(
            "License expired on {}",
            expires_at.format("%Y-%m-%d")
        ));
    }
    Ok(VerifiedLicense {
        payload,
        expires_at,
    })
}

/// Verify with the key compiled into this build. Fails closed without one.
pub fn verify_license_token(token: &str, now: DateTime<Utc>) -> Result<VerifiedLicense, String> {
    verify_with_optional_key(token, configured_public_key().as_ref(), now)
}

fn verify_with_optional_key(
    token: &str,
    public_key: Option<&VerifyingKey>,
    now: DateTime<Utc>,
) -> Result<VerifiedLicense, String> {
    match public_key {
        Some(key) => verify_license_token_with_key(token, key, now),
        None => Err(
            "License activation is not configured in this build of CinaVault, so no license key can be verified".into(),
        ),
    }
}

/// Trial state for a trial that started at `started` (None = unreadable start,
/// which is treated as expired). A clock earlier than the start means the
/// clock was rolled back: the trial counts as expired.
pub fn trial_state(started: Option<DateTime<Utc>>, now: DateTime<Utc>) -> TrialState {
    trial_state_for_days(started, now, TRIAL_DAYS)
}

/// [`trial_state`] for an explicit trial length; `trial_days <= 0` means no trial.
pub fn trial_state_for_days(
    started: Option<DateTime<Utc>>,
    now: DateTime<Utc>,
    trial_days: i64,
) -> TrialState {
    if trial_days <= 0 {
        return TrialState::Expired { ends_at: None };
    }
    let Some(started) = started else {
        return TrialState::Expired { ends_at: None };
    };
    let ends_at = started + Duration::days(trial_days);
    if now < started || now >= ends_at {
        return TrialState::Expired {
            ends_at: Some(ends_at),
        };
    }
    let remaining = ends_at - now;
    let mut days_left = remaining.num_days();
    if remaining - Duration::days(days_left) > Duration::zero() {
        days_left += 1;
    }
    TrialState::Active {
        days_left: days_left.clamp(1, trial_days),
        ends_at,
    }
}

/// Pure combination of trial and license into the reported entitlements.
pub fn compute_entitlements(
    trial: TrialState,
    license: Option<&VerifiedLicense>,
    licensing_configured: bool,
    checkout_url: Option<String>,
) -> Entitlements {
    let base = |plan: &str, enabled: bool| Entitlements {
        plan: plan.into(),
        trial_days_left: None,
        trial_ends_at: None,
        license_email: None,
        license_expires_at: None,
        features: FeatureFlags::all(enabled),
        checkout_url: checkout_url.clone(),
        price_label: PRICE_LABEL.into(),
        licensing_configured,
        trial_available: false,
    };
    if let Some(license) = license {
        let mut plus = base("plus", true);
        plus.license_email = Some(license.payload.email.clone());
        plus.license_expires_at = Some(license.expires_at.to_rfc3339());
        return plus;
    }
    match trial {
        TrialState::Active { days_left, ends_at } => {
            let mut trial = base("trial", true);
            trial.trial_days_left = Some(days_left);
            trial.trial_ends_at = Some(ends_at.to_rfc3339());
            trial
        }
        TrialState::NotStarted => {
            let mut free = base("free", false);
            free.trial_available = TRIAL_DAYS > 0;
            free
        }
        TrialState::Expired { ends_at } => {
            let mut free = base("free", false);
            free.trial_days_left = Some(0);
            free.trial_ends_at = ends_at.map(|time| time.to_rfc3339());
            free
        }
    }
}

fn read_setting(db: &Database, key: &str) -> Result<Option<String>, String> {
    db.get_setting_data(key)
        .map(|value| value.filter(|value| !value.trim().is_empty()))
        .map_err(|error| format!("Unable to read entitlement settings: {error}"))
}

/// Stored trial start: None when the user never opted in, Some(None) when the
/// stored value is unreadable (treated as a used, expired trial).
fn read_trial_start(db: &Database) -> Result<Option<Option<DateTime<Utc>>>, String> {
    Ok(read_setting(db, SETTING_TRIAL_STARTED_AT)?.map(|stored| {
        DateTime::parse_from_rfc3339(stored.trim())
            .ok()
            .map(|time| time.with_timezone(&Utc))
    }))
}

/// Opt in to the one-time trial. Refused when the trial was already used, a
/// license is active, or trials are disabled.
fn start_trial_at(
    db: &Database,
    public_key: Option<&VerifyingKey>,
    now: DateTime<Utc>,
) -> Result<Entitlements, String> {
    let current = load_entitlements_with_key(db, public_key, now)?;
    if current.plan == "plus" {
        return Err(format!("{PLAN_NAME} is already active on this install."));
    }
    if !current.trial_available {
        return Err(format!(
            "The {TRIAL_DAYS}-day {PLAN_NAME} trial has already been used on this install."
        ));
    }
    db.set_setting_data(SETTING_TRIAL_STARTED_AT, &now.to_rfc3339())
        .map_err(|error| format!("Unable to record trial start: {error}"))?;
    load_entitlements_with_key(db, public_key, now)
}

fn load_entitlements_with_key(
    db: &Database,
    public_key: Option<&VerifyingKey>,
    now: DateTime<Utc>,
) -> Result<Entitlements, String> {
    let trial = match read_trial_start(db)? {
        None => TrialState::NotStarted,
        Some(started) => trial_state(started, now),
    };
    let license = read_setting(db, SETTING_LICENSE_TOKEN)?
        .and_then(|token| verify_with_optional_key(&token, public_key, now).ok());
    Ok(compute_entitlements(
        trial,
        license.as_ref(),
        public_key.is_some(),
        checkout_url(),
    ))
}

pub fn load_entitlements(db: &Database) -> Result<Entitlements, String> {
    load_entitlements_with_key(db, configured_public_key().as_ref(), Utc::now())
}

fn paywall_error(feature: Feature, entitlements: &Entitlements) -> String {
    let reason = if entitlements.trial_ends_at.is_some() {
        "your free trial has ended"
    } else {
        "it is not included in the free plan"
    };
    let action = if entitlements.trial_available {
        format!("Start your free {TRIAL_DAYS}-day trial or upgrade to {PLAN_NAME} to unlock it.")
    } else {
        format!("Upgrade to {PLAN_NAME} to unlock it.")
    };
    format!(
        "PAYWALL:{}:{} requires {PLAN_NAME} ({PRICE_LABEL}) and {reason}. {action}",
        feature.id(),
        feature.label(),
    )
}

fn ensure_feature_with_key(
    db: &Database,
    feature: Feature,
    public_key: Option<&VerifyingKey>,
    now: DateTime<Utc>,
) -> Result<(), String> {
    let entitlements = load_entitlements_with_key(db, public_key, now)?;
    if entitlements.features.has(feature) {
        Ok(())
    } else {
        Err(paywall_error(feature, &entitlements))
    }
}

/// Ok when `feature` is unlocked (active trial or valid license); otherwise an
/// Err starting with `PAYWALL:<feature>:` followed by a sentence for the user.
pub fn ensure_feature(db: &Database, feature: Feature) -> Result<(), String> {
    ensure_feature_with_key(db, feature, configured_public_key().as_ref(), Utc::now())
}

/// True for metadata providers that only serve adult content, under every
/// spelling the metadata back end accepts (`metadata_ext`/`metadata_guard`
/// normalize "PGMA Modernized", "porn-site-nuxt", "IreneHub", "theporndb", ...
/// to their canonical keys, so the paywall must recognize the same aliases).
pub fn is_adult_provider(provider: &str) -> bool {
    if crate::adult_site_provider::is_porn_site_nuxt_alias(provider) {
        return true;
    }
    let provider: String = provider
        .trim()
        .to_ascii_lowercase()
        .chars()
        .map(|c| if c == '-' || c == ' ' { '_' } else { c })
        .collect();
    crate::adult_site_provider::is_porn_site_nuxt_alias(&provider)
        || matches!(
            provider.as_str(),
            "tpdb"
                | "theporndb"
                | "the_porn_db"
                | "porndb"
                | "porn_db"
                | "stashdb"
                | "stash_db"
                | "iafd"
                | "porn_site_nuxt"
                | "phoenixadult"
                | "phoenix_adult"
        )
        || provider.contains("pgma")
        || provider.contains("porn")
}

/// Marker reported in results when adult providers were left out of a run.
pub fn adult_providers_skipped_marker() -> String {
    format!("PAYWALL:{}", Feature::AdultMetadata.id())
}

fn strip_adult_provider_keys_with<V>(
    db: &Database,
    keys: &mut std::collections::HashMap<String, V>,
    public_key: Option<&VerifyingKey>,
    now: DateTime<Utc>,
) -> Option<String> {
    if ensure_feature_with_key(db, Feature::AdultMetadata, public_key, now).is_ok() {
        return None;
    }
    keys.retain(|provider, _| !is_adult_provider(provider));
    Some(adult_providers_skipped_marker())
}

/// When adult metadata is not unlocked, drop every adult provider from a
/// provider-key map so a run can carry on with the other providers. Returns
/// `Some("PAYWALL:adult_metadata")` when adult providers were skipped.
pub fn strip_adult_provider_keys<V>(
    db: &Database,
    keys: &mut std::collections::HashMap<String, V>,
) -> Option<String> {
    strip_adult_provider_keys_with(db, keys, configured_public_key().as_ref(), Utc::now())
}

/// Same as [`ensure_feature`] for callers that hold `AppState`.
pub fn ensure_feature_state(state: &AppState, feature: Feature) -> Result<(), String> {
    let db = state.db.lock().map_err(|error| error.to_string())?;
    ensure_feature(&db, feature)
}

/// Source types that are local storage and stay free. "adult" is a local
/// folder labelled adult: the library is free, adult *metadata* is not.
const LOCAL_SOURCE_TYPES: &[&str] = &["folder", "drive", "file", "local", "adult", ""];

/// True when a library source is not a local drive: a NAS/cloud/network
/// source type, or a network path (UNC share or URL) under any type.
pub fn is_external_source(source_type: &str, path: &str) -> bool {
    let source_type = source_type.trim().to_ascii_lowercase();
    if !LOCAL_SOURCE_TYPES.contains(&source_type.as_str()) {
        return true;
    }
    let path = path.trim();
    // `\\?\C:\...` and `\\.\` are local device paths; `\\?\UNC\` is a share.
    let device = path.starts_with("\\\\?\\") || path.starts_with("\\\\.\\");
    let device_unc = path.to_ascii_uppercase().starts_with("\\\\?\\UNC\\");
    let unc = device_unc || (!device && (path.starts_with("\\\\") || path.starts_with("//")));
    let url = path
        .split_once("://")
        .map(|(scheme, _)| {
            // Two+ letters so "C://x" is a drive; file:// is local.
            scheme.len() >= 2
                && !scheme.eq_ignore_ascii_case("file")
                && scheme
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || c == '+')
        })
        .unwrap_or(false);
    unc || url
}

#[tauri::command]
pub fn get_entitlements(state: State<'_, AppState>) -> Result<Entitlements, String> {
    let db = state.db.lock().map_err(|error| error.to_string())?;
    load_entitlements(&db)
}

#[tauri::command]
pub fn activate_license(state: State<'_, AppState>, token: String) -> Result<Entitlements, String> {
    let token = token.trim().to_string();
    verify_license_token(&token, Utc::now())?;
    let db = state.db.lock().map_err(|error| error.to_string())?;
    db.set_setting_data(SETTING_LICENSE_TOKEN, &token)
        .map_err(|error| format!("Unable to save the license: {error}"))?;
    load_entitlements(&db)
}

#[tauri::command]
pub fn start_trial(state: State<'_, AppState>) -> Result<Entitlements, String> {
    let db = state.db.lock().map_err(|error| error.to_string())?;
    start_trial_at(&db, configured_public_key().as_ref(), Utc::now())
}

#[tauri::command]
pub fn deactivate_license(state: State<'_, AppState>) -> Result<Entitlements, String> {
    let db = state.db.lock().map_err(|error| error.to_string())?;
    db.set_setting_data(SETTING_LICENSE_TOKEN, "")
        .map_err(|error| format!("Unable to remove the license: {error}"))?;
    load_entitlements(&db)
}

#[cfg(test)]
mod tests {
    use super::*;
    use ed25519_dalek::{Signer, SigningKey};

    fn signing_key(seed: u8) -> SigningKey {
        SigningKey::from_bytes(&[seed; 32])
    }

    fn b64(bytes: &[u8]) -> String {
        base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes)
    }

    fn token_for(key: &SigningKey, payload: &serde_json::Value) -> String {
        let signed = format!("{TOKEN_PREFIX}.{}", b64(payload.to_string().as_bytes()));
        let signature = key.sign(signed.as_bytes());
        format!("{signed}.{}", b64(&signature.to_bytes()))
    }

    fn payload(plan: &str, expires_at: &str) -> serde_json::Value {
        serde_json::json!({
            "license_id": "lic_123",
            "email": "buyer@example.com",
            "plan": plan,
            "issued_at": "2026-01-01T00:00:00Z",
            "expires_at": expires_at,
        })
    }

    fn time(value: &str) -> DateTime<Utc> {
        DateTime::parse_from_rfc3339(value)
            .unwrap()
            .with_timezone(&Utc)
    }

    fn temp_db() -> (Database, std::path::PathBuf) {
        let path = std::env::temp_dir().join(format!(
            "cinavault-entitlements-{}.db",
            uuid::Uuid::new_v4()
        ));
        (Database::new(path.to_str().unwrap()).unwrap(), path)
    }

    #[test]
    fn valid_token_verifies() {
        let key = signing_key(7);
        let token = token_for(&key, &payload(PLAN_ID, "2027-01-01T00:00:00Z"));
        let verified = verify_license_token_with_key(
            &token,
            &key.verifying_key(),
            time("2026-06-01T00:00:00Z"),
        )
        .unwrap();
        assert_eq!(verified.payload.email, "buyer@example.com");
        assert_eq!(verified.expires_at, time("2027-01-01T00:00:00Z"));

        // The configured-key path accepts the same key given as base64.
        let encoded =
            base64::engine::general_purpose::STANDARD.encode(key.verifying_key().to_bytes());
        let decoded = decode_public_key(&encoded).unwrap();
        assert!(
            verify_license_token_with_key(&token, &decoded, time("2026-06-01T00:00:00Z")).is_ok()
        );
    }

    /// TEST-ONLY keypair: the public key and token below were produced by
    /// `scripts/issue-license.mjs` with a throwaway private key that was never
    /// stored anywhere. They exist only to prove the Node issuer and this
    /// verifier agree on the token format. Never use this key in a build.
    const SCRIPT_FIXTURE_PUBLIC_KEY: &str = "6ZUOGcTGAPE+qxVjmFLWRO+JxCOT7U7pKAxAb1t7+dc=";
    const SCRIPT_FIXTURE_TOKEN: &str = "CVL1.eyJsaWNlbnNlX2lkIjoibGljX2ZpeHR1cmVfMDAxIiwiZW1haWwiOiJmaXh0dXJlQGV4YW1wbGUuY29tIiwicGxhbiI6ImNpbmF2YXVsdF9wbHVzIiwiaXNzdWVkX2F0IjoiMjAyNi0wMS0wMVQwMDowMDowMFoiLCJleHBpcmVzX2F0IjoiMjAyNy0wMS0wMVQwMDowMDowMFoifQ.HndviGGXwLL6jM4vihRSaUBimwNrd7t8lP5Ov8ZCPjph_4voL0NrczH5SPRCNVMStAv806F3-egp_6mH6hlcDg";

    #[test]
    fn token_from_issue_license_script_verifies() {
        let key = decode_public_key(SCRIPT_FIXTURE_PUBLIC_KEY).unwrap();
        let verified =
            verify_license_token_with_key(SCRIPT_FIXTURE_TOKEN, &key, time("2026-06-01T00:00:00Z"))
                .unwrap();
        assert_eq!(
            verified.payload,
            LicensePayload {
                license_id: "lic_fixture_001".into(),
                email: "fixture@example.com".into(),
                plan: PLAN_ID.into(),
                issued_at: "2026-01-01T00:00:00Z".into(),
                expires_at: "2027-01-01T00:00:00Z".into(),
            }
        );
        // Expired after its 12 months; rejected under any other key.
        assert!(verify_license_token_with_key(
            SCRIPT_FIXTURE_TOKEN,
            &key,
            time("2027-01-01T00:00:00Z")
        )
        .is_err());
        assert!(verify_license_token_with_key(
            SCRIPT_FIXTURE_TOKEN,
            &signing_key(1).verifying_key(),
            time("2026-06-01T00:00:00Z")
        )
        .is_err());
    }

    #[test]
    fn tampered_payload_fails() {
        let key = signing_key(7);
        let token = token_for(&key, &payload(PLAN_ID, "2027-01-01T00:00:00Z"));
        let parts: Vec<&str> = token.split('.').collect();
        let forged_payload = b64(payload(PLAN_ID, "2099-01-01T00:00:00Z")
            .to_string()
            .as_bytes());
        let forged = format!("{}.{}.{}", parts[0], forged_payload, parts[2]);
        let now = time("2026-06-01T00:00:00Z");
        assert!(
            verify_license_token_with_key(&forged, &key.verifying_key(), now)
                .unwrap_err()
                .contains("signature")
        );
        // Signed by a different key.
        assert!(
            verify_license_token_with_key(&token, &signing_key(8).verifying_key(), now).is_err()
        );
        // Garbage.
        assert!(verify_license_token_with_key("CVL1.abc", &key.verifying_key(), now).is_err());
        assert!(verify_license_token_with_key("", &key.verifying_key(), now).is_err());
    }

    #[test]
    fn expired_token_fails() {
        let key = signing_key(7);
        let token = token_for(&key, &payload(PLAN_ID, "2026-05-01T00:00:00Z"));
        let error = verify_license_token_with_key(
            &token,
            &key.verifying_key(),
            time("2026-06-01T00:00:00Z"),
        )
        .unwrap_err();
        assert!(error.contains("expired"), "{error}");
    }

    #[test]
    fn token_expiring_before_its_issue_time_fails() {
        let key = signing_key(7);
        let mut inverted = payload(PLAN_ID, "2026-06-01T00:00:00Z");
        inverted["issued_at"] = serde_json::json!("2026-06-01T12:00:00Z");
        let token = token_for(&key, &inverted);
        // `now` lies before both timestamps and within the issue-time skew
        // allowance, which used to let the inverted token through.
        let error = verify_license_token_with_key(
            &token,
            &key.verifying_key(),
            time("2026-05-31T23:00:00Z"),
        )
        .unwrap_err();
        assert!(error.contains("not after its issue time"), "{error}");

        let mut zero_length = payload(PLAN_ID, "2026-01-01T00:00:00Z");
        zero_length["issued_at"] = serde_json::json!("2026-01-01T00:00:00Z");
        let token = token_for(&key, &zero_length);
        assert!(verify_license_token_with_key(
            &token,
            &key.verifying_key(),
            time("2025-12-31T23:00:00Z"),
        )
        .is_err());
    }

    #[test]
    fn every_adult_provider_alias_is_paywalled() {
        for alias in [
            "tpdb",
            "TPDB",
            "theporndb",
            "ThePornDB",
            "the-porn-db",
            "porndb",
            "stashdb",
            "StashDB",
            "stash-db",
            "iafd",
            "IAFD",
            "phoenixadult",
            "PhoenixAdult",
            "phoenix-adult",
            "phoenix adult",
            "pgma",
            "PGMA Modernized",
            "pgma-modernized",
            "pgma_modernized",
            "plex pgma",
            "porn_site_nuxt",
            "porn-site-nuxt",
            "porn site nuxt",
            "Porn-Site-Nuxt",
            "pornsite_nuxt",
            "pornsite",
            "pornhub-irene",
            "pornhub_irene",
            "IreneHub",
            "irene_hub",
            "irene-hub",
            "nuxt_porn_site",
            "nuxt-porn-site",
            "  tpdb  ",
        ] {
            assert!(is_adult_provider(alias), "{alias} must be paywalled");
        }
        for general in [
            "tmdb",
            "omdb",
            "tvdb",
            "themoviedb",
            "fanart",
            "musicbrainz",
        ] {
            assert!(!is_adult_provider(general), "{general} is not adult");
        }
    }

    #[test]
    fn wrong_plan_fails() {
        let key = signing_key(7);
        let token = token_for(&key, &payload("cinavault_basic", "2027-01-01T00:00:00Z"));
        let error = verify_license_token_with_key(
            &token,
            &key.verifying_key(),
            time("2026-06-01T00:00:00Z"),
        )
        .unwrap_err();
        assert!(error.contains(PLAN_NAME), "{error}");
    }

    #[test]
    fn trial_math() {
        const T: i64 = 14;
        let start = time("2026-01-01T00:00:00Z");
        match trial_state_for_days(Some(start), start, T) {
            TrialState::Active { days_left, ends_at } => {
                assert_eq!(days_left, T);
                assert_eq!(ends_at, start + Duration::days(T));
            }
            other => panic!("expected active trial, got {other:?}"),
        }
        if T > 1 {
            // Partial days round up.
            let later = start + Duration::days(1) + Duration::hours(1);
            assert_eq!(
                trial_state_for_days(Some(start), later, T),
                TrialState::Active {
                    days_left: T - 1,
                    ends_at: start + Duration::days(T)
                }
            );
        }
        let end = start + Duration::days(T);
        assert!(matches!(
            trial_state_for_days(Some(start), end, T),
            TrialState::Expired { .. }
        ));
        assert!(matches!(
            trial_state_for_days(None, start, T),
            TrialState::Expired { ends_at: None }
        ));
    }

    #[test]
    fn clock_rollback_expires_trial() {
        const T: i64 = 14;
        let start = time("2026-03-01T00:00:00Z");
        assert!(matches!(
            trial_state_for_days(Some(start), start - Duration::seconds(1), T),
            TrialState::Expired { .. }
        ));
    }

    #[test]
    fn fresh_install_is_free_until_the_user_opts_in_to_the_trial() {
        assert_eq!(TRIAL_DAYS, 30, "John asked for an opt-in 30-day trial");
        let start = time("2026-10-01T00:00:00Z");
        let (db, path) = temp_db();
        let key = signing_key(11);
        let verifying = key.verifying_key();

        // Fresh install: free plan, Plus locked, trial offered but not running.
        let fresh = load_entitlements_with_key(&db, Some(&verifying), start).unwrap();
        assert_eq!(fresh.plan, "free");
        assert!(fresh.trial_available);
        assert_eq!(fresh.trial_days_left, None);
        assert!(!fresh.features.downloads);
        assert!(!fresh.features.adult_metadata);
        assert!(!fresh.features.external_libraries);
        assert!(!fresh.features.remote_access);
        let error =
            ensure_feature_with_key(&db, Feature::Downloads, Some(&verifying), start).unwrap_err();
        assert!(error.starts_with("PAYWALL:downloads:"), "{error}");
        assert!(error.contains("Start your free 30-day trial"), "{error}");
        // Reading entitlements never starts the trial.
        assert_eq!(read_setting(&db, SETTING_TRIAL_STARTED_AT).unwrap(), None);

        // Opting in unlocks everything for 30 days.
        let trial = start_trial_at(&db, Some(&verifying), start).unwrap();
        assert_eq!(trial.plan, "trial");
        assert_eq!(trial.trial_days_left, Some(30));
        assert!(!trial.trial_available);
        assert!(trial.features.downloads && trial.features.remote_access);

        // Only once per install, even after it ends.
        let again = start_trial_at(&db, Some(&verifying), start + Duration::days(2)).unwrap_err();
        assert!(again.contains("already been used"), "{again}");
        let ended =
            load_entitlements_with_key(&db, Some(&verifying), start + Duration::days(31)).unwrap();
        assert_eq!(ended.plan, "free");
        assert_eq!(ended.trial_days_left, Some(0));
        assert!(!ended.trial_available);
        let error = ensure_feature_with_key(
            &db,
            Feature::Downloads,
            Some(&verifying),
            start + Duration::days(31),
        )
        .unwrap_err();
        assert!(error.contains("trial has ended"), "{error}");
        assert!(!error.contains("Start your free"), "{error}");
        assert!(start_trial_at(&db, Some(&verifying), start + Duration::days(31)).is_err());
        drop(db);
        std::fs::remove_file(path).ok();
    }

    #[test]
    fn trial_is_refused_while_a_license_is_active() {
        let (db, path) = temp_db();
        let key = signing_key(12);
        let verifying = key.verifying_key();
        let now = time("2026-10-01T00:00:00Z");
        let token = token_for(&key, &payload(PLAN_ID, "2030-01-01T00:00:00Z"));
        db.set_setting_data(SETTING_LICENSE_TOKEN, &token).unwrap();
        let plus = load_entitlements_with_key(&db, Some(&verifying), now).unwrap();
        assert_eq!(plus.plan, "plus");
        assert!(!plus.trial_available);
        let error = start_trial_at(&db, Some(&verifying), now).unwrap_err();
        assert!(error.contains("already active"), "{error}");
        assert_eq!(read_setting(&db, SETTING_TRIAL_STARTED_AT).unwrap(), None);
        drop(db);
        std::fs::remove_file(path).ok();
    }

    #[test]
    fn fail_closed_without_public_key() {
        let key = signing_key(7);
        let token = token_for(&key, &payload(PLAN_ID, "2027-01-01T00:00:00Z"));
        let error =
            verify_with_optional_key(&token, None, time("2026-06-01T00:00:00Z")).unwrap_err();
        assert!(error.contains("not configured"), "{error}");

        // Even with a stored token, a keyless build reports no license.
        let (db, path) = temp_db();
        db.set_setting_data(SETTING_LICENSE_TOKEN, &token).unwrap();
        let now = time("2026-06-01T00:00:00Z");
        db.set_setting_data(
            SETTING_TRIAL_STARTED_AT,
            &(now - Duration::days(TRIAL_DAYS + 1)).to_rfc3339(),
        )
        .unwrap();
        let entitlements = load_entitlements_with_key(&db, None, now).unwrap();
        assert_eq!(entitlements.plan, "free");
        assert!(!entitlements.licensing_configured);
        assert!(!entitlements.features.downloads);
        drop(db);
        std::fs::remove_file(path).ok();
    }

    #[test]
    fn entitlements_persist_and_gate_features() {
        let (db, path) = temp_db();
        let key = signing_key(9);
        let verifying = key.verifying_key();
        let start = time("2026-01-01T00:00:00Z");

        // Opting in records the trial start.
        start_trial_at(&db, Some(&verifying), start).unwrap();
        let trial =
            load_entitlements_with_key(&db, Some(&verifying), start + Duration::days(3)).unwrap();
        if TRIAL_DAYS > 3 {
            assert_eq!(trial.plan, "trial");
            assert_eq!(trial.trial_days_left, Some(TRIAL_DAYS - 3));
            assert!(ensure_feature_with_key(
                &db,
                Feature::Downloads,
                Some(&verifying),
                start + Duration::days(3)
            )
            .is_ok());
        }

        let after = start + Duration::days(TRIAL_DAYS + 30);
        let error = ensure_feature_with_key(&db, Feature::RemoteAccess, Some(&verifying), after)
            .unwrap_err();
        assert!(error.starts_with("PAYWALL:remote_access:"), "{error}");
        assert!(error.contains("$9.99/month"));

        // A valid license unlocks everything.
        let token = token_for(&key, &payload(PLAN_ID, "2030-01-01T00:00:00Z"));
        db.set_setting_data(SETTING_LICENSE_TOKEN, &token).unwrap();
        let plus = load_entitlements_with_key(&db, Some(&verifying), after).unwrap();
        assert_eq!(plus.plan, "plus");
        assert_eq!(plus.license_email.as_deref(), Some("buyer@example.com"));
        assert!(plus.features.remote_access && plus.features.adult_metadata);
        assert!(
            ensure_feature_with_key(&db, Feature::ExternalLibraries, Some(&verifying), after)
                .is_ok()
        );

        // An unreadable stored trial start is treated as expired, not reset.
        db.set_setting_data(SETTING_LICENSE_TOKEN, "").unwrap();
        db.set_setting_data(SETTING_TRIAL_STARTED_AT, "garbage")
            .unwrap();
        assert_eq!(read_trial_start(&db).unwrap(), Some(None));
        let free = load_entitlements_with_key(&db, Some(&verifying), start).unwrap();
        assert_eq!(free.plan, "free");
        drop(db);
        std::fs::remove_file(path).ok();
    }

    #[test]
    fn local_sources_are_free_and_network_sources_are_external() {
        assert!(!is_external_source("folder", "D:\\Movies"));
        assert!(!is_external_source("drive", "E:\\"));
        assert!(!is_external_source("file", "/home/me/movie.mkv"));
        assert!(!is_external_source("adult", "/mnt/disk/private"));
        assert!(!is_external_source("Folder", "/media/usb"));
        assert!(is_external_source("nas", "/mnt/nas"));
        assert!(is_external_source("mixed", "C:\\Users\\me\\OneDrive"));
        assert!(is_external_source("folder", "\\\\192.168.1.5\\Movies"));
        assert!(is_external_source("folder", "//nas/Movies"));
        assert!(!is_external_source("drive", "\\\\?\\C:\\Movies"));
        assert!(is_external_source("folder", "\\\\?\\UNC\\nas\\Movies"));
        assert!(is_external_source("folder", "smb://nas/Movies"));
        assert!(is_external_source("drive", "https://example.com/dav"));
    }

    #[test]
    fn adult_provider_keys_are_stripped_only_without_entitlement() {
        let (db, path) = temp_db();
        let key = signing_key(3);
        let verifying = key.verifying_key();
        let start = time("2026-01-01T00:00:00Z");
        start_trial_at(&db, Some(&verifying), start).unwrap();
        let fresh = || -> std::collections::HashMap<String, String> {
            [
                "tmdb",
                "omdb",
                "tpdb",
                "theporndb",
                "stashdb",
                "iafd",
                "pgma",
                "porn_site_nuxt",
                "phoenixadult",
            ]
            .iter()
            .map(|p| (p.to_string(), "k".to_string()))
            .collect()
        };

        if TRIAL_DAYS > 0 {
            let mut during_trial = fresh();
            assert_eq!(
                strip_adult_provider_keys_with(&db, &mut during_trial, Some(&verifying), start),
                None
            );
            assert_eq!(during_trial.len(), 9);
        }

        let mut after_trial = fresh();
        let skipped = strip_adult_provider_keys_with(
            &db,
            &mut after_trial,
            Some(&verifying),
            start + Duration::days(TRIAL_DAYS + 1),
        );
        assert_eq!(skipped.as_deref(), Some("PAYWALL:adult_metadata"));
        let mut left: Vec<_> = after_trial.keys().cloned().collect();
        left.sort();
        assert_eq!(left, ["omdb", "tmdb"]);
        drop(db);
        std::fs::remove_file(path).ok();
    }

    #[test]
    fn feature_ids_match_contract() {
        let ids: Vec<&str> = [
            Feature::ExternalLibraries,
            Feature::Downloads,
            Feature::AdultMetadata,
            Feature::RemoteAccess,
        ]
        .iter()
        .map(|feature| feature.id())
        .collect();
        assert_eq!(
            ids,
            [
                "external_libraries",
                "downloads",
                "adult_metadata",
                "remote_access"
            ]
        );
        let json = serde_json::to_value(compute_entitlements(
            TrialState::Expired { ends_at: None },
            None,
            false,
            None,
        ))
        .unwrap();
        for field in [
            "plan",
            "trial_days_left",
            "trial_ends_at",
            "license_email",
            "license_expires_at",
            "features",
            "checkout_url",
            "price_label",
            "licensing_configured",
        ] {
            assert!(json.get(field).is_some(), "missing {field}");
        }
        assert_eq!(json["price_label"], "$9.99/month");
    }
}
