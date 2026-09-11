// CinaVault Premium — bundled WireGuard VPN and Windows Defender integration.
use crate::vpn_profile_store;
use std::path::PathBuf;
use std::process::Command;
use std::sync::{Mutex, OnceLock};
use tauri::{AppHandle, Manager};

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct EngineReadiness {
    ready: bool,
    source: &'static str,
    authentic: bool,
    details: String,
}

#[cfg(target_os = "windows")]
use std::os::windows::process::CommandExt;
#[cfg(target_os = "windows")]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

static STARTUP_STATUS: OnceLock<Mutex<serde_json::Value>> = OnceLock::new();

fn startup_status() -> &'static Mutex<serde_json::Value> {
    STARTUP_STATUS.get_or_init(|| Mutex::new(serde_json::json!({ "status": "notAttempted" })))
}

fn wireguard_executable(app: &AppHandle) -> Result<PathBuf, String> {
    let resource_dir = app
        .path()
        .resource_dir()
        .map_err(|error| format!("unable to resolve application resources: {error}"))?;
    let mut candidates = vec![];
    #[cfg(target_os = "windows")]
    {
        if let Some(program_files) = std::env::var_os("ProgramFiles") {
            candidates.push(
                PathBuf::from(program_files)
                    .join("WireGuard")
                    .join("wireguard.exe"),
            );
        }
        if let Some(program_files) = std::env::var_os("ProgramFiles(x86)") {
            candidates.push(
                PathBuf::from(program_files)
                    .join("WireGuard")
                    .join("wireguard.exe"),
            );
        }
    }
    candidates.extend([
        resource_dir
            .join("tools")
            .join("wireguard")
            .join("wireguard.exe"),
        resource_dir.join("wireguard").join("wireguard.exe"),
        resource_dir.join("wireguard.exe"),
    ]);
    candidates
        .into_iter()
        .find(|path| official_engine(path))
        .ok_or_else(|| "official WireGuard engine is missing or failed publisher, product, signature, or size verification".to_string())
}

