// CinaVault Premium — Google Drive OAuth (Issue 6).
//
// This is the ONLY Google sign-in path. It replaces the previous behaviour in
// which the frontend built an auth URL around a placeholder client ID and the
// backend discarded that URL and recorded a local sync folder instead.
//
// Flow (desktop-app OAuth, PKCE, no client secret):
//   1. Resolve a real client ID (see `resolve_client_id` — never hardcoded,
//      never the old "cinavault-gdrive-client" placeholder).
//   2. Bind the loopback listener FIRST on 127.0.0.1:19284, then open the
//      Google auth URL in the system browser (same `open::that` mechanism as
//      lib.rs `open_external_url`). The redirect URI below is the one already
//      used by the frontend constants (ONEDRIVE_REDIRECT / GDRIVE_REDIRECT in
//      CloudNASTab.tsx); no listener existed for it before this module.
//   3. Wait for the callback (max 5 minutes), verify `state`, exchange the
//      code for tokens with PKCE, and store the tokens ONLY in the OS
//      credential store via `crate::secure_credentials` (keyring, service
//      "CinaVault Premium"). Tokens are never written to the DB, a JSON file,
//      or a frontend constant. The DB record (`cloud_connection_googledrive`)
//      holds only { provider, method: "google_oauth", account, verified_at }.
//   4. Validation (`validate_tokens`, used by `cloud_get_status`) calls the
//      Drive about endpoint; on 401 it refreshes once with the refresh token
//      and retries. If refresh fails, the token is cleared and the caller
//      transitions to a true disconnected state.
//
// No new Cargo dependencies: PKCE uses `sha2` + `base64` (already present),
// randomness comes from `uuid` v4 (OS RNG), HTTP uses reqwest blocking
// (already enabled in Cargo.toml), URL query encoding is hand-rolled below
// so no `url`/`form_urlencoded` crate is needed.
//
// UNVERIFIED WITHOUT A REAL CLIENT ID (see issue6/NOTES.md): the browser
// redirect, the code exchange, refresh, revoke, and the about call all need
// a user-supplied Google Cloud OAuth client ID (desktop-app type, Drive API
// enabled, redirect URI http://localhost:19284/auth/callback registered).
// Cargo is not installed in this environment, so even compilation must be
// confirmed by CI (`npm run rust:check:windows`).

use crate::secure_credentials;
use crate::AppState;
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::io::{Read, Write};
use std::net::TcpListener;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tauri::State;

/// Redirect URI, unchanged from the pre-existing frontend constants: both
/// ONEDRIVE_REDIRECT and GDRIVE_REDIRECT in CloudNASTab.tsx already used
/// exactly this value. The difference is that a listener now exists for it.
pub const REDIRECT_URI: &str = "http://localhost:19284/auth/callback";
const LOOPBACK_ADDR: &str = "127.0.0.1:19284";
const AUTH_URL: &str = "https://accounts.google.com/o/oauth2/v2/auth";
const TOKEN_URL: &str = "https://oauth2.googleapis.com/token";
const REVOKE_URL: &str = "https://oauth2.googleapis.com/revoke";
const ABOUT_URL: &str = "https://www.googleapis.com/drive/v3/about";
const SCOPE: &str = "https://www.googleapis.com/auth/drive.readonly";
/// How long the sign-in browser flow may take before the command gives up.
const CALLBACK_TIMEOUT: Duration = Duration::from_secs(300);
/// Refresh a token this far before it actually expires (clock-skew buffer).
const EXPIRY_SKEW_SECS: i64 = 60;

/// Client ID resolution order (see `resolve_client_id`):
/// 1. `CINAVAULT_GDRIVE_CLIENT_ID` environment variable at runtime.
/// 2. OS credential-store entry `gdrive_client_id` (set from the UI via
///    `cloud_oauth_set_client_id`). This follows the repo's existing pattern
///    for runtime credentials (cf. HF_TOKEN env fallback in lib.rs) and keeps
///    the value out of the repository, the binary, and the DB.
const CLIENT_ID_ENV: &str = "CINAVAULT_GDRIVE_CLIENT_ID";
const CLIENT_ID_ENTRY: &str = "gdrive_client_id";
const TOKEN_ENTRY: &str = "googledrive_oauth_tokens";

