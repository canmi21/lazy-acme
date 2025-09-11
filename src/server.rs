/* src/server.rs */

use crate::config::AppConfig;
use axum::{Router, routing::get};
use fancy_log::{LogLevel, log};
use std::net::SocketAddr;
use tokio::net::TcpListener;
use tokio::signal;

/// Creates the Axum router and runs the HTTP server.
pub async fn run_server(config: AppConfig) -> Result<(), Box<dyn std::error::Error>> {
    let app = Router::new().route("/", get(hello_world_handler));

    let addr = SocketAddr::from(([127, 0, 0, 1], config.bind_port));
    let listener = TcpListener::bind(&addr).await?;

    log(
        LogLevel::Info,
        &format!("HTTP Server listening on: http://{}", addr),
    );

    // Run the server with a graceful shutdown signal.
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    Ok(())
}

/// The handler for the "/" route.
async fn hello_world_handler() -> &'static str {
    log(LogLevel::Info, "Received a request for /");
    "Hello World"
}

/// Listens for shutdown signals (Ctrl+C, SIGTERM)
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
