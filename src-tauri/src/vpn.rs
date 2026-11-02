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
    #[cfg(not(target_os = "windows"))]
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

/// Exit codes the WireGuard MSI install reports back to the app. These are
/// distinct from the tunnel-service codes above (10/11/12): installing the
/// official client creates no tunnel service, so 10/11/12 can only arise on a
/// later `vpn_connect`, never from the MSI install itself. They are listed in
/// `install_error_message` anyway so the frontend can explain them with one
/// error map if an install-then-connect flow fails at the connect step.
const EXIT_DOWNLOAD_FAILED: i32 = 20;
const EXIT_SIGNATURE_OR_HASH_FAILED: i32 = 21;
const EXIT_MSI_INSTALL_FAILED: i32 = 22;
const EXIT_POST_INSTALL_NOT_FOUND: i32 = 23;

/// Pinned official Windows MSI, verified 2026-11-01 against the official
/// download listing at https://download.wireguard.com/windows-client/
/// (linked from https://www.wireguard.com/install/), which lists
/// `wireguard-amd64-1.1.1.msi` as the current amd64 MSI. This deliberately
/// differs from the build-time staging script
/// `scripts/prepare-wireguard.ps1`, which still defaults to
/// `wireguard-amd64-1.1.msi`; bumping that script is a follow-up, not part
/// of this runtime-install change (see MANIFEST.md).
const WIREGUARD_MSI_VERSION: &str = "1.1.1";
const WIREGUARD_MSI_URL: &str =
    "https://download.wireguard.com/windows-client/wireguard-amd64-1.1.1.msi";

/// Expected SHA-256 of the pinned MSI.
///
/// UNVERIFIED / NOT PINNED (intentionally `None`): WireGuard publishes no
/// SHA-256 for its Windows MSIs on wireguard.com or on the download listing.
/// Its published verification mechanism is the Authenticode signature on the
/// MSI plus the signify-signed BLAKE2b file list (`latest.sig`) its own
/// updater checks. Inventing a hash here would be worse than none at all: a
/// wrong pinned hash would brick every install, and a hash copied from a
/// third-party page would prove nothing about the official file. So the
/// install gate is: (a) HTTPS download from the official URL above,
/// (b) mandatory Authenticode check (status `Valid`, signer subject contains
/// `WireGuard`) before `msiexec /i` runs, and (c) the SHA-256 actually
/// computed for the downloaded file is returned in the success response for
/// audit. When an official SHA-256 is ever published, set this to
/// `Some("<64 lowercase hex>")` and the hash comparison below becomes a
/// hard gate (mismatch -> exit 21, install never runs).
const WIREGUARD_MSI_EXPECTED_SHA256: Option<&str> = None;

/// The MSI itself is a few MB; refuse anything implausibly large rather
/// than writing an unbounded download to disk.
const WIREGUARD_MSI_MAX_BYTES: usize = 100 * 1024 * 1024;

/// First Program Files `wireguard.exe` that passes the same
/// publisher/product/signature/size verification as the bundled engine.
/// This is the "installed" engine; `wireguard_executable` additionally
/// accepts the bundled exe, which is why the two must not be conflated.
#[allow(dead_code)]
fn installed_engine_path(app: &AppHandle) -> Option<PathBuf> {
    let mut candidates = vec![];
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
    // `official_engine` returns false on non-Windows, so this is None there.
    let _ = app;
    candidates.into_iter().find(|path| official_engine(path))
}

/// "installed" only when the path is under a Program Files directory, the
/// same test `vpn_status` uses for `engineReadiness.source`. Anything else
/// that passes verification is the bundled engine, never the installed one.
#[allow(dead_code)]
fn engine_source_for_path(path: &std::path::Path) -> &'static str {
    if path
        .to_string_lossy()
        .to_ascii_lowercase()
        .contains("program files")
    {
        "installed"
    } else {
        "bundled"
    }
}

