/* src/server.rs */

use crate::{handlers, state::AppState};
use axum::{
    Router,
    routing::{get, post},
};
use fancy_log::{LogLevel, log};
use std::net::SocketAddr;
use tokio::net::TcpListener;
use tokio::signal;

pub async fn run_server(app_state: AppState) -> Result<(), Box<dyn std::error::Error>> {
    let app = Router::new()
        .route("/v1/task", get(handlers::get_task_status))
        .route("/v1/certificate", post(handlers::create_certificate))
        .route("/v1/certificate/{domain}", get(handlers::get_certificate))
        .route(
            "/v1/certificate/{domain}/key",
            get(handlers::get_certificate_key),
        )
        .with_state(app_state.clone());

    let addr = SocketAddr::from(([0, 0, 0, 0], app_state.config.bind_port));
    let listener = TcpListener::bind(&addr).await?;

    log(
        LogLevel::Info,
        &format!("HTTP Server listening on: http://{}", addr),
    );

    axum::serve(listener, app.into_make_service())
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    Ok(())
}

async fn shutdown_signal() {
    let ctrl_c = async {
        signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("failed to install signal handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }

    log(
        LogLevel::Warn,
        "Signal received, starting graceful shutdown...",
    );
}
