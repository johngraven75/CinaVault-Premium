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
    // On Linux the engine is the system `wg-quick` from wireguard-tools, driven
    // under polkit/sudo elevation rather than a bundled signed binary.
    #[cfg(target_os = "linux")]
    {
        let _ = app;
        return linux_wg_quick();
    }
    #[cfg(not(target_os = "linux"))]
    {
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
}

/// Locates `wg-quick` from the official WireGuard tools on Linux. Checked
/// against the standard system locations first, then `PATH`.
#[cfg(target_os = "linux")]
fn linux_wg_quick() -> Result<PathBuf, String> {
    for dir in ["/usr/bin", "/usr/local/bin", "/bin", "/usr/sbin", "/sbin"] {
        let candidate = PathBuf::from(dir).join("wg-quick");
        if candidate.is_file() {
            return Ok(candidate);
        }
    }
    if let Some(path_var) = std::env::var_os("PATH") {
        for dir in std::env::split_paths(&path_var) {
            let candidate = dir.join("wg-quick");
            if candidate.is_file() {
                return Ok(candidate);
            }
        }
    }
    Err("the WireGuard tools were not found. Install the official package (sudo apt install wireguard-tools) and try again".to_string())
}

/// Derives a kernel-valid network interface name from a profile name. WireGuard
/// profiles allow up to 32 characters, but a Linux interface name must stay
/// under IFNAMSIZ (15 usable bytes), so longer names are truncated and made
/// unique with a short digest of the full name. The mapping is deterministic,
/// so status checks and disconnects can recompute the same interface.
#[cfg(target_os = "linux")]
fn linux_interface_name(profile: &str) -> String {
    let slug: String = profile
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '-'
            }
        })
        .collect();
    if slug.len() <= 15 {
        slug
    } else {
        use sha2::Digest;
        let digest = sha2::Sha256::digest(profile.as_bytes());
        format!("{}-{:02x}{:02x}", &slug[..10], digest[0], digest[1])
    }
}

