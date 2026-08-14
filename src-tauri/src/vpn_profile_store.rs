use base64::Engine;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::net::IpAddr;
use std::path::{Path, PathBuf};
use std::process::Command;
use tauri::{AppHandle, Manager};

#[cfg(target_os = "windows")]
use std::os::windows::process::CommandExt;
#[cfg(target_os = "windows")]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

#[derive(Debug, Clone, Serialize)]
pub struct StoredVpnProfile {
    pub name: String,
    pub active: bool,
    pub addresses: Vec<String>,
    pub endpoint: String,
    pub allowed_ips: Vec<String>,
    pub verified: bool,
    pub is_default: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct ProfileState {
    default_profile: Option<String>,
    verified_profiles: Vec<String>,
    auto_connect: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ParsedVpnProfile {
    pub addresses: Vec<String>,
    pub endpoint: String,
    pub allowed_ips: Vec<String>,
}

pub fn profile_directory(app: &AppHandle) -> Result<PathBuf, String> {
    let directory = app
        .path()
        .app_data_dir()
        .map_err(|error| format!("unable to resolve app data directory: {error}"))?
        .join("vpn")
        .join("profiles");
    std::fs::create_dir_all(&directory)
        .map_err(|error| format!("unable to create VPN profile directory: {error}"))?;
    restrict_to_current_user(&directory)?;
    Ok(directory)
}

pub fn validate_profile(content: &str) -> Result<ParsedVpnProfile, String> {
    let mut section = "";
    let mut sections = HashMap::<String, HashMap<String, String>>::new();
    for (index, raw) in content.lines().enumerate() {
        let line = raw.split('#').next().unwrap_or("").trim();
        if line.is_empty() {
            continue;
        }
        if line.starts_with('[') && line.ends_with(']') {
            let name = &line[1..line.len() - 1];
            if !matches!(name, "Interface" | "Peer") {
                return Err(format!("section: unsupported section [{name}]"));
            }
            if sections.contains_key(name) {
                return Err(format!("section: duplicate [{name}] section"));
            }
            sections.insert(name.to_string(), HashMap::new());
            section = name;
            continue;
        }
        if section.is_empty() {
            return Err(format!(
                "line {}: field appears before a section",
                index + 1
            ));
        }
        let (key, value) = line
            .split_once('=')
            .ok_or_else(|| format!("line {}: expected key = value", index + 1))?;
        let key = key.trim();
        let value = value.trim();
        if value.is_empty() {
            return Err(format!("{key}: value cannot be empty"));
        }
        let fields = sections.get_mut(section).expect("section exists");
        if fields.insert(key.to_string(), value.to_string()).is_some() {
            return Err(format!("{key}: duplicate field"));
        }
    }
    let interface = sections
        .get("Interface")
        .ok_or("section: missing [Interface]")?;
    let peer = sections.get("Peer").ok_or("section: missing [Peer]")?;
    validate_key("PrivateKey", required(interface, "PrivateKey")?)?;
    validate_key("PublicKey", required(peer, "PublicKey")?)?;
    let addresses = csv(required(interface, "Address")?);
    for value in &addresses {
        validate_cidr("Address", value)?;
    }
    let allowed_ips = csv(required(peer, "AllowedIPs")?);
    for value in &allowed_ips {
        validate_cidr("AllowedIPs", value)?;
    }
    let endpoint = required(peer, "Endpoint")?.to_string();
    validate_endpoint(&endpoint)?;
    Ok(ParsedVpnProfile {
        addresses,
        endpoint,
        allowed_ips,
    })
}

fn required<'a>(fields: &'a HashMap<String, String>, key: &str) -> Result<&'a str, String> {
    fields
        .get(key)
        .map(String::as_str)
        .ok_or_else(|| format!("{key}: required field is missing"))
}
fn csv(value: &str) -> Vec<String> {
    value
        .split(',')
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .map(str::to_string)
        .collect()
}
fn validate_key(field: &str, value: &str) -> Result<(), String> {
    let decoded = base64::engine::general_purpose::STANDARD
        .decode(value)
        .map_err(|_| format!("{field}: must be a base64 WireGuard key"))?;
    if decoded.len() != 32 {
        return Err(format!("{field}: must decode to 32 bytes"));
    }
    Ok(())
}
fn validate_cidr(field: &str, value: &str) -> Result<(), String> {
    let (ip, prefix) = value
        .split_once('/')
        .ok_or_else(|| format!("{field}: '{value}' must use CIDR notation"))?;
    let ip: IpAddr = ip
        .parse()
        .map_err(|_| format!("{field}: '{value}' has an invalid IP address"))?;
    let prefix: u8 = prefix
        .parse()
        .map_err(|_| format!("{field}: '{value}' has an invalid prefix"))?;
    let max = if ip.is_ipv4() { 32 } else { 128 };
    if prefix > max {
        return Err(format!("{field}: '{value}' prefix exceeds {max}"));
    }
    Ok(())
}
fn validate_endpoint(value: &str) -> Result<(), String> {
    let (_, port) = value
        .rsplit_once(':')
        .ok_or("Endpoint: must include host and port")?;
    let port: u16 = port
        .parse()
        .map_err(|_| "Endpoint: port must be between 1 and 65535")?;
    if port == 0 || value.starts_with(':') {
        return Err("Endpoint: host and non-zero port are required".into());
    }
    Ok(())
}

