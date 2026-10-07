use crate::server_lifecycle::{self, NATIVE_SERVER_PORT};
use igd_next::{search_gateway, PortMappingProtocol, SearchOptions};
use natpmp::{Natpmp, Protocol, Response};
use serde::Serialize;
use std::net::{IpAddr, Ipv4Addr, SocketAddr, UdpSocket};
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::{Mutex, OnceLock};
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::{Child, Command};
use tokio::sync::{mpsc, oneshot};

const MAPPING_LEASE_SECONDS: u32 = 7_200;
const MAPPING_RENEW_SECONDS: u64 = 3_600;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteConnectivityStatus {
    pub running: bool,
    pub automatic: bool,
    pub port: u16,
    pub direct_available: bool,
    pub direct_method: Option<String>,
    pub direct_url: Option<String>,
    pub public_ip: Option<String>,
    pub relay_active: bool,
    pub relay_mode: Option<String>,
    pub relay_url: Option<String>,
    pub preferred_url: Option<String>,
    pub encrypted_transport_required: bool,
    pub last_error: Option<String>,
}

impl Default for RemoteConnectivityStatus {
    fn default() -> Self {
        Self {
            running: false,
            automatic: true,
            port: NATIVE_SERVER_PORT,
            direct_available: false,
            direct_method: None,
            direct_url: None,
            public_ip: None,
            relay_active: false,
            relay_mode: None,
            relay_url: None,
            preferred_url: None,
            encrypted_transport_required: true,
            last_error: None,
        }
    }
}

struct ConnectivityRuntime {
    status: RemoteConnectivityStatus,
    tunnel: Option<Child>,
    mapping_shutdown: Option<oneshot::Sender<()>>,
    mapping: Option<ActiveMapping>,
}

impl Default for ConnectivityRuntime {
    fn default() -> Self {
        Self {
            status: RemoteConnectivityStatus::default(),
            tunnel: None,
            mapping_shutdown: None,
            mapping: None,
        }
    }
}

#[derive(Clone, Copy)]
enum ActiveMapping {
    Upnp(u16),
    NatPmp(u16),
}

static CLOUDFLARED_PATH: OnceLock<PathBuf> = OnceLock::new();
static CONNECTIVITY_RUNTIME: OnceLock<Mutex<ConnectivityRuntime>> = OnceLock::new();
static CONNECTIVITY_OPERATION: OnceLock<tokio::sync::Mutex<()>> = OnceLock::new();

fn runtime() -> &'static Mutex<ConnectivityRuntime> {
    CONNECTIVITY_RUNTIME.get_or_init(|| Mutex::new(ConnectivityRuntime::default()))
}

fn operation() -> &'static tokio::sync::Mutex<()> {
    CONNECTIVITY_OPERATION.get_or_init(|| tokio::sync::Mutex::new(()))
}

pub fn configure(cloudflared_path: PathBuf) {
    let _ = CLOUDFLARED_PATH.set(cloudflared_path);
}

fn local_ipv4() -> Result<Ipv4Addr, String> {
    let socket = UdpSocket::bind((Ipv4Addr::UNSPECIFIED, 0)).map_err(|error| error.to_string())?;
    socket
        .connect((Ipv4Addr::new(1, 1, 1, 1), 80))
        .map_err(|error| error.to_string())?;
    match socket.local_addr().map_err(|error| error.to_string())?.ip() {
        IpAddr::V4(address) => Ok(address),
        IpAddr::V6(_) => Err("No IPv4 LAN address is available".into()),
    }
}

