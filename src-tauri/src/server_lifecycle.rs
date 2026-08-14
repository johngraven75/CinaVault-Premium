use crate::db::Database;
use crate::embedded_server;
use serde::{Deserialize, Serialize};
use std::net::{Ipv4Addr, SocketAddr};
use std::path::PathBuf;
use std::sync::{Arc, OnceLock};
use std::time::Duration;
use tokio::sync::{oneshot, Mutex};
use tokio::task::JoinHandle;

pub const NATIVE_SERVER_PORT: u16 = 32400;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NativeServerStatus {
    pub running: bool,
    pub healthy: bool,
    pub port: u16,
    pub bound_address: Option<String>,
    pub local_url: String,
    pub database_healthy: bool,
    pub remote_ready: bool,
    pub authentication: &'static str,
    pub remote_transport: &'static str,
    pub local_paths_exposed: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NativeServerHealth {
    pub healthy: bool,
    pub listener_healthy: bool,
    pub database_healthy: bool,
    pub port: Option<u16>,
    pub bound_address: Option<String>,
    pub error: Option<String>,
}

struct ServerRuntime {
    id: u64,
    bound_address: SocketAddr,
    shutdown: oneshot::Sender<()>,
    task: JoinHandle<()>,
}

struct LifecycleState {
    next_id: u64,
    runtime: Option<ServerRuntime>,
}

#[derive(Clone)]
pub struct NativeServerLifecycle {
    database_path: PathBuf,
    state: Arc<Mutex<LifecycleState>>,
    operation: Arc<Mutex<()>>,
    health_client: reqwest::Client,
}

impl NativeServerLifecycle {
    pub fn new(database_path: PathBuf) -> Self {
        Self {
            database_path,
            state: Arc::new(Mutex::new(LifecycleState {
                next_id: 1,
                runtime: None,
            })),
            operation: Arc::new(Mutex::new(())),
            health_client: reqwest::Client::new(),
        }
    }

    pub fn database_path(&self) -> &PathBuf {
        &self.database_path
    }

    pub async fn start(&self, port: Option<u16>) -> Result<NativeServerStatus, String> {
        let _operation = self.operation.lock().await;
        let active_port = self
            .state
            .lock()
            .await
            .runtime
            .as_ref()
            .map(|runtime| runtime.bound_address.port());
        if let Some(active_port) = active_port {
            if let Some(requested_port) = port.filter(|requested| *requested != 0) {
                if requested_port != active_port {
                    return Err(format!(
                        "Native server is already running on port {active_port}; stop it before requesting port {requested_port}"
                    ));
                }
            }
            return Ok(self.status().await);
        }

        let database_path = self.database_path_string()?;
        Database::new(&database_path)
            .map_err(|error| format!("Native server database is unavailable: {error}"))?;

        let requested_port = port.unwrap_or(NATIVE_SERVER_PORT);
        let listener = tokio::net::TcpListener::bind((Ipv4Addr::UNSPECIFIED, requested_port))
            .await
            .map_err(|error| {
                format!("Unable to bind native server on port {requested_port}: {error}")
            })?;
        let bound_address = listener
            .local_addr()
            .map_err(|error| format!("Unable to inspect native server listener: {error}"))?;
        let router = embedded_server::router(database_path);
        let (shutdown_tx, shutdown_rx) = oneshot::channel();

        let (runtime_id, state) = {
            let mut state = self.state.lock().await;
            let runtime_id = state.next_id;
            state.next_id = state.next_id.saturating_add(1);
            (runtime_id, Arc::clone(&self.state))
        };

        let task = tokio::spawn(async move {
            let result = axum::serve(listener, router)
                .with_graceful_shutdown(async {
                    let _ = shutdown_rx.await;
                })
                .await;
            if let Err(error) = result {
                log::error!("Native media server stopped unexpectedly: {error}");
            }
            let mut state = state.lock().await;
            if state.runtime.as_ref().map(|runtime| runtime.id) == Some(runtime_id) {
                state.runtime = None;
            }
        });

        self.state.lock().await.runtime = Some(ServerRuntime {
            id: runtime_id,
            bound_address,
            shutdown: shutdown_tx,
            task,
        });

        for _ in 0..40 {
            let health = self.health().await;
            if health.healthy {
                return Ok(self.status_from_health(health));
            }
            tokio::time::sleep(Duration::from_millis(25)).await;
        }

        let health = self.health().await;
        let reason = health
            .error
            .unwrap_or_else(|| "listener did not become healthy".to_string());
        let _ = self.stop_runtime().await;
        Err(format!(
            "Native server failed its startup health check: {reason}"
        ))
    }

    pub async fn stop(&self) -> Result<NativeServerStatus, String> {
        let _operation = self.operation.lock().await;
        self.stop_runtime().await
    }

    async fn stop_runtime(&self) -> Result<NativeServerStatus, String> {
        let runtime = self.state.lock().await.runtime.take();
        if let Some(runtime) = runtime {
            let _ = runtime.shutdown.send(());
            runtime
                .task
                .await
                .map_err(|error| format!("Native server shutdown task failed: {error}"))?;
        }
        Ok(self.stopped_status())
    }

    pub async fn status(&self) -> NativeServerStatus {
        let health = self.health().await;
        if health.port.is_some() {
            self.status_from_health(health)
        } else {
            self.stopped_status()
        }
    }

    pub async fn health(&self) -> NativeServerHealth {
        let (bound_address, port) = {
            let state = self.state.lock().await;
            match state.runtime.as_ref() {
                Some(runtime) => (
                    Some(runtime.bound_address.to_string()),
                    runtime.bound_address.port(),
                ),
                None => {
                    return NativeServerHealth {
                        healthy: false,
                        listener_healthy: false,
                        database_healthy: false,
                        port: None,
                        bound_address: None,
                        error: Some("Native server is stopped".into()),
                    };
                }
            }
        };

        let url = format!("http://127.0.0.1:{port}/health");
        match self
            .health_client
            .get(url)
            .timeout(Duration::from_secs(2))
            .send()
            .await
        {
            Ok(response) if response.status().is_success() => {
                match response.json::<HealthResponse>().await {
                    Ok(response) => NativeServerHealth {
                        healthy: response.status == "ok" && response.database_healthy,
                        listener_healthy: true,
                        database_healthy: response.database_healthy,
                        port: Some(port),
                        bound_address,
                        error: response.error,
                    },
                    Err(error) => NativeServerHealth {
                        healthy: false,
                        listener_healthy: true,
                        database_healthy: false,
                        port: Some(port),
                        bound_address,
                        error: Some(format!("Invalid native server health response: {error}")),
                    },
                }
            }
            Ok(response) => NativeServerHealth {
                healthy: false,
                listener_healthy: true,
                database_healthy: false,
                port: Some(port),
                bound_address,
                error: Some(format!(
                    "Native server health returned HTTP {}",
                    response.status()
                )),
            },
            Err(error) => NativeServerHealth {
                healthy: false,
                listener_healthy: false,
                database_healthy: false,
                port: Some(port),
                bound_address,
                error: Some(format!("Native server listener is unavailable: {error}")),
            },
        }
    }

    fn database_path_string(&self) -> Result<String, String> {
        self.database_path
            .to_str()
            .map(ToString::to_string)
            .ok_or_else(|| "Native server database path is not valid UTF-8".to_string())
    }

    fn stopped_status(&self) -> NativeServerStatus {
        NativeServerStatus {
            running: false,
            healthy: false,
            port: NATIVE_SERVER_PORT,
            bound_address: None,
            local_url: format!("http://127.0.0.1:{NATIVE_SERVER_PORT}"),
            database_healthy: false,
            remote_ready: false,
            authentication: "CinaVault account session",
            remote_transport: "HTTPS relay required by default",
            local_paths_exposed: false,
        }
    }

    fn status_from_health(&self, health: NativeServerHealth) -> NativeServerStatus {
        let port = health.port.unwrap_or(NATIVE_SERVER_PORT);
        NativeServerStatus {
            running: health.listener_healthy,
            healthy: health.healthy,
            port,
            bound_address: health.bound_address,
            local_url: format!("http://127.0.0.1:{port}"),
            database_healthy: health.database_healthy,
            remote_ready: false,
            authentication: "CinaVault account session",
            remote_transport: "HTTPS relay required by default",
            local_paths_exposed: false,
        }
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct HealthResponse {
    status: String,
    database_healthy: bool,
    error: Option<String>,
}

static CONFIGURED_LIFECYCLE: OnceLock<NativeServerLifecycle> = OnceLock::new();

pub fn configure(database_path: PathBuf) -> Result<(), String> {
    if let Some(configured) = CONFIGURED_LIFECYCLE.get() {
        return if configured.database_path() == &database_path {
            Ok(())
        } else {
            Err("Native server database is already configured with a different path".into())
        };
    }
    CONFIGURED_LIFECYCLE
        .set(NativeServerLifecycle::new(database_path))
        .map_err(|_| "Native server database configuration raced with another caller".to_string())
}

pub fn configured() -> Result<&'static NativeServerLifecycle, String> {
    CONFIGURED_LIFECYCLE
        .get()
        .ok_or_else(|| "Native server database is not configured".to_string())
}

#[tauri::command]
pub async fn start_embedded_server(port: Option<u16>) -> Result<NativeServerStatus, String> {
    configured()?.start(port).await
}

#[tauri::command]
pub async fn stop_embedded_server() -> Result<NativeServerStatus, String> {
    configured()?.stop().await
}

#[tauri::command]
pub async fn get_embedded_server_status() -> Result<NativeServerStatus, String> {
    Ok(configured()?.status().await)
}

#[tauri::command]
pub async fn get_embedded_server_health() -> Result<NativeServerHealth, String> {
    Ok(configured()?.health().await)
}
