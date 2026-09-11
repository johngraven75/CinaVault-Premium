use cinavault_premium_lib::server_lifecycle::NativeServerLifecycle;
use std::path::PathBuf;

fn temporary_database_path(test_name: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "cinavault-{test_name}-{}-{}.db",
        std::process::id(),
        uuid::Uuid::new_v4()
    ))
}

#[tokio::test]
async fn lifecycle_reports_actual_listener_and_database_health_and_stops_idempotently() {
    let database_path = temporary_database_path("native-lifecycle");
    let lifecycle = NativeServerLifecycle::new(database_path.clone());

    let stopped = lifecycle.status().await;
    assert!(!stopped.running);
    assert!(!stopped.healthy);
    assert!(stopped.bound_address.is_none());

    let started = lifecycle
        .start(Some(0))
        .await
        .expect("an ephemeral native server should start");
    assert!(started.running);
    assert!(started.healthy);
    assert!(started.port > 0);
    assert_eq!(
        started.bound_address.as_deref(),
        Some(format!("0.0.0.0:{}", started.port).as_str())
    );

    let health = lifecycle.health().await;
    assert!(health.healthy);
    assert!(health.listener_healthy);
    assert!(health.database_healthy);
    assert_eq!(health.port, Some(started.port));

    lifecycle.stop().await.expect("the server should stop");
    lifecycle.stop().await.expect("stopping twice must be safe");
    assert!(!lifecycle.health().await.healthy);

    let restarted = lifecycle
        .start(Some(0))
        .await
        .expect("the server should restart after a graceful stop");
    assert!(restarted.healthy);
    lifecycle
        .stop()
        .await
        .expect("the restarted server should stop");

    let _ = std::fs::remove_file(database_path);
}

#[tokio::test]
async fn lifecycle_rejects_a_different_port_while_the_listener_is_running() {
    let database_path = temporary_database_path("native-port-authority");
    let lifecycle = NativeServerLifecycle::new(database_path.clone());
    let started = lifecycle
        .start(Some(0))
        .await
        .expect("an ephemeral native server should start");
    let different_port = if started.port == u16::MAX {
        started.port - 1
    } else {
        started.port + 1
    };

    let error = lifecycle
        .start(Some(different_port))
        .await
        .expect_err("one lifecycle must not claim two different listener ports");
    assert!(error.contains("already running"));
    assert!(error.contains(&started.port.to_string()));

    lifecycle.stop().await.expect("the server should stop");
    let _ = std::fs::remove_file(database_path);
}