pub fn import_profile(app: &AppHandle, source_path: &str) -> Result<StoredVpnProfile, String> {
    let source = Path::new(source_path);
    if !source.is_file() {
        return Err("selected WireGuard profile does not exist or is not a file".to_string());
    }
    if !source
        .extension()
        .and_then(|value| value.to_str())
        .map(|value| value.eq_ignore_ascii_case("conf"))
        .unwrap_or(false)
    {
        return Err("WireGuard profile must use the .conf extension".to_string());
    }

    let content = std::fs::read_to_string(source)
        .map_err(|error| format!("unable to read WireGuard profile: {error}"))?;
    let parsed = validate_profile(&content)?;

    let raw_name = source
        .file_stem()
        .and_then(|value| value.to_str())
        .ok_or("WireGuard profile has an invalid filename")?;
    let name = sanitize_profile_name(raw_name)?;
    let destination = profile_directory(app)?.join(format!("{name}.conf"));
    let temporary = destination.with_extension("conf.part");

    std::fs::write(&temporary, content.as_bytes())
        .map_err(|error| format!("unable to stage WireGuard profile: {error}"))?;
    restrict_to_current_user(&temporary)?;
    if destination.exists() {
        std::fs::remove_file(&destination)
            .map_err(|error| format!("unable to replace existing WireGuard profile: {error}"))?;
    }
    std::fs::rename(&temporary, &destination)
        .map_err(|error| format!("unable to store WireGuard profile: {error}"))?;
    restrict_to_current_user(&destination)?;
    let mut state = read_state(app)?;
    state.verified_profiles.retain(|value| value != &name);
    if state.default_profile.as_deref() == Some(&name) {
        state.default_profile = None;
        state.auto_connect = false;
    }
    write_state(app, &state)?;

    Ok(StoredVpnProfile {
        name,
        active: false,
        addresses: parsed.addresses,
        endpoint: parsed.endpoint,
        allowed_ips: parsed.allowed_ips,
        verified: false,
        is_default: false,
    })
}

pub fn list_profiles(
    app: &AppHandle,
    active_name: Option<&str>,
) -> Result<Vec<StoredVpnProfile>, String> {
    let mut profiles = Vec::new();
    for entry in std::fs::read_dir(profile_directory(app)?)
        .map_err(|error| format!("unable to list WireGuard profiles: {error}"))?
    {
        let entry =
            entry.map_err(|error| format!("unable to inspect WireGuard profile: {error}"))?;
        let path = entry.path();
        if !path.is_file()
            || !path
                .extension()
                .and_then(|value| value.to_str())
                .map(|value| value.eq_ignore_ascii_case("conf"))
                .unwrap_or(false)
        {
            continue;
        }
        let Some(name) = path.file_stem().and_then(|value| value.to_str()) else {
            continue;
        };
        let content = std::fs::read_to_string(&path)
            .map_err(|error| format!("unable to read stored WireGuard profile: {error}"))?;
        let parsed = validate_profile(&content)?;
        let state = read_state(app)?;
        profiles.push(StoredVpnProfile {
            name: name.to_string(),
            active: active_name.map(|value| value == name).unwrap_or(false),
            addresses: parsed.addresses,
            endpoint: parsed.endpoint,
            allowed_ips: parsed.allowed_ips,
            verified: state.verified_profiles.iter().any(|value| value == name),
            is_default: state.default_profile.as_deref() == Some(name),
        });
    }
    profiles.sort_by(|left, right| left.name.to_lowercase().cmp(&right.name.to_lowercase()));
    Ok(profiles)
}

fn state_path(app: &AppHandle) -> Result<PathBuf, String> {
    Ok(profile_directory(app)?
        .parent()
        .ok_or("VPN storage root is unavailable")?
        .join("state.json"))
}

fn read_state(app: &AppHandle) -> Result<ProfileState, String> {
    let path = state_path(app)?;
    if !path.is_file() {
        return Ok(ProfileState::default());
    }
    let text = std::fs::read_to_string(path)
        .map_err(|error| format!("unable to read VPN state: {error}"))?;
    serde_json::from_str(&text).map_err(|error| format!("unable to parse VPN state: {error}"))
}

fn write_state(app: &AppHandle, state: &ProfileState) -> Result<(), String> {
    let path = state_path(app)?;
    let temporary = path.with_extension("json.part");
    let bytes = serde_json::to_vec_pretty(state).map_err(|error| error.to_string())?;
    std::fs::write(&temporary, bytes)
        .map_err(|error| format!("unable to stage VPN state: {error}"))?;
    restrict_to_current_user(&temporary)?;
    std::fs::rename(&temporary, &path)
        .map_err(|error| format!("unable to store VPN state: {error}"))?;
    restrict_to_current_user(&path)
}