/// Values that are placeholders, not real Google client IDs. Any of these is
/// rejected wherever a client ID is read or saved.
const PLACEHOLDER_CLIENT_IDS: &[&str] = &["cinavault-gdrive-client", "cinavault-onedrive-client"];

/// Full user-facing "not configured" text. The `GDRIVE_NOT_CONFIGURED:` prefix
/// is what the frontend matches on to show the client-ID input form.
pub const NOT_CONFIGURED_ERROR: &str = "GDRIVE_NOT_CONFIGURED: No real Google Cloud OAuth client ID is configured. Create a Desktop-app OAuth client ID in Google Cloud (Drive API enabled, redirect URI http://localhost:19284/auth/callback registered), then paste it in the Google Drive card below or set the CINAVAULT_GDRIVE_CLIENT_ID environment variable and try again.";

fn is_placeholder(client_id: &str) -> bool {
    PLACEHOLDER_CLIENT_IDS.contains(&client_id.trim())
}

/// Resolve the Google OAuth client ID from runtime configuration / secure
/// storage only. Never returns a hardcoded or placeholder value.
pub fn resolve_client_id() -> Result<String, String> {
    if let Ok(value) = std::env::var(CLIENT_ID_ENV) {
        let value = value.trim().to_string();
        if !value.is_empty() && !is_placeholder(&value) {
            return Ok(value);
        }
    }
    if let Ok(Some(value)) = secure_credentials::get(CLIENT_ID_ENTRY) {
        let value = value.trim().to_string();
        if !value.is_empty() && !is_placeholder(&value) {
            return Ok(value);
        }
    }
    Err(NOT_CONFIGURED_ERROR.to_string())
}

pub fn client_configured() -> bool {
    resolve_client_id().is_ok()
}

/// Save a user-supplied client ID to the OS credential store (not the DB,
/// not a source file). The value must look like a real Google OAuth client
/// ID; placeholders and malformed values are rejected with an explanation.
#[tauri::command]
pub fn cloud_oauth_set_client_id(client_id: String) -> Result<Value, String> {
    let client_id = client_id.trim().to_string();
    if client_id.is_empty() {
        return Err("Paste a Google Cloud OAuth client ID first.".to_string());
    }
    if is_placeholder(&client_id) {
        return Err(
            "That value is a placeholder, not a real Google client ID. Paste the \
             OAuth client ID from your Google Cloud project (it ends with \
             .apps.googleusercontent.com)."
                .to_string(),
        );
    }
    if client_id.contains(char::is_whitespace)
        || !client_id.ends_with(".apps.googleusercontent.com")
    {
        return Err(
            "That does not look like a Google OAuth client ID. It should be the \
             desktop-app client ID from Google Cloud, ending with \
             .apps.googleusercontent.com, with no spaces."
                .to_string(),
        );
    }
    secure_credentials::set(CLIENT_ID_ENTRY, &client_id)?;
    Ok(json!({ "success": true, "configured": true }))
}

// ── URL query encoding (hand-rolled; no `url` crate in Cargo.toml) ─────────

fn is_unreserved(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b'~')
}

pub(crate) fn percent_encode(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for byte in value.bytes() {
        if is_unreserved(byte) {
            out.push(byte as char);
        } else {
            out.push_str(&format!("%{byte:02X}"));
        }
    }
    out
}