#[cfg(target_os = "windows")]
fn official_engine(path: &std::path::Path) -> bool {
    if !path.is_file()
        || path
            .metadata()
            .map(|value| value.len() < 1_048_576)
            .unwrap_or(true)
    {
        return false;
    }
    let escaped = path.to_string_lossy().replace('\'', "''");
    let script = format!("$p=Get-Item -LiteralPath '{escaped}';$s=Get-AuthenticodeSignature -LiteralPath '{escaped}';if($p.VersionInfo.ProductName -match 'WireGuard' -and $s.Status -eq 'Valid' -and $s.SignerCertificate.Subject -match 'WireGuard'){{exit 0}}else{{exit 1}}");
    hidden_command("powershell.exe")
        .args(["-NoProfile", "-NonInteractive", "-Command", &script])
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

#[cfg(test)]
fn engine_metadata_is_official(
    size: u64,
    product: &str,
    signer: &str,
    signature_valid: bool,
) -> bool {
    size >= 1_048_576
        && signature_valid
        && product.to_ascii_lowercase().contains("wireguard")
        && signer.to_ascii_lowercase().contains("wireguard")
}

#[cfg(not(target_os = "windows"))]
fn official_engine(_path: &std::path::Path) -> bool {
    false
}

#[cfg(target_os = "windows")]
fn hidden_command(program: impl AsRef<std::ffi::OsStr>) -> Command {
    let mut command = Command::new(program);
    command.creation_flags(CREATE_NO_WINDOW);
    command
}

#[cfg(not(target_os = "windows"))]
fn hidden_command(program: impl AsRef<std::ffi::OsStr>) -> Command {
    Command::new(program)
}

fn tunnel_service_name(profile_name: &str) -> String {
    format!("WireGuardTunnel${profile_name}")
}

fn service_is_running(profile_name: &str) -> bool {
    #[cfg(target_os = "windows")]
    {
        hidden_command("sc.exe")
            .args(["query", &tunnel_service_name(profile_name)])
            .output()
            .map(|output| {
                output.status.success()
                    && String::from_utf8_lossy(&output.stdout)
                        .to_ascii_uppercase()
                        .contains("RUNNING")
            })
            .unwrap_or(false)
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = profile_name;
        false
    }
}

#[cfg(target_os = "windows")]
fn handshake_is_verified(executable: &std::path::Path, profile: &str) -> bool {
    hidden_command(executable)
        .arg("/dumplog")
        .output()
        .ok()
        .filter(|output| output.status.success())
        .map(|output| log_confirms_handshake(&String::from_utf8_lossy(&output.stdout), profile))
        .unwrap_or(false)
}

#[cfg(not(target_os = "windows"))]
fn handshake_is_verified(_executable: &std::path::Path, _profile: &str) -> bool {
    false
}

fn log_confirms_handshake(log: &str, profile: &str) -> bool {
    let log = log.to_ascii_lowercase();
    log.contains(&profile.to_ascii_lowercase())
        && (log.contains("handshake response") || log.contains("latest handshake"))
}

#[tauri::command]
pub async fn vpn_import_profile(
    app: AppHandle,
    source_path: String,
) -> Result<vpn_profile_store::StoredVpnProfile, String> {
    vpn_profile_store::import_profile(&app, &source_path)
}

#[tauri::command]
pub async fn vpn_profiles(
    app: AppHandle,
) -> Result<Vec<vpn_profile_store::StoredVpnProfile>, String> {
    let profiles = vpn_profile_store::list_profiles(&app, None)?;
    Ok(profiles
        .into_iter()
        .map(|mut profile| {
            profile.active = service_is_running(&profile.name);
            profile
        })
        .collect())
}

#[tauri::command]
pub async fn vpn_connect(app: AppHandle, profile: String) -> Result<serde_json::Value, String> {
    #[cfg(target_os = "windows")]
    {
        let executable = wireguard_executable(&app)?;
        let profile_path = vpn_profile_store::profile_path(&app, &profile)?;
        let output = hidden_command(&executable)
            .arg("/installtunnelservice")
            .arg(&profile_path)
            .output()
            .map_err(|error| format!("failed to start bundled WireGuard engine: {error}"))?;
        if !output.status.success() && !service_is_running(&profile) {
            return Err(format!(
                "WireGuard tunnel failed to start: {}",
                String::from_utf8_lossy(&output.stderr).trim()
            ));
        }
        if !service_is_running(&profile) {
            return Err("WireGuard service installation completed but verification did not report a running tunnel".into());
        }
        if !handshake_is_verified(&executable, &profile) {
            let _ = hidden_command(&executable)
                .arg("/uninstalltunnelservice")
                .arg(&profile)
                .output();
            return Err("WireGuard tunnel started but no active handshake was verified; the partial tunnel service was removed".into());
        }
        vpn_profile_store::record_verified_connection(&app, &profile)?;
        Ok(serde_json::json!({
            "status": "connected",
            "profile": profile,
            "service": tunnel_service_name(&profile),
            "engine": executable.to_string_lossy(),
        }))
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = (app, profile);
        Err("bundled WireGuard tunnels are currently supported on Windows only".to_string())
    }
}

#[tauri::command]
pub async fn vpn_disconnect(app: AppHandle) -> Result<serde_json::Value, String> {
    #[cfg(target_os = "windows")]
    {
        let executable = wireguard_executable(&app)?;
        let profiles = vpn_profile_store::list_profiles(&app, None)?;
        let active: Vec<String> = profiles
            .into_iter()
            .filter(|profile| service_is_running(&profile.name))
            .map(|profile| profile.name)
            .collect();
        for profile in &active {
            let output = hidden_command(&executable)
                .arg("/uninstalltunnelservice")
                .arg(profile)
                .output()
                .map_err(|error| format!("failed to stop WireGuard tunnel '{profile}': {error}"))?;
            if !output.status.success() && service_is_running(profile) {
                return Err(format!(
                    "WireGuard tunnel '{profile}' failed to stop: {}",
                    String::from_utf8_lossy(&output.stderr).trim()
                ));
            }
        }
        Ok(serde_json::json!({
            "status": "disconnected",
            "profiles": active,
        }))
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = app;
        Err("bundled WireGuard tunnels are currently supported on Windows only".to_string())
    }
}

#[tauri::command]
pub async fn vpn_status(app: AppHandle) -> Result<serde_json::Value, String> {
    let engine = wireguard_executable(&app).ok();
    let readiness = EngineReadiness {
        ready: engine.is_some(),
        source: if engine.as_ref().is_some_and(|path| {
            path.to_string_lossy()
                .to_ascii_lowercase()
                .contains("program files")
        }) {
            "installed"
        } else if engine.is_some() {
            "bundled"
        } else {
            "missing"
        },
        authentic: engine.is_some(),
        details: if engine.is_some() {
            "Official WireGuard publisher, product, signature, and size verified".into()
        } else {
            "Install the official signed WireGuard client or repair CinaVault resources".into()
        },
    };
    let profiles = vpn_profile_store::list_profiles(&app, None).unwrap_or_default();
    let profile_values: Vec<serde_json::Value> = profiles
        .iter()
        .map(|profile| {
            serde_json::json!({
                "name": profile.name,
                "active": service_is_running(&profile.name),
                "addresses": profile.addresses,
                "endpoint": profile.endpoint,
                "allowedIps": profile.allowed_ips,
                "verified": profile.verified,
                "isDefault": profile.is_default,
            })
        })
        .collect();
    let active_profile = profiles
        .iter()
        .find(|profile| service_is_running(&profile.name))
        .map(|profile| profile.name.clone());
    Ok(serde_json::json!({
        "installed": engine.is_some(),
        "engineBundled": readiness.source == "bundled",
        "engineReadiness": readiness,
        "connected": active_profile.is_some(),
        "activeProfile": active_profile,
        "profiles": profile_values,
        "details": if engine.is_some() {
            "Bundled WireGuard engine ready"
        } else {
            "Bundled WireGuard engine missing from installation"
        },
        "startupStatus": startup_status().lock().map_err(|error| error.to_string())?.clone(),
    }))
}

#[tauri::command]
pub async fn run_antivirus_scan() -> Result<serde_json::Value, String> {
    #[cfg(target_os = "windows")]
    {
        let output = hidden_command("powershell")
            .args(["-NoProfile", "-Command", "Start-MpScan -ScanType QuickScan"])
            .output()
            .map_err(|error| format!("failed to start Windows Defender scan: {error}"))?;
        Ok(serde_json::json!({
            "status": if output.status.success() { "scan_started" } else { "failed" },
            "type": "quick",
            "output": String::from_utf8_lossy(&output.stdout).trim(),
            "error": String::from_utf8_lossy(&output.stderr).trim(),
        }))
    }
    #[cfg(not(target_os = "windows"))]
    {
        Ok(serde_json::json!({
            "status": "unsupported",
            "message": "Windows Defender scan is only available on Windows",
        }))
    }
}

#[tauri::command]
pub async fn update_av_signatures() -> Result<serde_json::Value, String> {
    #[cfg(target_os = "windows")]
    {
        let output = hidden_command("powershell")
            .args(["-NoProfile", "-Command", "Update-MpSignature"])
            .output()
            .map_err(|error| format!("failed to update Windows Defender signatures: {error}"))?;
        Ok(serde_json::json!({
            "status": if output.status.success() { "updated" } else { "failed" },
            "output": String::from_utf8_lossy(&output.stdout).trim(),
            "error": String::from_utf8_lossy(&output.stderr).trim(),
        }))
    }
    #[cfg(not(target_os = "windows"))]
    {
        Ok(serde_json::json!({
            "status": "unsupported",
            "message": "Windows Defender is only available on Windows",
        }))
    }
}

#[tauri::command]
pub async fn install_security_tools(app: AppHandle) -> Result<serde_json::Value, String> {
    match wireguard_executable(&app) {
        Ok(_) => Ok(serde_json::json!({
            "status": "ready",
            "message": "An authentic official WireGuard engine is ready. No installation was performed."
        })),
        Err(error) => Ok(serde_json::json!({
            "status": "action_required",
            "message": "WireGuard is not installed. Download and install the official signed Windows client; administrator approval is required and CinaVault will remain usable if installation is cancelled.",
            "officialDownload": "https://www.wireguard.com/install/",
            "details": error
        })),
    }
}

#[tauri::command]
pub async fn vpn_select_default(
    app: AppHandle,
    profile: String,
    auto_connect: bool,
) -> Result<(), String> {
    vpn_profile_store::select_default(&app, &profile, auto_connect)
}

pub async fn startup_auto_connect(app: AppHandle) -> serde_json::Value {
    let result = startup_auto_connect_inner(app).await;
    if let Ok(mut status) = startup_status().lock() {
        *status = result.clone();
    }
    result
}

async fn startup_auto_connect_inner(app: AppHandle) -> serde_json::Value {
    let selected = match vpn_profile_store::default_profile(&app) {
        Ok(Some((profile, true))) => profile,
        Ok(_) => return serde_json::json!({"status":"disabled"}),
        Err(error) => return serde_json::json!({"status":"failed","error":error}),
    };
    let connect_app = app.clone();
    run_autoconnect_attempt(
        &selected,
        std::time::Duration::from_secs(15),
        || vpn_connect(connect_app, selected.clone()),
        || vpn_disconnect(app),
    )
    .await
}

async fn run_autoconnect_attempt<C, CFut, D, DFut>(
    profile: &str,
    timeout: std::time::Duration,
    connect: C,
    cleanup: D,
) -> serde_json::Value
where
    C: FnOnce() -> CFut,
    CFut: std::future::Future<Output = Result<serde_json::Value, String>>,
    D: FnOnce() -> DFut,
    DFut: std::future::Future<Output = Result<serde_json::Value, String>>,
{
    match tokio::time::timeout(timeout, connect()).await {
        Ok(Ok(value)) => value,
        result => {
            let cleanup_result = cleanup().await;
            let error = match result {
                Ok(Err(error)) => error,
                Err(_) => format!(
                    "WireGuard auto-connect timed out after {} seconds",
                    timeout.as_secs()
                ),
                _ => unreachable!(),
            };
            serde_json::json!({
                "status":"failed",
                "profile":profile,
                "error":error,
                "cleanedUp":cleanup_result.is_ok()
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{engine_metadata_is_official, log_confirms_handshake, run_autoconnect_attempt};
    use std::sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    };

    #[tokio::test]
    async fn failed_autoconnect_cleans_up_and_returns_without_blocking_startup() {
        let cleaned = Arc::new(AtomicBool::new(false));
        let cleanup_flag = cleaned.clone();
        let result = run_autoconnect_attempt(
            "home",
            std::time::Duration::from_millis(50),
            || async { Err("handshake verification failed".to_string()) },
            move || async move {
                cleanup_flag.store(true, Ordering::SeqCst);
                Ok(serde_json::json!({"status":"disconnected"}))
            },
        )
        .await;
        assert_eq!(result["status"], "failed");
        assert_eq!(result["cleanedUp"], true);
        assert!(cleaned.load(Ordering::SeqCst));
    }

    #[tokio::test]
    async fn timed_out_autoconnect_cleans_up() {
        let result = run_autoconnect_attempt(
            "home",
            std::time::Duration::from_millis(5),
            || async {
                tokio::time::sleep(std::time::Duration::from_secs(1)).await;
                Ok(serde_json::json!({"status":"connected"}))
            },
            || async { Ok(serde_json::json!({"status":"disconnected"})) },
        )
        .await;
        assert_eq!(result["status"], "failed");
        assert!(result["error"].as_str().unwrap().contains("timed out"));
    }

    #[test]
    fn engine_readiness_rejects_unsigned_or_wrong_publisher_binaries() {
        assert!(engine_metadata_is_official(
            2_000_000,
            "WireGuard",
            "WireGuard LLC",
            true
        ));
        assert!(!engine_metadata_is_official(
            2_000_000,
            "WireGuard",
            "Unknown Corp",
            true
        ));
        assert!(!engine_metadata_is_official(
            2_000_000,
            "WireGuard",
            "WireGuard LLC",
            false
        ));
        assert!(!engine_metadata_is_official(
            100,
            "WireGuard",
            "WireGuard LLC",
            true
        ));
    }

    #[test]
    fn manual_verification_requires_a_profile_specific_handshake() {
        assert!(log_confirms_handshake(
            "home: Receiving handshake response from peer",
            "home"
        ));
        assert!(!log_confirms_handshake(
            "work: Receiving handshake response from peer",
            "home"
        ));
        assert!(!log_confirms_handshake(
            "home: tunnel service started",
            "home"
        ));
    }
}