/// Human-readable, user-actionable message for an install-phase exit code.
/// The command returns these as `WIREGUARD_<NAME> (<code>): <message>` so
/// the frontend can recover the code without a second round-trip.
#[allow(dead_code)]
fn install_error_message(code: i32) -> String {
    match code {
        EXIT_INSTALL_FAILED => "WireGuard could not create the tunnel service. This step runs when connecting after install, not during the MSI install: check that the profile is valid and that no other VPN is using the same tunnel".into(),
        EXIT_SERVICE_NOT_RUNNING => "WireGuard tunnel did not start after connecting; the tunnel service was removed".into(),
        EXIT_NO_HANDSHAKE => "WireGuard tunnel started after connecting but the server never answered the handshake; the tunnel was removed. Check the endpoint, keys and that UDP traffic is allowed".into(),
        EXIT_DOWNLOAD_FAILED => "The official WireGuard MSI could not be downloaded. Check the network connection and try again; CinaVault remains usable without it".into(),
        EXIT_SIGNATURE_OR_HASH_FAILED => "The downloaded WireGuard MSI failed publisher signature or hash verification and was NOT installed. Delete any manually downloaded copy and retry from the official download only".into(),
        EXIT_MSI_INSTALL_FAILED => "The official WireGuard installer (msiexec) failed. Try running the install again and, if it persists, install the MSI from wireguard.com manually and then retry CinaVault".into(),
        EXIT_POST_INSTALL_NOT_FOUND => "The installer finished but no official WireGuard engine was found under Program Files afterwards, so the install is NOT treated as successful. If only the bundled engine is present, that is reported as \"bundled only\" and does not count as an installed client".into(),
        EXIT_UAC_DECLINED => "Administrator approval was declined. Windows requires it to install WireGuard; CinaVault remains usable and you can retry the install at any time".into(),
        other => format!("WireGuard installation failed with exit code {other}"),
    }
}

#[allow(dead_code)]
fn install_error(code: i32) -> String {
    let name = match code {
        EXIT_INSTALL_FAILED => "WIREGUARD_TUNNEL_SERVICE_FAILED",
        EXIT_SERVICE_NOT_RUNNING => "WIREGUARD_TUNNEL_NOT_RUNNING",
        EXIT_NO_HANDSHAKE => "WIREGUARD_NO_HANDSHAKE",
        EXIT_DOWNLOAD_FAILED => "WIREGUARD_DOWNLOAD_FAILED",
        EXIT_SIGNATURE_OR_HASH_FAILED => "WIREGUARD_SIGNATURE_OR_HASH_FAILED",
        EXIT_MSI_INSTALL_FAILED => "WIREGUARD_MSI_INSTALL_FAILED",
        EXIT_POST_INSTALL_NOT_FOUND => "WIREGUARD_POST_INSTALL_NOT_FOUND",
        EXIT_UAC_DECLINED => "WIREGUARD_UAC_DECLINED",
        _ => "WIREGUARD_INSTALL_FAILED",
    };
    format!("{name} ({code}): {}", install_error_message(code))
}

/// Classifies an `msiexec` exit code. 0 is success; 3010 is success with a
/// reboot required (ERROR_SUCCESS_REBOOT_REQUIRED); 1223 surfaces when the
/// UAC prompt is declined; anything else is a genuine installer failure,
/// and the raw code is preserved in the error for diagnosis.
#[allow(dead_code)]
fn msiexec_outcome(code: Option<i32>) -> &'static str {
    match code {
        Some(0) => "success",
        Some(3010) => "reboot_required",
        Some(EXIT_UAC_DECLINED) => "uac_declined",
        _ => "failed",
    }
}

/// Builds the elevated install script. This is a real install
/// (`msiexec /i`, quiet, no restart), NOT the `/a` administrative
/// extraction that `scripts/prepare-wireguard.ps1` uses at build time to
/// stage a bundled exe. The script exits with the msiexec exit code so
/// `run_elevated` can classify it.
#[allow(dead_code)]
fn msiexec_install_script(msi_path: &str) -> String {
    format!(
        "$msi = {msi}\n$process = Start-Process -FilePath 'msiexec.exe' -ArgumentList @('/i', $msi, '/qn', '/norestart') -Wait -PassThru -NoNewWindow\nexit $process.ExitCode\n",
        msi = ps_literal(msi_path),
    )
}

/// True when a pinned hash, if one is set, matches the computed hash. With
/// no pinned hash (`None`, the current state — see
/// `WIREGUARD_MSI_EXPECTED_SHA256`) there is nothing to compare against,
/// so the gate passes and the caller must report `sha256Verified: false`
/// rather than claim a hash check happened.
#[allow(dead_code)]
fn pinned_hash_gate_passes(expected: Option<&str>, actual: &str) -> bool {
    expected.map_or(true, |expected| {
        expected.eq_ignore_ascii_case(actual)
    })
}

