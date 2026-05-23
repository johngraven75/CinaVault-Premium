// CinaVault Premium — Built-in VPN Module (WireGuard)
use serde::{Deserialize, Serialize};
use std::net::{IpAddr, Ipv4Addr};
use std::process::Command;
use std::str::FromStr;
use tauri::State;
use crate::AppState;

#[tauri::command]
pub async fn vpnb_status() -> Result<serde_json::Value, String> {
    // Check if WireGuard interface exists
    let output = if cfg!(target_os = "windows") {
        Command::new("wg")
            .arg("show")
            .output()
    } else {
        Command::new("wg")
            .arg("show")
            .output()
    };

    match output {
        Ok(out) => {
            let stdout = String::from_utf8_lossy(&out.stdout).to_string();
            let has_interface = !stdout.trim().is_empty();
            
            Ok(serde_json::json!({
                "installed": true, // We'll ship WireGuard with the app
                "connected": has_interface,
                "details": if has_interface { stdout.clone() } else { "WireGuard available but not connected".to_string() },
                "interface": if has_interface { Some(stdout.lines().next().unwrap_or("").to_string()) } else { None::<String> }
            }))
        }
        Err(_) => {
            Ok(serde_json::json!({
                "installed": false,
                "connected": false,
                "details": "WireGuard not available. Built-in VPN requires administrator privileges.",
                 "interface": None::<String>
            }))
        }
    }
}

#[tauri::command]
pub async fn vpnb_connect(config: String) -> Result<serde_json::Value, String> {
    // Write config to temporary file and start WireGuard
    use std::fs::File;
    use std::io::Write;
    use std::env::temp_dir;
    
    let config_path = temp_dir().join("cinavault_vpn.conf");
    
    // Write the WireGuard config
    let mut file = File::create(&config_path)
        .map_err(|e| format!("Failed to create VPN config: {}", e))?;
    
    file.write_all(config.as_bytes())
        .map_err(|e| format!("Failed to write VPN config: {}", e))?;
    
    // Start WireGuard interface
    let output = if cfg!(target_os = "windows") {
        Command::new("wg-quick")
            .arg("up")
            .arg(config_path.to_string_lossy().as_ref())
            .output()
    } else {
        Command::new("wg-quick")
            .arg("up")
            .arg(config_path.to_string_lossy().as_ref())
            .output()
    };
    
    match output {
        Ok(out) => {
            if out.status.success() {
                Ok(serde_json::json!({
                    "status": "connected",
                    "message": "VPN connected successfully",
                    "config_path": config_path.to_string_lossy().to_string()
                }))
            } else {
                let stderr = String::from_utf8_lossy(&out.stderr).to_string();
                Ok(serde_json::json!({
                    "status": "failed",
                    "message": format!("Failed to connect VPN: {}", stderr),
                    "config_path": config_path.to_string_lossy().to_string()
                }))
            }
        }
        Err(e) => Err(format!("Failed to start WireGuard: {}", e)),
    }
}

#[tauri::command]
pub async fn vpnb_disconnect() -> Result<serde_json::Value, String> {
    // Disconnect WireGuard interface
    let output = if cfg!(target_os = "windows") {
        Command::new("wg-quick")
            .arg("down")
            .arg("cinavault_vpn") // Use interface name
            .output()
    } else {
        Command::new("wg-quick")
            .arg("down")
            .arg("cinavault_vpn")
            .output()
    };
    
    match output {
        Ok(out) => {
            if out.status.success() {
                Ok(serde_json::json!({
                    "status": "disconnected",
                    "message": "VPN disconnected successfully"
                }))
            } else {
                let stderr = String::from_utf8_lossy(&out.stderr).to_string();
                Ok(serde_json::json!({
                    "status": "failed",
                    "message": format!("Failed to disconnect VPN: {}", stderr)
                }))
            }
        }
        Err(e) => Err(format!("Failed to stop WireGuard: {}", e)),
    }
}

// Generate a simple WireGuard config for testing
#[tauri::command]
pub async fn vpnb_generate_test_config(endpoint: String, public_key: String) -> Result<serde_json::Value, String> {
    let config = format!(
        r#"[Interface]
PrivateKey = CINAVAULT_CLIENT_PRIVATE_KEY_PLACEHOLDER
Address = 10.0.0.2/32
DNS = 1.1.1.1

[Peer]
PublicKey = {}
Endpoint = {}
AllowedIPs = 0.0.0.0/0, ::/0
PersistentKeepalive = 25
"#,
        public_key, endpoint
    );
    
    Ok(serde_json::json!({
        "config": config,
        "instructions": "Replace CINAVAULT_CLIENT_PRIVATE_KEY_PLACEHOLDER with actual client private key"
    }))
}