async fn discover_public_ip() -> Option<String> {
    reqwest::Client::new()
        .get("https://api.ipify.org")
        .timeout(Duration::from_secs(8))
        .send()
        .await
        .ok()?
        .error_for_status()
        .ok()?
        .text()
        .await
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

async fn map_upnp(port: u16) -> Result<String, String> {
    tokio::task::spawn_blocking(move || {
        let local_address = SocketAddr::new(IpAddr::V4(local_ipv4()?), port);
        let gateway =
            search_gateway(SearchOptions::default()).map_err(|error| error.to_string())?;
        gateway
            .add_port(
                PortMappingProtocol::TCP,
                port,
                local_address,
                MAPPING_LEASE_SECONDS,
                "CinaVault Premium Build 170",
            )
            .map_err(|error| error.to_string())?;
        match gateway.get_external_ip() {
            Ok(address) => Ok(address.to_string()),
            Err(error) => {
                let cleanup = gateway
                    .remove_port(PortMappingProtocol::TCP, port)
                    .err()
                    .map(|cleanup_error| format!("; rollback failed ({cleanup_error})"))
                    .unwrap_or_default();
                Err(format!("{error}{cleanup}"))
            }
        }
    })
    .await
    .map_err(|error| error.to_string())?
}

async fn map_nat_pmp(port: u16) -> Result<(), String> {
    tokio::task::spawn_blocking(move || {
        let mut client = Natpmp::new().map_err(|error| error.to_string())?;
        client
            .send_port_mapping_request(Protocol::TCP, port, port, MAPPING_LEASE_SECONDS)
            .map_err(|error| error.to_string())?;
        std::thread::sleep(Duration::from_millis(300));
        let response = match client.read_response_or_retry() {
            Ok(response) => response,
            Err(error) => {
                let rollback = client
                    .send_port_mapping_request(Protocol::TCP, port, port, 0)
                    .err()
                    .map(|cleanup_error| format!("; rollback failed ({cleanup_error})"))
                    .unwrap_or_default();
                return Err(format!("{error}{rollback}"));
            }
        };
        match response {
            Response::TCP(_) => Ok(()),
            _ => {
                let rollback = client
                    .send_port_mapping_request(Protocol::TCP, port, port, 0)
                    .err()
                    .map(|error| format!("; rollback failed ({error})"))
                    .unwrap_or_default();
                Err(format!(
                    "NAT-PMP gateway returned an unexpected response{rollback}"
                ))
            }
        }
    })
    .await
    .map_err(|error| error.to_string())?
}

async fn unmap_upnp(port: u16) -> Result<(), String> {
    tokio::task::spawn_blocking(move || {
        let gateway =
            search_gateway(SearchOptions::default()).map_err(|error| error.to_string())?;
        gateway
            .remove_port(PortMappingProtocol::TCP, port)
            .map_err(|error| error.to_string())
    })
    .await
    .map_err(|error| error.to_string())?
}

async fn unmap_nat_pmp(port: u16) -> Result<(), String> {
    tokio::task::spawn_blocking(move || {
        let mut client = Natpmp::new().map_err(|error| error.to_string())?;
        client
            .send_port_mapping_request(Protocol::TCP, port, port, 0)
            .map_err(|error| error.to_string())?;
        std::thread::sleep(Duration::from_millis(300));
        match client
            .read_response_or_retry()
            .map_err(|error| error.to_string())?
        {
            Response::TCP(_) => Ok(()),
            _ => Err("NAT-PMP gateway returned an unexpected removal response".into()),
        }
    })
    .await
    .map_err(|error| error.to_string())?
}

async fn renew_mapping(method: String, port: u16, mut shutdown: oneshot::Receiver<()>) {
    loop {
        tokio::select! {
            _ = tokio::time::sleep(Duration::from_secs(MAPPING_RENEW_SECONDS)) => {
                let result = if method == "UPnP" {
                    map_upnp(port).await.map(|_| ())
                } else {
                    map_nat_pmp(port).await
                };
                if let Err(error) = result {
                    log::warn!("Automatic {method} mapping renewal failed: {error}");
                }
            }
            _ = &mut shutdown => break,
        }
    }
}

fn parse_quick_tunnel_url(line: &str) -> Option<String> {
    line.split_whitespace()
        .map(|value| {
            value.trim_matches(|character: char| {
                matches!(
                    character,
                    '|' | ',' | ';' | '(' | ')' | '[' | ']' | '"' | '\''
                )
            })
        })
        .find(|value| value.starts_with("https://") && value.contains(".trycloudflare.com"))
        .map(ToString::to_string)
}

async fn read_process_lines<R>(reader: R, sender: mpsc::Sender<String>)
where
    R: tokio::io::AsyncRead + Unpin,
{
    let mut lines = BufReader::new(reader).lines();
    while let Ok(Some(line)) = lines.next_line().await {
        let _ = sender.send(line).await;
    }
}

async fn start_cloud_relay(port: u16) -> Result<(Child, String, String), String> {
    let executable = CLOUDFLARED_PATH
        .get()
        .cloned()
        .ok_or("Cloud relay executable path is not configured")?;
    if !executable.is_file() {
        return Err(format!(
            "Cloud relay executable is missing: {}",
            executable.display()
        ));
    }

    let token = std::env::var("CINAVAULT_CLOUDFLARE_TUNNEL_TOKEN")
        .ok()
        .filter(|value| !value.trim().is_empty());
    let configured_url = std::env::var("CINAVAULT_CLOUDFLARE_PUBLIC_URL")
        .ok()
        .map(|value| value.trim().trim_end_matches('/').to_string())
        .filter(|value| value.starts_with("https://"));

    let mut command = Command::new(executable);
    command
        .kill_on_drop(true)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    if let Some(token) = token {
        let public_url = configured_url
            .ok_or("CINAVAULT_CLOUDFLARE_PUBLIC_URL is required with a named tunnel token")?;
        command.args(["tunnel", "--no-autoupdate", "run", "--token", &token]);
        let child = command
            .spawn()
            .map_err(|error| format!("Unable to start named cloud relay: {error}"))?;
        return Ok((child, public_url, "named".into()));
    }

    command.args([
        "tunnel",
        "--no-autoupdate",
        "--url",
        &format!("http://127.0.0.1:{port}"),
    ]);
    let mut child = command
        .spawn()
        .map_err(|error| format!("Unable to start automatic cloud relay: {error}"))?;

    let stdout = child.stdout.take();
    let stderr = child.stderr.take();
    let (sender, mut receiver) = mpsc::channel::<String>(64);
    if let Some(stdout) = stdout {
        tauri::async_runtime::spawn(read_process_lines(stdout, sender.clone()));
    }
    if let Some(stderr) = stderr {
        tauri::async_runtime::spawn(read_process_lines(stderr, sender));
    }

    let url = tokio::time::timeout(Duration::from_secs(30), async {
        while let Some(line) = receiver.recv().await {
            log::info!("cloudflared: {line}");
            if let Some(url) = parse_quick_tunnel_url(&line) {
                return Some(url);
            }
        }
        None
    })
    .await
    .map_err(|_| "Cloud relay did not publish a URL within 30 seconds".to_string())?
    .ok_or("Cloud relay exited before publishing a URL")?;

    if !url.starts_with("https://") {
        return Err("Cloud relay returned a non-HTTPS endpoint".into());
    }

    Ok((child, url, "quick".into()))
}

async fn verify_external_route(url: &str) -> Result<(), String> {
    let health_url = format!("{}/health", url.trim_end_matches('/'));
    let client = reqwest::Client::new();
    let mut last_error = "route did not answer".to_string();
    for _ in 0..10 {
        match client
            .get(&health_url)
            .timeout(Duration::from_secs(2))
            .send()
            .await
        {
            Ok(response) if response.status().is_success() => {
                match response.json::<serde_json::Value>().await {
                    Ok(body)
                        if body.get("status").and_then(|value| value.as_str()) == Some("ok")
                            && body
                                .get("databaseHealthy")
                                .and_then(|value| value.as_bool())
                                == Some(true) =>
                    {
                        return Ok(())
                    }
                    Ok(_) => last_error = "route returned an unhealthy native server".into(),
                    Err(error) => {
                        last_error = format!("route returned invalid health data: {error}")
                    }
                }
            }
            Ok(response) => last_error = format!("route returned HTTP {}", response.status()),
            Err(error) => last_error = error.to_string(),
        }
        tokio::time::sleep(Duration::from_millis(500)).await;
    }
    Err(format!("External route health check failed: {last_error}"))
}

async fn stop_runtime() -> Result<(), String> {
    let (mut tunnel, mapping_shutdown, mapping) = {
        let mut guard = runtime().lock().map_err(|error| error.to_string())?;
        (
            guard.tunnel.take(),
            guard.mapping_shutdown.take(),
            guard.mapping.take(),
        )
    };
    let mut cleanup_errors = Vec::new();
    if let Some(shutdown) = mapping_shutdown {
        let _ = shutdown.send(());
    }
    if let Some(child) = tunnel.as_mut() {
        if let Err(error) = child.kill().await {
            cleanup_errors.push(format!("Unable to stop cloud relay: {error}"));
        }
    }
    if let Some(mapping) = mapping {
        let result = match mapping {
            ActiveMapping::Upnp(port) => unmap_upnp(port).await,
            ActiveMapping::NatPmp(port) => unmap_nat_pmp(port).await,
        };
        if let Err(error) = result {
            cleanup_errors.push(format!("Unable to remove router mapping: {error}"));
        }
    }
    if !cleanup_errors.is_empty() {
        return Err(cleanup_errors.join("; "));
    }
    Ok(())
}

fn unavailable_status(port: u16, error: String) -> Result<RemoteConnectivityStatus, String> {
    let status = RemoteConnectivityStatus {
        port,
        last_error: Some(error),
        ..RemoteConnectivityStatus::default()
    };
    runtime().lock().map_err(|error| error.to_string())?.status = status.clone();
    Ok(status)
}

#[tauri::command]
pub async fn start_remote_connectivity(
    port: Option<u16>,
    prefer_relay: Option<bool>,
    allow_relay: Option<bool>,
    enable_upnp: Option<bool>,
    enable_nat_pmp: Option<bool>,
) -> Result<RemoteConnectivityStatus, String> {
    let _operation = operation().lock().await;
    let port = port.unwrap_or(NATIVE_SERVER_PORT);
    let prefer_relay = prefer_relay.unwrap_or(true);
    let allow_relay = allow_relay.unwrap_or(true);
    let enable_upnp = enable_upnp.unwrap_or(true);
    let enable_nat_pmp = enable_nat_pmp.unwrap_or(true);
    if let Err(error) = stop_runtime().await {
        return unavailable_status(
            port,
            format!("Previous remote connectivity cleanup failed: {error}"),
        );
    }
    runtime().lock().map_err(|error| error.to_string())?.status = RemoteConnectivityStatus {
        port,
        ..RemoteConnectivityStatus::default()
    };

    let lifecycle = server_lifecycle::configured()?;
    if let Err(error) = lifecycle.start(Some(port)).await {
        return unavailable_status(
            port,
            format!("Native server listener could not start: {error}"),
        );
    }
    let server_health = lifecycle.health().await;
    if !server_health.healthy || server_health.port != Some(port) {
        let reason = server_health
            .error
            .unwrap_or_else(|| "listener or database health check failed".into());
        return unavailable_status(port, format!("Native server is unhealthy: {reason}"));
    }

    let mut status = RemoteConnectivityStatus {
        running: true,
        port,
        ..RemoteConnectivityStatus::default()
    };
    let mut mapping_errors = Vec::<String>::new();

    if enable_upnp {
        match map_upnp(port).await {
            Ok(public_ip) => {
                status.direct_available = true;
                status.direct_method = Some("UPnP".into());
                status.public_ip = Some(public_ip.clone());
                status.direct_url = Some(format!("http://{public_ip}:{port}"));
                runtime().lock().map_err(|error| error.to_string())?.mapping =
                    Some(ActiveMapping::Upnp(port));
            }
            Err(error) => mapping_errors.push(format!("UPnP unavailable ({error})")),
        }
    } else {
        mapping_errors.push("UPnP disabled".into());
    }

    if !status.direct_available {
        if enable_nat_pmp {
            match map_nat_pmp(port).await {
                Ok(()) => {
                    let public_ip = discover_public_ip().await;
                    status.direct_available = true;
                    status.direct_method = Some("NAT-PMP".into());
                    status.public_ip = public_ip.clone();
                    status.direct_url = public_ip.map(|value| format!("http://{value}:{port}"));
                    runtime().lock().map_err(|error| error.to_string())?.mapping =
                        Some(ActiveMapping::NatPmp(port));
                }
                Err(error) => mapping_errors.push(format!("NAT-PMP unavailable ({error})")),
            }
        } else {
            mapping_errors.push("NAT-PMP disabled".into());
        }
    }

    if status.direct_available {
        let method = status
            .direct_method
            .clone()
            .unwrap_or_else(|| "UPnP".into());
        let (shutdown_tx, shutdown_rx) = oneshot::channel();
        tauri::async_runtime::spawn(renew_mapping(method, port, shutdown_rx));
        runtime()
            .lock()
            .map_err(|error| error.to_string())?
            .mapping_shutdown = Some(shutdown_tx);
    }

    if allow_relay || prefer_relay {
        match start_cloud_relay(port).await {
            Ok((child, relay_url, relay_mode)) => {
                status.relay_active = true;
                status.relay_url = Some(relay_url.clone());
                status.relay_mode = Some(relay_mode);
                runtime().lock().map_err(|error| error.to_string())?.tunnel = Some(child);
                if let Err(error) = verify_external_route(&relay_url).await {
                    let cleanup_error = stop_runtime().await.err();
                    let cleanup = cleanup_error
                        .map(|value| format!("; rollback incomplete ({value})"))
                        .unwrap_or_default();
                    return unavailable_status(port, format!("{error}{cleanup}"));
                }
            }
            Err(error) => {
                let mapping_summary = if mapping_errors.is_empty() {
                    "Direct mapping succeeded but is withheld from clients because encrypted transport is required".to_string()
                } else {
                    mapping_errors.join("; ")
                };
                status.last_error = Some(format!(
                    "{mapping_summary}; encrypted cloud relay unavailable ({error})"
                ));
            }
        }
    } else {
        status.last_error =
            Some("Encrypted cloud relay is disabled; no public client URL will be exposed".into());
    }

    status.preferred_url = status
        .relay_url
        .clone()
        .filter(|value| value.starts_with("https://"));

    if status.preferred_url.is_none() {
        status.running = false;
        let cleanup_error = stop_runtime().await.err();
        status.direct_available = false;
        status.direct_method = None;
        status.direct_url = None;
        status.public_ip = None;
        status.relay_active = false;
        status.relay_mode = None;
        status.relay_url = None;
        if let Some(error) = cleanup_error {
            let previous = status.last_error.take().unwrap_or_default();
            status.last_error = Some(format!("{previous}; rollback incomplete ({error})"));
        }
    }

    runtime().lock().map_err(|error| error.to_string())?.status = status.clone();
    Ok(status)
}

#[tauri::command]
pub async fn stop_remote_connectivity() -> Result<RemoteConnectivityStatus, String> {
    let _operation = operation().lock().await;
    let cleanup = stop_runtime().await;
    let status = RemoteConnectivityStatus::default();
    runtime().lock().map_err(|error| error.to_string())?.status = status.clone();
    cleanup?;
    Ok(status)
}

#[tauri::command]
pub fn get_remote_connectivity_status() -> Result<RemoteConnectivityStatus, String> {
    runtime()
        .lock()
        .map(|guard| guard.status.clone())
        .map_err(|error| error.to_string())
}
