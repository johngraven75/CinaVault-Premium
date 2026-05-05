// CinaVault Premium — VPN & Security Module (Windscribe + AV)
use serde::{Deserialize, Serialize};
use std::process::Command;

const VPN_LOCATIONS: &[(&str, &str)] = &[
    ("US East", "US-East"),
    ("US West", "US-West"),
    ("US Central", "US-Central"),
    ("Canada", "CA"),
    ("UK", "GB"),
    ("Netherlands", "NL"),
    ("Germany", "DE"),
    ("France", "FR"),
    ("Switzerland", "CH"),
    ("Hong Kong", "HK"),
];

#[tauri::command]
pub async fn vpn_connect(location: String) -> Result<serde_json::Value, String> {
    let loc_code = VPN_LOCATIONS.iter()
        .find(|(name, _)| *name == location)
        .map(|(_, code)| *code)
        .unwrap_or(&location);

    let output = Command::new("windscribe")
        .args(&["connect", loc_code])
        .output()
        .map_err(|e| format!("Failed to run windscribe: {}. Is Windscribe installed?", e))?;

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();

    Ok(serde_json::json!({
        "status": if output.status.success() { "connected" } else { "failed" },
        "location": location,
        "output": stdout,
        "error": if stderr.is_empty() { None } else { Some(stderr) },
    }))
}

#[tauri::command]
pub async fn vpn_disconnect() -> Result<serde_json::Value, String> {
    let output = Command::new("windscribe")
        .arg("disconnect")
        .output()
        .map_err(|e| format!("Failed to run windscribe: {}", e))?;

    Ok(serde_json::json!({
        "status": if output.status.success() { "disconnected" } else { "failed" },
        "output": String::from_utf8_lossy(&output.stdout).to_string(),
    }))
}

#[tauri::command]
pub async fn vpn_status() -> Result<serde_json::Value, String> {
    let output = Command::new("windscribe")
        .arg("status")
        .output();

    match output {
        Ok(out) => {
            let stdout = String::from_utf8_lossy(&out.stdout).to_string();
            let connected = stdout.to_lowercase().contains("connected");
            Ok(serde_json::json!({
                "installed": true,
                "connected": connected,
                "details": stdout,
                "locations": VPN_LOCATIONS.iter().map(|(name, code)| {
                    serde_json::json!({ "name": name, "code": code })
                }).collect::<Vec<_>>(),
            }))
        }
        Err(_) => {
            Ok(serde_json::json!({
                "installed": false,
                "connected": false,
                "details": "Windscribe CLI not found. Install via windscribe.com",
                "locations": VPN_LOCATIONS.iter().map(|(name, code)| {
                    serde_json::json!({ "name": name, "code": code })
                }).collect::<Vec<_>>(),
            }))
        }
    }
}

#[tauri::command]
pub async fn run_antivirus_scan() -> Result<serde_json::Value, String> {
    #[cfg(target_os = "windows")]
    {
        let output = Command::new("powershell")
            .args(&["-Command", "Start-MpScan -ScanType QuickScan"])
            .output()
            .map_err(|e| format!("Failed to start scan: {}", e))?;

        Ok(serde_json::json!({
            "status": if output.status.success() { "scan_started" } else { "failed" },
            "type": "quick",
            "output": String::from_utf8_lossy(&output.stdout).to_string(),
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
        let output = Command::new("powershell")
            .args(&["-Command", "Update-MpSignature"])
            .output()
            .map_err(|e| format!("Failed to update signatures: {}", e))?;

        Ok(serde_json::json!({
            "status": if output.status.success() { "updated" } else { "failed" },
            "output": String::from_utf8_lossy(&output.stdout).to_string(),
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
pub async fn install_security_tools() -> Result<serde_json::Value, String> {
    #[cfg(target_os = "windows")]
    {
        // Install Windscribe via winget
        let output = Command::new("winget")
            .args(&["install", "--id", "Windscribe.Windscribe", "--accept-package-agreements", "--accept-source-agreements"])
            .output()
            .map_err(|e| format!("winget failed: {}", e))?;

        Ok(serde_json::json!({
            "status": if output.status.success() { "installed" } else { "check_output" },
            "output": String::from_utf8_lossy(&output.stdout).to_string(),
        }))
    }

    #[cfg(not(target_os = "windows"))]
    {
        Ok(serde_json::json!({
            "status": "unsupported",
            "message": "Auto-install is only available on Windows via winget",
        }))
    }
}