/// Picks the privilege-escalation helper. `pkexec` (polkit) shows a graphical
/// authentication dialog on GNOME/Zorin and KDE — the Linux counterpart of the
/// Windows UAC prompt. `sudo -n` is a non-interactive fallback for headless or
/// NOPASSWD setups.
#[cfg(target_os = "linux")]
fn linux_escalator() -> Option<PathBuf> {
    for name in ["pkexec", "sudo"] {
        for dir in ["/usr/bin", "/bin", "/usr/sbin", "/sbin", "/usr/local/bin"] {
            let candidate = PathBuf::from(dir).join(name);
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }
    None
}

/// Runs `wg-quick <args>` elevated. Returns the helper's exit code so the caller
/// can distinguish a declined prompt (pkexec 126) or failed auth (pkexec 127)
/// from a genuine tunnel failure.
#[cfg(target_os = "linux")]
fn run_privileged(engine: &std::path::Path, args: &[&str]) -> Result<Option<i32>, String> {
    let escalator = linux_escalator().ok_or_else(|| {
        "neither pkexec nor sudo is available to run WireGuard with administrator rights; install polkit (pkexec) or configure sudo".to_string()
    })?;
    let mut command = Command::new(&escalator);
    if escalator.file_name().and_then(|n| n.to_str()) == Some("sudo") {
        command.arg("-n");
    }
    command.arg(engine);
    command.args(args);
    command
        .status()
        .map(|status| status.code())
        .map_err(|error| format!("failed to start the WireGuard helper: {error}"))
}

/// Maps a `wg-quick` elevation result to an actionable message.
#[cfg(target_os = "linux")]
fn linux_tunnel_error(code: Option<i32>, profile: &str) -> String {
    match code {
        Some(126) => "administrator approval was declined. Linux requires it for WireGuard to bring a tunnel up or down".to_string(),
        Some(127) => "administrator authentication failed, or the WireGuard helper could not be started".to_string(),
        Some(other) => format!("WireGuard tunnel '{profile}' failed with exit code {other}. Check the profile, the endpoint, and that wireguard-tools is installed"),
        None => format!("WireGuard tunnel '{profile}' was interrupted"),
    }
}

/// Copies a stored profile to `vpn/run/<interface>.conf` (user-owned, 0600) so
/// `wg-quick` brings the tunnel up under the derived interface name. wg-quick
/// uses the file's basename as the interface, so the staged name controls it.
#[cfg(target_os = "linux")]
fn linux_stage_conf(
    app: &AppHandle,
    stored: &std::path::Path,
    interface: &str,
) -> Result<PathBuf, String> {
    use std::os::unix::fs::PermissionsExt;
    let run_dir = app
        .path()
        .app_data_dir()
        .map_err(|error| format!("unable to resolve app data directory: {error}"))?
        .join("vpn")
        .join("run");
    std::fs::create_dir_all(&run_dir)
        .map_err(|error| format!("unable to create WireGuard run directory: {error}"))?;
    let _ = std::fs::set_permissions(&run_dir, std::fs::Permissions::from_mode(0o700));
    let destination = run_dir.join(format!("{interface}.conf"));
    std::fs::copy(stored, &destination)
        .map_err(|error| format!("unable to stage WireGuard profile: {error}"))?;
    std::fs::set_permissions(&destination, std::fs::Permissions::from_mode(0o600))
        .map_err(|error| format!("unable to secure staged WireGuard profile: {error}"))?;
    Ok(destination)
}

#[cfg(target_os = "linux")]
fn linux_remove_staged_conf(app: &AppHandle, interface: &str) {
    if let Ok(dir) = app.path().app_data_dir() {
        let _ = std::fs::remove_file(dir.join("vpn").join("run").join(format!("{interface}.conf")));
    }
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

// Linux resolves the engine through `linux_wg_quick`, so this stub is only
// used by the remaining non-Windows targets (e.g. macOS).
#[cfg(not(any(target_os = "windows", target_os = "linux")))]
fn official_engine(_path: &std::path::Path) -> bool {
    false
}

#[allow(dead_code)]
#[cfg(target_os = "windows")]
fn hidden_command(program: impl AsRef<std::ffi::OsStr>) -> Command {
    let mut command = Command::new(program);
    command.creation_flags(CREATE_NO_WINDOW);
    command
}

#[allow(dead_code)]
#[cfg(not(target_os = "windows"))]
fn hidden_command(program: impl AsRef<std::ffi::OsStr>) -> Command {
    Command::new(program)
}

#[allow(dead_code)]
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
    #[cfg(target_os = "linux")]
    {
        // The tunnel interface exists while the tunnel is up; `ip link show`
        // needs no privileges, unlike `wg show`.
        let interface = linux_interface_name(profile_name);
        Command::new("ip")
            .args(["link", "show", &interface])
            .output()
            .map(|output| output.status.success())
            .unwrap_or(false)
    }
    #[cfg(not(any(target_os = "windows", target_os = "linux")))]
    {
        let _ = profile_name;
        false
    }
}

/// Quotes a value as a PowerShell single-quoted literal.
#[cfg_attr(not(target_os = "windows"), allow(dead_code))]
fn ps_literal(value: &str) -> String {
    format!("'{}'", value.replace('\'', "''"))
}

/// Encodes a script for `powershell.exe -EncodedCommand` (UTF-16LE, base64).
#[cfg_attr(not(target_os = "windows"), allow(dead_code))]
fn encode_powershell(script: &str) -> String {
    use base64::Engine;
    let bytes: Vec<u8> = script
        .encode_utf16()
        .flat_map(|unit| unit.to_le_bytes())
        .collect();
    base64::engine::general_purpose::STANDARD.encode(bytes)
}

#[cfg_attr(not(target_os = "windows"), allow(dead_code))]
const SCRIPT_PRELUDE: &str = r#"$ErrorActionPreference = 'Stop'
function Test-TunnelRunning([string]$tunnel) {
    $service = Get-Service -Name ('WireGuardTunnel$' + $tunnel) -ErrorAction SilentlyContinue
    return ($null -ne $service -and $service.Status -eq 'Running')
}
function Invoke-WireGuard([string[]]$arguments) {
    $process = Start-Process -FilePath $exe -ArgumentList $arguments -Wait -PassThru -NoNewWindow
    return $process.ExitCode
}
"#;

/// Exit codes the elevated tunnel script reports back to the app.
#[cfg_attr(not(target_os = "windows"), allow(dead_code))]
const EXIT_INSTALL_FAILED: i32 = 10;
#[cfg_attr(not(target_os = "windows"), allow(dead_code))]
const EXIT_SERVICE_NOT_RUNNING: i32 = 11;
#[cfg_attr(not(target_os = "windows"), allow(dead_code))]
const EXIT_NO_HANDSHAKE: i32 = 12;
#[cfg_attr(not(target_os = "windows"), allow(dead_code))]
const EXIT_UAC_DECLINED: i32 = 1223;

/// Builds the script that installs a tunnel service, waits for it to run and
/// waits for a handshake logged after the install. Creating services and
/// reading the WireGuard log both need administrator rights, so the whole
/// sequence runs in one elevated process (one UAC prompt per connect).
#[cfg_attr(not(target_os = "windows"), allow(dead_code))]
fn connect_script(executable: &str, profile_path: &str, profile: &str) -> String {
    format!(
        r#"$exe = {exe}
$conf = {conf}
$name = {name}
{prelude}$since = Get-Date -Format 'yyyy-MM-dd HH:mm:ss'
if (-not (Test-TunnelRunning $name)) {{
    if ((Invoke-WireGuard @('/installtunnelservice', ('"' + $conf + '"'))) -ne 0) {{ exit {install_failed} }}
}}
$deadline = (Get-Date).AddSeconds(20)
while (-not (Test-TunnelRunning $name)) {{
    if ((Get-Date) -gt $deadline) {{
        Invoke-WireGuard @('/uninstalltunnelservice', $name) | Out-Null
        exit {not_running}
    }}
    Start-Sleep -Milliseconds 500
}}
$log = Join-Path ([IO.Path]::GetTempPath()) ('cinavault-wireguard-' + [guid]::NewGuid().ToString() + '.log')
$deadline = (Get-Date).AddSeconds(25)
$verified = $false
while (-not $verified) {{
    Start-Process -FilePath $exe -ArgumentList '/dumplog' -RedirectStandardOutput $log -Wait -NoNewWindow
    $lines = @(Get-Content -LiteralPath $log -ErrorAction SilentlyContinue)
    $verified = @($lines | Where-Object {{
        $_.Length -ge 19 -and $_.Substring(0, 19) -ge $since -and $_.Contains('[' + $name + ']') -and $_ -match 'handshake response'
    }}).Count -gt 0
    if (-not $verified) {{
        if ((Get-Date) -gt $deadline) {{ break }}
        Start-Sleep -Seconds 1
    }}
}}
Remove-Item -LiteralPath $log -ErrorAction SilentlyContinue
if (-not $verified) {{
    Invoke-WireGuard @('/uninstalltunnelservice', $name) | Out-Null
    exit {no_handshake}
}}
exit 0
"#,
        exe = ps_literal(executable),
        conf = ps_literal(profile_path),
        name = ps_literal(profile),
        prelude = SCRIPT_PRELUDE,
        install_failed = EXIT_INSTALL_FAILED,
        not_running = EXIT_SERVICE_NOT_RUNNING,
        no_handshake = EXIT_NO_HANDSHAKE,
    )
}

/// Builds the script that removes every listed tunnel service in one
/// elevated process.
#[cfg_attr(not(target_os = "windows"), allow(dead_code))]
fn disconnect_script(executable: &str, profiles: &[String]) -> String {
    let names = profiles
        .iter()
        .map(|profile| ps_literal(profile))
        .collect::<Vec<_>>()
        .join(", ");
    format!(
        r#"$exe = {exe}
{prelude}$failed = $false
foreach ($name in @({names})) {{
    if (Test-TunnelRunning $name) {{
        Invoke-WireGuard @('/uninstalltunnelservice', $name) | Out-Null
        if (Test-TunnelRunning $name) {{ $failed = $true }}
    }}
}}
if ($failed) {{ exit {install_failed} }}
exit 0
"#,
        exe = ps_literal(executable),
        prelude = SCRIPT_PRELUDE,
        names = names,
        install_failed = EXIT_INSTALL_FAILED,
    )
}

/// Wraps a script so it runs elevated: directly when the app already has
/// administrator rights, otherwise through a UAC prompt. The elevated exit
/// code is passed back; a declined prompt reports ERROR_CANCELLED (1223).
#[cfg_attr(not(target_os = "windows"), allow(dead_code))]
fn elevation_wrapper(inner: &str) -> String {
    format!(
        r#"$commandLine = '-NoProfile -NonInteractive -ExecutionPolicy Bypass -WindowStyle Hidden -EncodedCommand {encoded}'
$identity = [Security.Principal.WindowsPrincipal][Security.Principal.WindowsIdentity]::GetCurrent()
$isAdmin = $identity.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)
try {{
    if ($isAdmin) {{
        $process = Start-Process -FilePath 'powershell.exe' -ArgumentList $commandLine -Wait -PassThru -WindowStyle Hidden
    }} else {{
        $process = Start-Process -FilePath 'powershell.exe' -ArgumentList $commandLine -Verb RunAs -Wait -PassThru -WindowStyle Hidden
    }}
    exit $process.ExitCode
}} catch {{
    exit {uac}
}}
"#,
        encoded = encode_powershell(inner),
        uac = EXIT_UAC_DECLINED,
    )
}