pub fn record_verified_connection(app: &AppHandle, name: &str) -> Result<(), String> {
    let name = sanitize_profile_name(name)?;
    profile_path(app, &name)?;
    let mut state = read_state(app)?;
    if !state.verified_profiles.iter().any(|value| value == &name) {
        state.verified_profiles.push(name);
    }
    write_state(app, &state)
}

pub fn select_default(app: &AppHandle, name: &str, auto_connect: bool) -> Result<(), String> {
    let name = sanitize_profile_name(name)?;
    let mut state = read_state(app)?;
    select_default_state(&mut state, &name, auto_connect)?;
    write_state(app, &state)
}

fn select_default_state(
    state: &mut ProfileState,
    name: &str,
    auto_connect: bool,
) -> Result<(), String> {
    if !state.verified_profiles.iter().any(|value| value == name) {
        return Err(
            "default profile: complete a successful manual connection verification first".into(),
        );
    }
    state.default_profile = Some(name.to_string());
    state.auto_connect = auto_connect;
    Ok(())
}

pub fn default_profile(app: &AppHandle) -> Result<Option<(String, bool)>, String> {
    let state = read_state(app)?;
    Ok(state.default_profile.map(|name| (name, state.auto_connect)))
}

pub fn profile_path(app: &AppHandle, name: &str) -> Result<PathBuf, String> {
    let name = sanitize_profile_name(name)?;
    let path = profile_directory(app)?.join(format!("{name}.conf"));
    if !path.is_file() {
        return Err(format!("WireGuard profile '{name}' is not stored"));
    }
    Ok(path)
}

fn sanitize_profile_name(name: &str) -> Result<String, String> {
    let name = name.trim();
    if name.is_empty() || name.len() > 64 {
        return Err("WireGuard profile name must contain 1 to 64 characters".into());
    }
    if !name
        .chars()
        .all(|character| character.is_ascii_alphanumeric() || matches!(character, '-' | '_'))
    {
        return Err(
            "WireGuard profile name may contain only letters, numbers, hyphens, and underscores"
                .into(),
        );
    }
    Ok(name.to_string())
}

#[cfg(target_os = "windows")]
fn restrict_to_current_user(path: &Path) -> Result<(), String> {
    let mut command = Command::new("icacls");
    command
        .arg(path)
        .args(["/inheritance:r", "/grant:r", "%USERNAME%:(F)"])
        .creation_flags(CREATE_NO_WINDOW);
    let output = command
        .output()
        .map_err(|error| format!("unable to secure VPN profile permissions: {error}"))?;
    if output.status.success() {
        Ok(())
    } else {
        Err(format!(
            "unable to secure VPN profile permissions: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ))
    }
}

#[cfg(not(target_os = "windows"))]
fn restrict_to_current_user(_path: &Path) -> Result<(), String> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{sanitize_profile_name, select_default_state, validate_profile, ProfileState};

    #[test]
    fn validates_complete_wireguard_profile() {
        let profile = "[Interface]\nPrivateKey = AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=\nAddress = 10.7.0.2/32\n[Peer]\nPublicKey = AQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQE=\nEndpoint = vpn.example:51820\nAllowedIPs = 10.7.0.1/32";
        assert!(validate_profile(profile).is_ok());
    }

    #[test]
    fn rejects_incomplete_wireguard_profile() {
        let error = validate_profile("[Interface]\nPrivateKey = secret").unwrap_err();
        assert!(error.contains("[Peer]"));
    }

    #[test]
    fn reports_field_specific_errors_without_echoing_secrets() {
        let secret = "not-a-private-key";
        let profile = format!("[Interface]\nPrivateKey = {secret}\nAddress = 10.7.0.2/99\n[Peer]\nPublicKey = AQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQE=\nEndpoint = missing-port\nAllowedIPs = 10.7.0.1/32");
        let error = validate_profile(&profile).unwrap_err();
        assert!(error.starts_with("PrivateKey:"));
        assert!(!error.contains(secret));
    }

    #[test]
    fn rejects_duplicate_sections_and_fields() {
        assert!(
            validate_profile("[Interface]\nPrivateKey=x\n[Interface]\nPrivateKey=y")
                .unwrap_err()
                .contains("duplicate [Interface]")
        );
        assert!(
            validate_profile("[Interface]\nPrivateKey=x\nPrivateKey=y\n[Peer]")
                .unwrap_err()
                .contains("PrivateKey: duplicate")
        );
    }

    #[test]
    fn sanitizes_profile_names() {
        assert_eq!(sanitize_profile_name("Home_VPN").unwrap(), "Home_VPN");
        assert!(sanitize_profile_name("Home VPN").is_err());
        assert!(sanitize_profile_name("...").is_err());
    }

    #[test]
    fn default_requires_prior_successful_manual_verification() {
        let mut state = ProfileState::default();
        let error = select_default_state(&mut state, "home", true).unwrap_err();
        assert!(error.contains("successful manual connection verification"));
        state.verified_profiles.push("home".into());
        select_default_state(&mut state, "home", true).unwrap();
        assert_eq!(state.default_profile.as_deref(), Some("home"));
        assert!(state.auto_connect);
    }
}