pub(crate) fn percent_decode(value: &str) -> String {
    let bytes = value.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'%' if i + 2 < bytes.len() => {
                let hex = std::str::from_utf8(&bytes[i + 1..i + 3]).unwrap_or("");
                if let Ok(byte) = u8::from_str_radix(hex, 16) {
                    out.push(byte);
                    i += 3;
                    continue;
                }
                out.push(b'%');
                i += 1;
            }
            b'+' => {
                out.push(b' ');
                i += 1;
            }
            byte => {
                out.push(byte);
                i += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn parse_query(query: &str) -> Vec<(String, String)> {
    query
        .split('&')
        .filter(|part| !part.is_empty())
        .filter_map(|part| {
            let (key, value) = part.split_once('=')?;
            Some((percent_decode(key), percent_decode(value)))
        })
        .collect()
}

fn query_value<'a>(pairs: &'a [(String, String)], key: &str) -> Option<&'a str> {
    pairs
        .iter()
        .find(|(name, _)| name == key)
        .map(|(_, value)| value.as_str())
}

// ── PKCE ────────────────────────────────────────────────────────────────────

/// (verifier, challenge). The verifier is 32 bytes from two UUID v4 values
/// (OS RNG), base64url-encoded to 43 characters, within RFC 7636's 43–128
/// range; the challenge is base64url(SHA-256(verifier)).
pub(crate) fn pkce_pair() -> (String, String) {
    let mut random = [0u8; 32];
    random[..16].copy_from_slice(uuid::Uuid::new_v4().as_bytes());
    random[16..].copy_from_slice(uuid::Uuid::new_v4().as_bytes());
    let verifier = URL_SAFE_NO_PAD.encode(random);
    let mut hasher = Sha256::new();
    hasher.update(verifier.as_bytes());
    let challenge = URL_SAFE_NO_PAD.encode(hasher.finalize());
    (verifier, challenge)
}

fn auth_url(client_id: &str, challenge: &str, state: &str) -> String {
    let params = [
        ("client_id", client_id),
        ("response_type", "code"),
        ("redirect_uri", REDIRECT_URI),
        ("scope", SCOPE),
        ("access_type", "offline"),
        ("prompt", "consent"),
        ("code_challenge", challenge),
        ("code_challenge_method", "S256"),
        ("state", state),
    ];
    let query = params
        .iter()
        .map(|(key, value)| format!("{}={}", percent_encode(key), percent_encode(value)))
        .collect::<Vec<_>>()
        .join("&");
    format!("{AUTH_URL}?{query}")
}

// ── Tokens (OS credential store only) ───────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenSet {
    pub access_token: String,
    #[serde(default)]
    pub refresh_token: Option<String>,
    /// Unix seconds at which the access token expires (already skew-adjusted).
    #[serde(default)]
    pub expires_at_unix: i64,
    #[serde(default)]
    pub scope: Option<String>,
}

impl TokenSet {
    fn is_expired(&self, now_unix: i64) -> bool {
        self.expires_at_unix <= now_unix + EXPIRY_SKEW_SECS
    }
}

fn now_unix() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs() as i64)
        .unwrap_or(0)
}

fn load_tokens() -> Result<Option<TokenSet>, String> {
    match secure_credentials::get(TOKEN_ENTRY)? {
        Some(raw) => {
            let tokens: TokenSet =
                serde_json::from_str(&raw).map_err(|error| error.to_string())?;
            Ok(Some(tokens))
        }
        None => Ok(None),
    }
}

fn store_tokens(tokens: &TokenSet) -> Result<(), String> {
    let raw = serde_json::to_string(tokens).map_err(|error| error.to_string())?;
    secure_credentials::set(TOKEN_ENTRY, &raw)
}

pub(crate) fn clear_tokens() {
    if let Err(error) = secure_credentials::delete(TOKEN_ENTRY) {
        log::warn!("Could not delete stored Google Drive tokens: {error}");
    }
}

/// Best-effort revocation, then credential-store deletion. Revocation failure
/// never blocks clearing: a token the user is disconnecting must not survive
/// because Google's revoke endpoint was unreachable. Google's revoke of a
/// refresh token also invalidates its access tokens, so the refresh token is
/// revoked when present, otherwise the access token.
pub fn revoke_and_clear() {
    if let Ok(Some(tokens)) = load_tokens() {
        let token = tokens
            .refresh_token
            .as_deref()
            .unwrap_or(tokens.access_token.as_str());
        let client = reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(15))
            .build();
        if let Ok(client) = client {
            let result = client
                .post(REVOKE_URL)
                .form(&[("token", token)])
                .send();
            if let Err(error) = result {
                log::warn!("Google token revocation failed (clearing anyway): {error}");
            }
        }
    }
    clear_tokens();
}