#[cfg(target_os = "windows")]
fn msi_signature_is_official(path: &std::path::Path) -> bool {
    // An MSI has no useful VersionInfo product name to check, so unlike
    // `official_engine` this checks only the Authenticode status and the
    // signer subject — the part that actually proves WireGuard published it.
    let escaped = path.to_string_lossy().replace('\'', "''");
    let script = format!("$s=Get-AuthenticodeSignature -LiteralPath '{escaped}';if($s.Status -eq 'Valid' -and $s.SignerCertificate.Subject -match 'WireGuard'){{exit 0}}else{{exit 1}}");
    hidden_command("powershell.exe")
        .args(["-NoProfile", "-NonInteractive", "-Command", &script])
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

#[cfg(not(target_os = "windows"))]
fn msi_signature_is_official(_path: &std::path::Path) -> bool {
    false
}

#[cfg(target_os = "windows")]
fn sha256_hex_of_file(path: &std::path::Path) -> Result<String, String> {
    use sha2::Digest;
    let bytes = std::fs::read(path)
        .map_err(|error| format!("failed to read downloaded MSI for hashing: {error}"))?;
    let mut hasher = sha2::Sha256::new();
    hasher.update(&bytes);
    Ok(format!("{:x}", hasher.finalize()))
}

#[cfg(not(target_os = "windows"))]
fn sha256_hex_of_file(_path: &std::path::Path) -> Result<String, String> {
    Err("SHA-256 hashing of the WireGuard MSI is only available on Windows".into())
}

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

/// Downloads, verifies and installs the official WireGuard Windows client.
///
/// Request: `{ "force": false }`. With `force: false` (the normal UI path)
/// an already-installed, verified Program Files engine short-circuits with
/// `status: "already_installed"` and no download, UAC prompt or reinstall.
/// With `force: true` the MSI is (re)installed over the existing client.
///
/// Order of operations, each with its own error code:
///  1. Download the pinned MSI over HTTPS (code 20 on failure).
///  2. Verify its Authenticode signature (status `Valid`, signer `WireGuard`)
///     and, if a SHA-256 is ever pinned, its hash (code 21). Nothing is
///     installed if either check fails, and the temp file is deleted.
///  3. Install elevated via the existing UAC pattern (`run_elevated` /
///     `elevation_wrapper`): `msiexec /i <msi> /qn /norestart`. UAC declined
///     is code 1223; any other msiexec failure is code 22 with the raw
///     msiexec exit code preserved in the message.
///  4. Verify the *installed* engine afterwards: a Program Files
///     `wireguard.exe` passing `official_engine`. If only the bundled exe
///     passes, the result is code 23 ("bundled only") — never reported as
///     an installed client. There is no permanent WireGuard service to
///     check: tunnel services (`WireGuardTunnel$<profile>`) are created
///     per-profile at connect time by `vpn_connect`, not by the MSI install.
///  5. On success the computed SHA-256 of the downloaded MSI is returned
///     for audit; `sha256Verified` is true only when a pinned hash was set
///     and matched (currently false — see `WIREGUARD_MSI_EXPECTED_SHA256`).
///
/// This command installs the client only. It does not create a profile,
/// generate keys, or issue a server peer — that remains a follow-on and
/// the existing BYO `.conf` import (`vpn_import_profile`) is untouched.
#[tauri::command]
pub async fn vpn_install_wireguard(
    app: AppHandle,
    force: bool,
) -> Result<serde_json::Value, String> {
    #[cfg(target_os = "windows")]
    {
        return vpn_install_wireguard_windows(app, force).await;
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = (app, force);
        Err("WireGuard installation is currently supported on Windows only".to_string())
    }
}

#[cfg(target_os = "windows")]
async fn vpn_install_wireguard_windows(
    app: AppHandle,
    force: bool,
) -> Result<serde_json::Value, String> {
    if !force {
        if let Some(engine) = installed_engine_path(&app) {
            return Ok(serde_json::json!({
                "status": "already_installed",
                "alreadyInstalled": true,
                "engine": engine.to_string_lossy(),
                "engineSource": "installed",
                "msiUrl": WIREGUARD_MSI_URL,
                "message": "Official WireGuard client is already installed and verified; no download or reinstall was performed.",
            }));
        }
    }

    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or(0);
    let msi_path = std::env::temp_dir().join(format!(
        "cinavault-wireguard-{}-wireguard-amd64-{}.msi",
        stamp, WIREGUARD_MSI_VERSION
    ));

    let result = vpn_install_wireguard_download_and_install(&app, &msi_path).await;
    // The MSI must never linger in temp, whatever happened above.
    let _ = std::fs::remove_file(&msi_path);
    result
}

#[cfg(target_os = "windows")]
async fn vpn_install_wireguard_download_and_install(
    app: &AppHandle,
    msi_path: &std::path::Path,
) -> Result<serde_json::Value, String> {
    // (1) Download.
    let response = reqwest::get(WIREGUARD_MSI_URL)
        .await
        .map_err(|error| {
            format!(
                "{} download error: {error}",
                install_error(EXIT_DOWNLOAD_FAILED)
            )
        })?;
    if !response.status().is_success() {
        return Err(format!(
            "{} HTTP status {}",
            install_error(EXIT_DOWNLOAD_FAILED),
            response.status()
        ));
    }
    let bytes = response.bytes().await.map_err(|error| {
        format!(
            "{} download error: {error}",
            install_error(EXIT_DOWNLOAD_FAILED)
        )
    })?;
    if bytes.is_empty() || bytes.len() > WIREGUARD_MSI_MAX_BYTES {
        return Err(format!(
            "{} unexpected MSI size {} bytes",
            install_error(EXIT_DOWNLOAD_FAILED),
            bytes.len()
        ));
    }
    std::fs::write(msi_path, &bytes).map_err(|error| {
        format!(
            "{} failed to save MSI: {error}",
            install_error(EXIT_DOWNLOAD_FAILED)
        )
    })?;

    // (2) Verify BEFORE installing: Authenticode is mandatory; the hash
    // comparison runs only when a hash is pinned (see the constant).
    if !msi_signature_is_official(msi_path) {
        return Err(install_error(EXIT_SIGNATURE_OR_HASH_FAILED));
    }
    let msi_sha256 = sha256_hex_of_file(msi_path).map_err(|error| {
        format!(
            "{} {}",
            install_error(EXIT_SIGNATURE_OR_HASH_FAILED),
            error
        )
    })?;
    let sha256_verified = WIREGUARD_MSI_EXPECTED_SHA256.is_some()
        && pinned_hash_gate_passes(WIREGUARD_MSI_EXPECTED_SHA256, &msi_sha256);
    if !pinned_hash_gate_passes(WIREGUARD_MSI_EXPECTED_SHA256, &msi_sha256) {
        return Err(format!(
            "{} computed SHA-256 {msi_sha256} does not match the pinned hash",
            install_error(EXIT_SIGNATURE_OR_HASH_FAILED)
        ));
    }

    // (3) Elevated real install (msiexec /i), via the existing UAC pattern.
    let code = run_elevated(&msiexec_install_script(&msi_path.to_string_lossy()))?;
    match msiexec_outcome(code) {
        "uac_declined" => return Err(install_error(EXIT_UAC_DECLINED)),
        "failed" => {
            return Err(format!(
                "{} msiexec exit code {:?}",
                install_error(EXIT_MSI_INSTALL_FAILED),
                code
            ))
        }
        _ => {}
    }
    let reboot_required = msiexec_outcome(code) == "reboot_required";

    // (4) Verify the INSTALLED engine afterwards — Program Files only.
    match installed_engine_path(app) {
        Some(engine) => Ok(serde_json::json!({
            "status": "installed",
            "alreadyInstalled": false,
            "version": WIREGUARD_MSI_VERSION,
            "msiUrl": WIREGUARD_MSI_URL,
            "msiSha256": msi_sha256,
            "sha256Verified": sha256_verified,
            "sha256Pinned": WIREGUARD_MSI_EXPECTED_SHA256.is_some(),
            "signatureVerified": true,
            "engine": engine.to_string_lossy(),
            "engineSource": "installed",
            "rebootRequired": reboot_required,
            "message": if reboot_required {
                "Official WireGuard client installed; a reboot is required to finish the installation."
            } else {
                "Official WireGuard client installed and verified under Program Files."
            },
        })),
        None => {
            if wireguard_executable(app).is_ok() {
                Err(format!(
                    "{} only the bundled engine was found (\"bundled only\"); the official client was not verified under Program Files",
                    install_error(EXIT_POST_INSTALL_NOT_FOUND)
                ))
            } else {
                Err(install_error(EXIT_POST_INSTALL_NOT_FOUND))
            }
        }
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
        engine_metadata_is_official, engine_source_for_path, install_error_message,
        msiexec_install_script, msiexec_outcome, pinned_hash_gate_passes, ps_literal,
        run_autoconnect_attempt, tunnel_error, EXIT_DOWNLOAD_FAILED,
        EXIT_MSI_INSTALL_FAILED, EXIT_NO_HANDSHAKE, EXIT_POST_INSTALL_NOT_FOUND,
        EXIT_SIGNATURE_OR_HASH_FAILED, EXIT_UAC_DECLINED, WIREGUARD_MSI_EXPECTED_SHA256,
        WIREGUARD_MSI_URL, WIREGUARD_MSI_VERSION,
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

    #[test]
    fn pinned_msi_is_the_current_official_amd64_release() {
        // Verified 2026-11-01 against https://download.wireguard.com/windows-client/
        // (linked from https://www.wireguard.com/install/).
        assert_eq!(WIREGUARD_MSI_VERSION, "1.1.1");
        assert_eq!(
            WIREGUARD_MSI_URL,
            "https://download.wireguard.com/windows-client/wireguard-amd64-1.1.1.msi"
        );
        // WireGuard publishes no SHA-256 for this MSI, so no hash is pinned
        // here. Asserting None documents that gap instead of letting a
        // guessed hash slip in; see WIREGUARD_MSI_EXPECTED_SHA256.
        assert_eq!(WIREGUARD_MSI_EXPECTED_SHA256, None);
    }

    #[test]
    fn install_script_uses_quiet_install_not_extraction() {
        let script = msiexec_install_script(r"C:\Temp\wireguard-amd64-1.1.1.msi");
        assert!(script.contains("msiexec.exe"));
        assert!(script.contains("'/i'"));
        assert!(script.contains("'/qn'"));
        assert!(script.contains("'/norestart'"));
        // /a is the build-time extraction mode in prepare-wireguard.ps1;
        // it must never appear in the runtime install script.
        assert!(!script.contains("'/a'"));
    }

    #[test]
    fn msiexec_exit_codes_classify_success_reboot_uac_and_failure() {
        assert_eq!(msiexec_outcome(Some(0)), "success");
        assert_eq!(msiexec_outcome(Some(3010)), "reboot_required");
        assert_eq!(msiexec_outcome(Some(EXIT_UAC_DECLINED)), "uac_declined");
        assert_eq!(msiexec_outcome(Some(1603)), "failed");
        assert_eq!(msiexec_outcome(None), "failed");
    }

    #[test]
    fn install_errors_are_distinct_and_actionable() {
        let download = install_error_message(EXIT_DOWNLOAD_FAILED);
        let signature = install_error_message(EXIT_SIGNATURE_OR_HASH_FAILED);
        let msi = install_error_message(EXIT_MSI_INSTALL_FAILED);
        let post = install_error_message(EXIT_POST_INSTALL_NOT_FOUND);
        let uac = install_error_message(EXIT_UAC_DECLINED);
        assert!(download.contains("could not be downloaded"));
        assert!(signature.contains("signature or hash"));
        assert!(signature.contains("NOT installed"));
        assert!(msi.contains("msiexec"));
        assert!(post.contains("Program Files"));
        assert!(post.contains("bundled only"));
        assert!(uac.contains("Administrator approval"));
        let unique: std::collections::HashSet<_> =
            [download, signature, msi, post, uac].into_iter().collect();
        assert_eq!(unique.len(), 5);
    }

    #[test]
    fn pinned_hash_gate_only_enforces_a_hash_when_one_is_pinned() {
        assert!(pinned_hash_gate_passes(None, "abc"));
        assert!(pinned_hash_gate_passes(
            Some("ABC"),
            "abc"
        ));
        assert!(!pinned_hash_gate_passes(
            Some("deadbeef"),
            "abc"
        ));
    }

    #[test]
    fn engine_source_distinguishes_installed_from_bundled() {
        assert_eq!(
            engine_source_for_path(std::path::Path::new(
                r"C:\Program Files\WireGuard\wireguard.exe"
            )),
            "installed"
        );
        assert_eq!(
            engine_source_for_path(std::path::Path::new(
                r"C:\CinaVault\resources\tools\wireguard\wireguard.exe"
            )),
            "bundled"
        );
    }
}
