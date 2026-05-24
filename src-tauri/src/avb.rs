// CinaVault Premium — Built-in antivirus bridge.
use std::process::Command;

fn run_powershell(script: &str) -> Result<(bool, String, String), String> {
    let output = Command::new("powershell")
        .args([
            "-NoProfile",
            "-ExecutionPolicy",
            "Bypass",
            "-Command",
            script,
        ])
        .output()
        .map_err(|error| format!("PowerShell failed: {}", error))?;

    Ok((
        output.status.success(),
        String::from_utf8_lossy(&output.stdout).trim().to_string(),
        String::from_utf8_lossy(&output.stderr).trim().to_string(),
    ))
}

#[tauri::command]
pub async fn avb_status() -> Result<serde_json::Value, String> {
    #[cfg(target_os = "windows")]
    {
        let (success, stdout, stderr) = run_powershell(
            "Get-MpComputerStatus | Select-Object AMServiceEnabled,AntivirusEnabled,RealTimeProtectionEnabled,AntivirusSignatureLastUpdated,AntivirusSignatureVersion | ConvertTo-Json -Compress",
        )?;
        Ok(serde_json::json!({
            "installed": success,
            "engine": "Microsoft Defender Antivirus",
            "status": if success { "ready" } else { "error" },
            "details": stdout,
            "error": if stderr.is_empty() { None } else { Some(stderr) },
        }))
    }

    #[cfg(not(target_os = "windows"))]
    {
        Ok(serde_json::json!({
            "installed": false,
            "engine": "Microsoft Defender Antivirus",
            "status": "unsupported",
            "message": "Built-in antivirus uses Windows Defender on Windows builds.",
        }))
    }
}

#[tauri::command]
pub async fn avb_scan_path(path: String) -> Result<serde_json::Value, String> {
    #[cfg(target_os = "windows")]
    {
        let escaped = path.replace('\'', "''");
        let script = format!("Start-MpScan -ScanPath '{}' -ScanType CustomScan", escaped);
        let (success, stdout, stderr) = run_powershell(&script)?;
        Ok(serde_json::json!({
            "status": if success { "scan_started" } else { "failed" },
            "path": path,
            "engine": "Microsoft Defender Antivirus",
            "output": stdout,
            "error": if stderr.is_empty() { None } else { Some(stderr) },
        }))
    }

    #[cfg(not(target_os = "windows"))]
    {
        Ok(serde_json::json!({
            "status": "unsupported",
            "path": path,
            "message": "Built-in antivirus scans are available on Windows builds.",
        }))
    }
}

#[tauri::command]
pub async fn avb_update_database() -> Result<serde_json::Value, String> {
    #[cfg(target_os = "windows")]
    {
        let (success, stdout, stderr) = run_powershell("Update-MpSignature")?;
        Ok(serde_json::json!({
            "status": if success { "updated" } else { "failed" },
            "engine": "Microsoft Defender Antivirus",
            "output": stdout,
            "error": if stderr.is_empty() { None } else { Some(stderr) },
        }))
    }

    #[cfg(not(target_os = "windows"))]
    {
        Ok(serde_json::json!({
            "status": "unsupported",
            "message": "Signature updates are available on Windows builds.",
        }))
    }
}

#[tauri::command]
pub async fn avb_install_tools() -> Result<serde_json::Value, String> {
    avb_status().await
}