// ── Token exchange / refresh / Drive about ──────────────────────────────────

#[derive(Debug, Deserialize)]
struct TokenResponse {
    access_token: String,
    #[serde(default)]
    refresh_token: Option<String>,
    #[serde(default)]
    expires_in: Option<i64>,
    #[serde(default)]
    scope: Option<String>,
}

fn http_client() -> Result<reqwest::blocking::Client, String> {
    reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(30))
        .build()
        .map_err(|error| error.to_string())
}

fn exchange_code(client_id: &str, code: &str, verifier: &str) -> Result<TokenSet, String> {
    let response = http_client()?
        .post(TOKEN_URL)
        .form(&[
            ("code", code),
            ("client_id", client_id),
            ("redirect_uri", REDIRECT_URI),
            ("grant_type", "authorization_code"),
            ("code_verifier", verifier),
        ])
        .send()
        .map_err(|error| format!("Google token exchange failed: {error}"))?;
    if !response.status().is_success() {
        return Err(format!(
            "Google token exchange failed (HTTP {}). Check that the client ID is a \
             desktop-app OAuth client with the redirect URI {REDIRECT_URI} registered.",
            response.status()
        ));
    }
    let body: TokenResponse = response
        .json()
        .map_err(|error| format!("Google token exchange returned an unreadable response: {error}"))?;
    let expires_in = body.expires_in.unwrap_or(3600).max(60);
    Ok(TokenSet {
        access_token: body.access_token,
        refresh_token: body.refresh_token,
        expires_at_unix: now_unix() + expires_in - EXPIRY_SKEW_SECS,
        scope: body.scope,
    })
}

fn refresh_tokens(client_id: &str, refresh_token: &str) -> Result<TokenSet, String> {
    let response = http_client()?
        .post(TOKEN_URL)
        .form(&[
            ("refresh_token", refresh_token),
            ("client_id", client_id),
            ("grant_type", "refresh_token"),
        ])
        .send()
        .map_err(|error| format!("Google token refresh failed: {error}"))?;
    if !response.status().is_success() {
        return Err(format!(
            "Google token refresh failed (HTTP {}). The sign-in has expired; use Reconnect.",
            response.status()
        ));
    }
    let body: TokenResponse = response
        .json()
        .map_err(|error| format!("Google token refresh returned an unreadable response: {error}"))?;
    let expires_in = body.expires_in.unwrap_or(3600).max(60);
    Ok(TokenSet {
        access_token: body.access_token,
        // Google does not always return a new refresh token; keep the old one.
        refresh_token: body.refresh_token.or(Some(refresh_token.to_string())),
        expires_at_unix: now_unix() + expires_in - EXPIRY_SKEW_SECS,
        scope: body.scope,
    })
}

enum AboutOutcome {
    Ok(String),
    Unauthorized,
    Transient(String),
}

fn drive_about(access_token: &str) -> AboutOutcome {
    let client = match http_client() {
        Ok(client) => client,
        Err(error) => return AboutOutcome::Transient(error),
    };
    let response = match client
        .get(ABOUT_URL)
        .query(&[("fields", "user")])
        .bearer_auth(access_token)
        .send()
    {
        Ok(response) => response,
        Err(error) => return AboutOutcome::Transient(format!("Drive check failed: {error}")),
    };
    if response.status().as_u16() == 401 {
        return AboutOutcome::Unauthorized;
    }
    if !response.status().is_success() {
        return AboutOutcome::Transient(format!(
            "Drive check failed (HTTP {})",
            response.status()
        ));
    }
    let body: Value = match response.json() {
        Ok(body) => body,
        Err(error) => {
            return AboutOutcome::Transient(format!(
                "Drive check returned an unreadable response: {error}"
            ))
        }
    };
    AboutOutcome::Ok(
        body["user"]["emailAddress"]
            .as_str()
            .unwrap_or("Google Drive")
            .to_string(),
    )
}