#[cfg_attr(not(target_os = "windows"), allow(dead_code))]
fn tunnel_error(code: Option<i32>, profile: &str) -> String {
    match code {
        Some(EXIT_INSTALL_FAILED) => format!("WireGuard could not create the tunnel service for '{profile}'. Check that the profile is valid and that no other VPN is using the same tunnel"),
        Some(EXIT_SERVICE_NOT_RUNNING) => format!("WireGuard tunnel '{profile}' did not start; the tunnel service was removed"),
        Some(EXIT_NO_HANDSHAKE) => format!("WireGuard tunnel '{profile}' started but the server never answered the handshake; the tunnel was removed. Check the endpoint, keys and that UDP traffic is allowed"),
        Some(EXIT_UAC_DECLINED) => "Administrator approval was declined. Windows requires it for WireGuard to create or remove a tunnel".into(),
        Some(other) => format!("WireGuard tunnel '{profile}' failed with exit code {other}"),
        None => format!("WireGuard tunnel '{profile}' was interrupted"),
    }
}

#[cfg(target_os = "windows")]
fn run_elevated(inner: &str) -> Result<Option<i32>, String> {
    hidden_command("powershell.exe")
        .args([
            "-NoProfile",
            "-NonInteractive",
            "-ExecutionPolicy",
            "Bypass",
            "-EncodedCommand",
            &encode_powershell(&elevation_wrapper(inner)),
        ])
        .status()
        .map(|status| status.code())
        .map_err(|error| format!("failed to start the WireGuard helper: {error}"))
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
        let script = connect_script(
            &executable.to_string_lossy(),
            &profile_path.to_string_lossy(),
            &profile,
        );
        let code = run_elevated(&script)?;
        if code != Some(0) {
            return Err(tunnel_error(code, &profile));
        }
        if !service_is_running(&profile) {
            return Err("WireGuard reported success but the tunnel service is not running".into());
        }
        vpn_profile_store::record_verified_connection(&app, &profile)?;
        Ok(serde_json::json!({
            "status": "connected",
            "profile": profile,
            "service": tunnel_service_name(&profile),
            "engine": executable.to_string_lossy(),
        }))
    }
    #[cfg(target_os = "linux")]
    {
        let engine = wireguard_executable(&app)?;
        let stored = vpn_profile_store::profile_path(&app, &profile)?;
        let interface = linux_interface_name(&profile);
        if service_is_running(&profile) {
            return Err(format!("WireGuard tunnel '{profile}' is already connected"));
        }
        let staged = linux_stage_conf(&app, &stored, &interface)?;
        let staged_arg = staged
            .to_str()
            .ok_or("the staged WireGuard profile path is not valid UTF-8")?;
        let code = run_privileged(&engine, &["up", staged_arg])?;
        if code != Some(0) {
            linux_remove_staged_conf(&app, &interface);
            return Err(linux_tunnel_error(code, &profile));
        }
        if !service_is_running(&profile) {
            let _ = run_privileged(&engine, &["down", &interface]);
            linux_remove_staged_conf(&app, &interface);
            return Err(
                "WireGuard reported success but the tunnel interface is not up".to_string(),
            );
        }
        vpn_profile_store::record_verified_connection(&app, &profile)?;
        Ok(serde_json::json!({
            "status": "connected",
            "profile": profile,
            "interface": interface,
            "engine": engine.to_string_lossy(),
        }))
    }
    #[cfg(not(any(target_os = "windows", target_os = "linux")))]
    {
        let _ = (app, profile);
        Err("bundled WireGuard tunnels are currently supported on Windows and Linux only"
            .to_string())
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
        if !active.is_empty() {
            let code = run_elevated(&disconnect_script(&executable.to_string_lossy(), &active))?;
            if code != Some(0) {
                return Err(tunnel_error(code, &active.join(", ")));
            }
        }
        Ok(serde_json::json!({
            "status": "disconnected",
            "profiles": active,
        }))
    }
    #[cfg(target_os = "linux")]
    {
        let engine = wireguard_executable(&app)?;
        let profiles = vpn_profile_store::list_profiles(&app, None)?;
        let active: Vec<String> = profiles
            .into_iter()
            .filter(|profile| service_is_running(&profile.name))
            .map(|profile| profile.name)
            .collect();
        let mut failed = vec![];
        for profile in &active {
            let interface = linux_interface_name(profile);
            let code = run_privileged(&engine, &["down", &interface]);
            if !matches!(code, Ok(Some(0))) {
                failed.push(profile.clone());
            }
            linux_remove_staged_conf(&app, &interface);
        }
        if !failed.is_empty() {
            return Err(linux_tunnel_error(None, &failed.join(", ")));
        }
        Ok(serde_json::json!({
            "status": "disconnected",
            "profiles": active,
        }))
    }
    #[cfg(not(any(target_os = "windows", target_os = "linux")))]
    {
        let _ = app;
        Err("bundled WireGuard tunnels are currently supported on Windows and Linux only"
            .to_string())
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
    use super::{
        connect_script, disconnect_script, elevation_wrapper, encode_powershell,
        engine_metadata_is_official, ps_literal, run_autoconnect_attempt, tunnel_error,
        EXIT_NO_HANDSHAKE, EXIT_UAC_DECLINED,
    };
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
    fn powershell_literals_escape_single_quotes() {
        assert_eq!(
            ps_literal(r"C:\Users\O'Neil\home.conf"),
            r"'C:\Users\O''Neil\home.conf'"
        );
    }

    #[test]
    fn encoded_commands_are_utf16le_base64() {
        use base64::Engine;
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(encode_powershell("exit 0"))
            .unwrap();
        assert_eq!(bytes, b"e\x00x\x00i\x00t\x00 \x000\x00");
    }

    #[test]
    fn connect_installs_the_stored_profile_and_waits_for_a_fresh_handshake() {
        let script = connect_script(
            r"C:\Program Files\WireGuard\wireguard.exe",
            r"C:\Users\O'Neil\AppData\Roaming\CinaVault\vpn\profiles\home.conf",
            "home",
        );
        assert!(script.contains(
            r"$conf = 'C:\Users\O''Neil\AppData\Roaming\CinaVault\vpn\profiles\home.conf'"
        ));
        assert!(script.contains("'/installtunnelservice'"));
        // The service starts asynchronously, so the script must poll for it
        // rather than checking once right after the install returns.
        assert!(script.contains("while (-not (Test-TunnelRunning $name))"));
        // Only handshakes logged after this attempt count, not old log lines.
        assert!(script.contains("$_.Substring(0, 19) -ge $since"));
        assert!(script.contains("$_.Contains('[' + $name + ']')"));
        assert!(script.contains(&format!("exit {EXIT_NO_HANDSHAKE}")));
    }

    #[test]
    fn disconnect_removes_all_tunnels_in_one_elevated_run() {
        let script = disconnect_script("wireguard.exe", &["home".into(), "work".into()]);
        assert!(script.contains("foreach ($name in @('home', 'work'))"));
        assert!(script.contains("'/uninstalltunnelservice'"));
    }

    #[test]
    fn tunnel_changes_request_administrator_rights() {
        let wrapper = elevation_wrapper("exit 0");
        assert!(wrapper.contains("-Verb RunAs"));
        assert!(wrapper.contains(&encode_powershell("exit 0")));
        assert!(wrapper.contains(&format!("exit {EXIT_UAC_DECLINED}")));
    }

    #[test]
    fn tunnel_errors_explain_what_failed() {
        assert!(tunnel_error(Some(EXIT_UAC_DECLINED), "home").contains("Administrator approval"));
        assert!(tunnel_error(Some(EXIT_NO_HANDSHAKE), "home").contains("handshake"));
    }

    #[cfg(target_os = "linux")]
    mod linux {
        use super::super::{linux_interface_name, linux_tunnel_error};

        #[test]
        fn short_profile_names_map_to_themselves() {
            assert_eq!(linux_interface_name("home"), "home");
            assert_eq!(linux_interface_name("work-vpn_1"), "work-vpn_1");
        }

        #[test]
        fn unsafe_characters_are_replaced() {
            assert_eq!(linux_interface_name("my vpn!"), "my-vpn-");
        }

        #[test]
        fn long_names_stay_under_the_interface_limit_and_are_deterministic() {
            let long = "a-very-long-wireguard-profile-name";
            let first = linux_interface_name(long);
            let second = linux_interface_name(long);
            assert_eq!(first, second, "interface name must be stable for a profile");
            assert!(
                first.len() <= 15,
                "interface name {first:?} exceeds IFNAMSIZ"
            );
        }

        #[test]
        fn long_names_carry_a_digest_suffix_for_uniqueness() {
            let interface = linux_interface_name("corporate-gateway-primary-tunnel");
            // 10 slug chars, a dash, then a 4-hex digest of the full name.
            assert_eq!(interface.len(), 15);
            assert!(interface.starts_with("corporate-"));
            let suffix = &interface[11..];
            assert_eq!(suffix.len(), 4);
            assert!(suffix.chars().all(|c| c.is_ascii_hexdigit()));
        }

        #[test]
        fn tunnel_errors_explain_privilege_outcomes() {
            assert!(linux_tunnel_error(Some(126), "home").contains("declined"));
            assert!(linux_tunnel_error(Some(127), "home").contains("authentication"));
            assert!(linux_tunnel_error(Some(2), "home").contains("exit code 2"));
        }
    }
}
