// CinaVault Premium — Built-in VPN bridge.
use std::env::temp_dir;
use std::fs::File;
use std::io::Write;
use std::path::PathBuf;
use std::process::Command;

fn command_exists(command: &str) -> bool {
    Command::new(command)
        .arg("--help")
        .output()
        .map(|_| true)
        .unwrap_or(false)
}

fn generate_wireguard_private_key() -> Result<String, String> {
    let output = Command::new("wg")
        .arg("genkey")
        .output()
        .map_err(|error| format!("WireGuard key generation requires the wg CLI: {}", error))?;

    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).trim().to_string());
    }

    let private_key = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if private_key.is_empty() {
        Err("WireGuard returned an empty private key".to_string())
    } else {
        Ok(private_key)
    }
}

#[cfg(target_os = "windows")]
fn wireguard_exe() -> Option<PathBuf> {
    [
        r"C:\Program Files\WireGuard\wireguard.exe",
        r"C:\Program Files (x86)\WireGuard\wireguard.exe",
    ]
    .iter()
    .map(PathBuf::from)
    .find(|path| path.exists())
}

#[tauri::command]
pub async fn vpnb_status() -> Result<serde_json::Value, String> {
    let wireguard_gui = {
        #[cfg(target_os = "windows")]
        {
            wireguard_exe().is_some()
        }
        #[cfg(not(target_os = "windows"))]
        {
            false
        }
    };

    match Command::new("wg").arg("show").output() {
        Ok(output) => {
            let stdout = String::from_utf8_lossy(&output.stdout).to_string();
            let connected = output.status.success() && !stdout.trim().is_empty();
            Ok(serde_json::json!({
                "installed": true,
                "wgQuickInstalled": command_exists("wg-quick"),
                "wireguardGuiInstalled": wireguard_gui,
                "connected": connected,
                "details": if connected { stdout.clone() } else { "WireGuard is available but no tunnel is active".to_string() },
                "interface": if connected { stdout.lines().next().map(str::to_string) } else { None::<String> },
            }))
        }
        Err(_) => Ok(serde_json::json!({
            "installed": wireguard_gui,
            "wgQuickInstalled": false,
            "wireguardGuiInstalled": wireguard_gui,
            "connected": false,
            "details": if wireguard_gui { "WireGuard desktop service is available" } else { "WireGuard is not installed" },
            "interface": None::<String>,
        })),
    }
}

#[tauri::command]
pub async fn vpnb_connect(config: String) -> Result<serde_json::Value, String> {
    let config_path = temp_dir().join("cinavault_vpn.conf");
    let mut file = File::create(&config_path)
        .map_err(|error| format!("Failed to create VPN config: {}", error))?;
    file.write_all(config.as_bytes())
        .map_err(|error| format!("Failed to write VPN config: {}", error))?;

    let output = if command_exists("wg-quick") {
        Command::new("wg-quick")
            .arg("up")
            .arg(config_path.to_string_lossy().as_ref())
            .output()
    } else {
        #[cfg(target_os = "windows")]
        {
            let exe = wireguard_exe()
                .ok_or("WireGuard is not installed. Install WireGuard for Windows to use built-in VPN.")?;
            Command::new(exe)
                .arg("/installtunnelservice")
                .arg(config_path.to_string_lossy().as_ref())
                .output()
        }
        #[cfg(not(target_os = "windows"))]
        {
            Err(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "wg-quick is not installed",
            ))
        }
    };

    match output {
        Ok(output) => {
            let stderr = String::from_utf8_lossy(&output.stderr).to_string();
            Ok(serde_json::json!({
                "status": if output.status.success() { "connected" } else { "failed" },
                "message": if output.status.success() { "VPN connected successfully" } else { "WireGuard connection failed" },
                "config_path": config_path.to_string_lossy().to_string(),
                "error": if stderr.is_empty() { None } else { Some(stderr) },
            }))
        }
        Err(error) => Err(format!("Failed to start WireGuard: {}", error)),
    }
}

#[tauri::command]
pub async fn vpnb_disconnect() -> Result<serde_json::Value, String> {
    let output = if command_exists("wg-quick") {
        Command::new("wg-quick")
            .arg("down")
            .arg("cinavault_vpn")
            .output()
    } else {
        #[cfg(target_os = "windows")]
        {
            let exe = wireguard_exe()
                .ok_or("WireGuard is not installed. Install WireGuard for Windows to use built-in VPN.")?;
            Command::new(exe)
                .arg("/uninstalltunnelservice")
                .arg("cinavault_vpn")
                .output()
        }
        #[cfg(not(target_os = "windows"))]
        {
            Err(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "wg-quick is not installed",
            ))
        }
    };

    match output {
        Ok(output) => {
            let stderr = String::from_utf8_lossy(&output.stderr).to_string();
            Ok(serde_json::json!({
                "status": if output.status.success() { "disconnected" } else { "failed" },
                "message": if output.status.success() { "VPN disconnected successfully" } else { "WireGuard disconnect failed" },
                "error": if stderr.is_empty() { None } else { Some(stderr) },
            }))
        }
        Err(error) => Err(format!("Failed to stop WireGuard: {}", error)),
    }
}

#[tauri::command]
pub async fn vpnb_generate_test_config(
    endpoint: String,
    public_key: String,
) -> Result<serde_json::Value, String> {
    let private_key = generate_wireguard_private_key()?;
    let config = format!(
        r#"[Interface]
PrivateKey = {}
Address = 10.0.0.2/32
DNS = 1.1.1.1

[Peer]
PublicKey = {}
Endpoint = {}
AllowedIPs = 0.0.0.0/0, ::/0
PersistentKeepalive = 25
"#,
        private_key, public_key, endpoint
    );

    Ok(serde_json::json!({
        "config": config,
        "instructions": "Client private key generated locally with WireGuard.",
    }))
}