/// Outcome of validating the stored Google Drive token. `Invalid` means the
/// token is gone or unrefreshable — the caller must transition to a true
/// disconnected state. `Transient` means "do not clear anything": the network
/// failed, or no client ID is configured to refresh with.
pub enum DriveCheck {
    Valid { email: String },
    NoToken,
    Invalid,
    Transient(String),
}

/// Validate the stored token against Drive, refreshing once on expiry or
/// 401 and retrying. This is the refresh-on-401 path; future Drive API
/// commands must call this (not `load_tokens` directly) for an access token.
pub fn validate_tokens() -> DriveCheck {
    let mut tokens = match load_tokens() {
        Ok(Some(tokens)) => tokens,
        Ok(None) => return DriveCheck::NoToken,
        Err(error) => return DriveCheck::Transient(error),
    };
    // Refresh first when the token is expired (or about to be).
    if tokens.is_expired(now_unix()) {
        match refresh_once(&mut tokens) {
            Ok(()) => {}
            Err(DriveCheck::Invalid) => return DriveCheck::Invalid,
            Err(DriveCheck::Transient(message)) => return DriveCheck::Transient(message),
            Err(_) => return DriveCheck::Invalid,
        }
    }
    match drive_about(&tokens.access_token) {
        AboutOutcome::Ok(email) => DriveCheck::Valid { email },
        AboutOutcome::Unauthorized => {
            // Refresh-on-401: exactly one refresh, then one retry.
            match refresh_once(&mut tokens) {
                Ok(()) => match drive_about(&tokens.access_token) {
                    AboutOutcome::Ok(email) => DriveCheck::Valid { email },
                    AboutOutcome::Unauthorized => {
                        clear_tokens();
                        DriveCheck::Invalid
                    }
                    AboutOutcome::Transient(message) => DriveCheck::Transient(message),
                },
                Err(DriveCheck::Invalid) => DriveCheck::Invalid,
                Err(DriveCheck::Transient(message)) => DriveCheck::Transient(message),
                Err(_) => DriveCheck::Invalid,
            }
        }
        AboutOutcome::Transient(message) => DriveCheck::Transient(message),
    }
}

/// One refresh attempt. On success the new tokens are stored. On failure the
/// stored tokens are cleared and `Invalid` is returned, except when no
/// client ID is configured to refresh with — then nothing is cleared and
/// `Transient` carries the not-configured message.
fn refresh_once(tokens: &mut TokenSet) -> Result<(), DriveCheck> {
    let Some(refresh_token) = tokens.refresh_token.clone() else {
        clear_tokens();
        return Err(DriveCheck::Invalid);
    };
    let client_id = match resolve_client_id() {
        Ok(client_id) => client_id,
        Err(message) => return Err(DriveCheck::Transient(message)),
    };
    match refresh_tokens(&client_id, &refresh_token) {
        Ok(new_tokens) => {
            *tokens = new_tokens;
            match store_tokens(tokens) {
                Ok(()) => Ok(()),
                Err(error) => Err(DriveCheck::Transient(error)),
            }
        }
        Err(_) => {
            clear_tokens();
            Err(DriveCheck::Invalid)
        }
    }
}

// ── Loopback callback listener ──────────────────────────────────────────────

fn respond(stream: &mut std::net::TcpStream, status: &str, body: &str) {
    let response = format!(
        "HTTP/1.1 {status}\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        body.len(),
        body
    );
    let _ = stream.write_all(response.as_bytes());
    let _ = stream.flush();
}

