// CinaVault Premium — bundled WireGuard VPN and Windows Defender integration.
use crate::vpn_profile_store;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::thread;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Manager};

#[cfg(target_os = "windows")]
use std::os::windows::process::CommandExt;
#[cfg(target_os = "windows")]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

fn wireguard_executable(app: &AppHandle) -> Result<PathBuf, String> {
    let resource_dir = app
        .path()
        .resource_dir()
        .map_err(|error| format!("unable to resolve application resources: {error}"))?;
    let candidates = [
        resource_dir.join("tools").join("wireguard").join("wireguard.exe"),
        resource_dir.join("wireguard").join("wireguard.exe"),
        resource_dir.join("wireguard.exe"),
    ];
    candidates
        .into_iter()
        .find(|path| path.is_file())
        .ok_or_else(|| format!(
            "bundled WireGuard engine is missing from this installation (resources: {})",
            resource_dir.display()
        ))
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
fn powershell_quote(value: &Path) -> String {
    value.to_string_lossy().replace(''', "''")
}

#[cfg(target_os = "windows")]
fn run_wireguard_elevated(executable: &Path, arguments: &[String]) -> Result<(), String> {
    let quoted_executable = powershell_quote(executable);
    let argument_list = arguments
        .iter()
        .map(|value| format!("'{}'", value.replace(''', "''")))
        .collect::<Vec<_>>()
        .join(",");
    let script = format!(
        "$p = Start-Process -FilePath '{}' -ArgumentList @({}) -Verb RunAs -Wait -PassThru; exit $p.ExitCode",
        quoted_executable, argument_list
    );
    let output = hidden_command("powershell.exe")
        .args(["-NoProfile", "-NonInteractive", "-ExecutionPolicy", "Bypass", "-Command", &script])
        .output()
        .map_err(|error| format!("unable to request administrator permission for WireGuard: {error}"))?;
    if output.status.success() {
        Ok(())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        Err(if stderr.is_empty() {
            "WireGuard administrator request was denied or the tunnel command failed".to_string()
        } else {
            format!("WireGuard elevated command failed: {stderr}")
        })
    }
}

#[cfg(target_os = "windows")]
fn wait_for_service(profile_name: &str, expected_running: bool) -> bool {
    let deadline = Instant::now() + Duration::from_secs(15);
    while Instant::now() < deadline {
        if service_is_running(profile_name) == expected_running {
            return true;
        }
        thread::sleep(Duration::from_millis(500));
    }
    service_is_running(profile_name) == expected_running
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
        if !profile_path.is_file() {
            return Err(format!("VPN profile file is missing: {}", profile_path.display()));
        }
        if service_is_running(&profile) {
            return Ok(serde_json::json!({
                "status": "connected",
                "profile": profile,
                "service": tunnel_service_name(&profile),
                "engine": executable.to_string_lossy(),
                "alreadyRunning": true,
            }));
        }
        run_wireguard_elevated(
            &executable,
            &[
                "/installtunnelservice".to_string(),
                profile_path.to_string_lossy().to_string(),
            ],
        )?;
        if !wait_for_service(&profile, true) {
            return Err(format!(
                "WireGuard accepted the request but tunnel service '{}' did not reach RUNNING state. Check the profile keys, endpoint, and administrator approval.",
                tunnel_service_name(&profile)
            ));
        }
        Ok(serde_json::json!({
            "status": "connected",
            "profile": profile,
            "service": tunnel_service_name(&profile),
            "engine": executable.to_string_lossy(),
            "alreadyRunning": false,
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
            run_wireguard_elevated(
                &executable,
                &["/uninstalltunnelservice".to_string(), profile.clone()],
            )?;
            if !wait_for_service(profile, false) {
                return Err(format!(
                    "WireGuard tunnel '{}' did not stop after administrator approval",
                    profile
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
    let profiles = vpn_profile_store::list_profiles(&app, None).unwrap_or_default();
    let profile_values: Vec<serde_json::Value> = profiles
        .iter()
        .map(|profile| {
            serde_json::json!({
                "name": profile.name,
                "path": profile.path,
                "active": service_is_running(&profile.name),
            })
        })
        .collect();
    let active_profile = profiles
        .iter()
        .find(|profile| service_is_running(&profile.name))
        .map(|profile| profile.name.clone());
    Ok(serde_json::json!({
        "installed": engine.is_some(),
        "engineBundled": engine.is_some(),
        "enginePath": engine.as_ref().map(|path| path.to_string_lossy().to_string()),
        "connected": active_profile.is_some(),
        "activeProfile": active_profile,
        "profiles": profile_values,
        "details": if engine.is_some() {
            "Bundled WireGuard engine ready; connecting will request administrator approval"
        } else {
            "Bundled WireGuard engine missing from installation"
        },
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
    let executable = wireguard_executable(&app)?;
    Ok(serde_json::json!({
        "status": "bundled",
        "message": "WireGuard is bundled with CinaVault. Connecting a tunnel requests Windows administrator approval.",
        "engine": executable.to_string_lossy(),
    }))
}