/// Parse a callback request line. Returns the authorization code, or an error
/// for a denied/error callback. State verification happens in the caller.
fn parse_callback_request(request_line: &str) -> Result<(String, String), String> {
    let path = request_line
        .split_whitespace()
        .nth(1)
        .ok_or_else(|| "Malformed browser callback request".to_string())?;
    let (_, query) = path
        .split_once('?')
        .ok_or_else(|| "Browser callback had no query string".to_string())?;
    let pairs = parse_query(query);
    if let Some(error) = query_value(&pairs, "error") {
        return Err(format!(
            "Google sign-in was not completed ({error}). Try again, or use the local-folder fallback."
        ));
    }
    let code = query_value(&pairs, "code")
        .ok_or_else(|| "Browser callback contained no authorization code".to_string())?;
    let state = query_value(&pairs, "state").unwrap_or("");
    Ok((code.to_string(), state.to_string()))
}

/// Wait for the single callback to the loopback listener. Requests to other
/// paths (favicons, probes) are answered and ignored; a callback with a
/// mismatched `state` is rejected and waiting continues, so a forged or
/// stray redirect cannot complete the flow.
fn wait_for_callback(
    listener: TcpListener,
    expected_state: &str,
    timeout: Duration,
) -> Result<String, String> {
    listener
        .set_nonblocking(true)
        .map_err(|error| error.to_string())?;
    let deadline = Instant::now() + timeout;
    let mut buffer = [0u8; 8192];
    loop {
        if Instant::now() > deadline {
            return Err(
                "Google sign-in timed out after 5 minutes. Close the browser tab and try again."
                    .to_string(),
            );
        }
        match listener.accept() {
            Ok((mut stream, _)) => {
                let _ = stream.set_read_timeout(Some(Duration::from_secs(10)));
                let mut raw = Vec::new();
                loop {
                    match stream.read(&mut buffer) {
                        Ok(0) => break,
                        Ok(read) => {
                            raw.extend_from_slice(&buffer[..read]);
                            if raw.windows(4).any(|window| window == b"\r\n\r\n") {
                                break;
                            }
                            if raw.len() > 16384 {
                                break;
                            }
                        }
                        Err(_) => break,
                    }
                }
                let text = String::from_utf8_lossy(&raw);
                let request_line = text.lines().next().unwrap_or("");
                let path = request_line.split_whitespace().nth(1).unwrap_or("");
                if !path.starts_with("/auth/callback") {
                    respond(&mut stream, "404 Not Found", "<p>Not found.</p>");
                    continue;
                }
                match parse_callback_request(request_line) {
                    Ok((code, state)) => {
                        if state != expected_state {
                            respond(
                                &mut stream,
                                "400 Bad Request",
                                "<p>Sign-in state did not match. Close this tab and try again.</p>",
                            );
                            continue;
                        }
                        respond(
                            &mut stream,
                            "200 OK",
                            "<p>Google Drive connected to CinaVault. You can close this tab.</p>",
                        );
                        return Ok(code);
                    }
                    Err(message) => {
                        respond(&mut stream, "400 Bad Request", "<p>Sign-in failed.</p>");
                        return Err(message);
                    }
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                std::thread::sleep(Duration::from_millis(50));
            }
            Err(error) => return Err(format!("Loopback listener failed: {error}")),
        }
    }
}

// ── Full sign-in command ────────────────────────────────────────────────────

/// Complete Google Drive sign-in: open Google in the system browser, wait for
/// the loopback callback, exchange the code (PKCE), validate via Drive about,
/// and store tokens in the OS credential store. The DB record written here
/// contains no token. On any failure before the exchange, nothing is stored
/// and no "connected" record is written.
#[tauri::command]
pub fn cloud_oauth_connect(state: State<AppState>, provider: String) -> Result<Value, String> {
    let normalized = provider.trim().to_ascii_lowercase();
    if !matches!(
        normalized.as_str(),
        "googledrive" | "google_drive" | "google drive" | "gdrive"
    ) {
        return Err(format!("Unsupported cloud provider for Google OAuth: {provider}"));
    }
    let client_id = resolve_client_id()?;
    // Bind FIRST so the redirect cannot arrive before the listener exists.
    let listener = TcpListener::bind(LOOPBACK_ADDR).map_err(|error| {
        format!(
            "Could not listen on {LOOPBACK_ADDR} for the Google sign-in callback ({error}). \
             Close anything else using port 19284 and try again."
        )
    })?;
    let (verifier, challenge) = pkce_pair();
    let state_token = uuid::Uuid::new_v4().to_string();
    let url = auth_url(&client_id, &challenge, &state_token);
    open::that(&url).map_err(|error| format!("Could not open the browser: {error}"))?;
    let code = wait_for_callback(listener, &state_token, CALLBACK_TIMEOUT)?;
    let tokens = exchange_code(&client_id, &code, &verifier)?;
    // Validate immediately so the account label is a real Google identity,
    // not an assumption. A transient about failure does not fail sign-in:
    // the tokens were just issued and the next status check revalidates.
    let account = match drive_about(&tokens.access_token) {
        AboutOutcome::Ok(email) => email,
        _ => "Google Drive".to_string(),
    };
    // Keep the refresh token even if a future refresh response omits it:
    // exchange_code already stored what Google returned; nothing to merge.
    store_tokens(&tokens)?;
    let record = json!({
        "provider": "googledrive",
        "method": "google_oauth",
        "account": account,
        "verified_at": chrono::Utc::now().to_rfc3339(),
    });
    {
        let db = state.db.lock().map_err(|error| error.to_string())?;
        db.set_setting_data("cloud_connection_googledrive", &record.to_string())
            .map_err(|error| error.to_string())?;
    }
    Ok(json!({
        "success": true,
        "account": account,
        "method": "google_oauth",
    }))
}

#[cfg(test)]
mod tests {
    use super::{is_placeholder, parse_callback_request, percent_decode, percent_encode, pkce_pair};
    use base64::Engine as _;
    use sha2::Digest;

    #[test]
    fn pkce_challenge_matches_rfc7636_example() {
        // RFC 7636 Appendix B: this verifier must produce this challenge.
        let verifier = "dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk";
        let mut hasher = sha2::Sha256::new();
        hasher.update(verifier.as_bytes());
        let challenge =
            base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(hasher.finalize());
        assert_eq!(challenge, "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM");
        // And our own generator always yields a 43-char verifier.
        let (generated_verifier, generated_challenge) = pkce_pair();
        assert_eq!(generated_verifier.len(), 43);
        assert!(!generated_challenge.is_empty());
        assert_ne!(generated_verifier, generated_challenge);
    }

    #[test]
    fn query_encoding_round_trips() {
        let original = "https://example.com/a b?x=1&y=2";
        assert_eq!(percent_decode(&percent_encode(original)), original);
        assert_eq!(percent_encode("a b"), "a%20b");
    }

    #[test]
    fn callback_parsing_accepts_code_and_rejects_denial() {
        let (code, state) =
            parse_callback_request("GET /auth/callback?code=abc123&state=xyz HTTP/1.1")
                .unwrap();
        assert_eq!(code, "abc123");
        assert_eq!(state, "xyz");
        assert!(parse_callback_request(
            "GET /auth/callback?error=access_denied&state=xyz HTTP/1.1"
        )
        .is_err());
        assert!(parse_callback_request("GET /auth/callback?state=xyz HTTP/1.1").is_err());
    }

    #[test]
    fn placeholders_are_never_valid_client_ids() {
        assert!(is_placeholder("cinavault-gdrive-client"));
        assert!(is_placeholder("cinavault-onedrive-client"));
        assert!(!is_placeholder("1234567890-abcdef.apps.googleusercontent.com"));
    }

    #[test]
    fn expired_tokens_are_detected_with_skew() {
        let tokens = super::TokenSet {
            access_token: "x".to_string(),
            refresh_token: None,
            expires_at_unix: 1000,
            scope: None,
        };
        assert!(tokens.is_expired(1000));
        assert!(!tokens.is_expired(100));
    }
}
