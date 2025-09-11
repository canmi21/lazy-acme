/* src/main.rs */

use fancy_log::{LogLevel, log, set_log_level};
use lazy_motd::lazy_motd;

mod acme;
mod config;
mod handlers;
mod init;
mod response;
mod server;
mod state;
mod tasks;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // --- Initialization ---
    let app_config = config::AppConfig::load();
    set_log_level(app_config.log_level);
    lazy_motd!();

    // --- First-time setup check ---
    if init::initialize_app(&app_config).await? {
        log(
            LogLevel::Info,
            "First-time setup complete. Please edit the configuration files and restart.",
        );
        log(
            LogLevel::Info,
            &format!("Configuration directory: {:?}", app_config.dir_path),
        );
        return Ok(());
    }

    // --- Create Shared State and Start Services ---
    log(LogLevel::Info, "Configuration loaded. Starting services...");
    let app_state = state::AppState::new(app_config);

    // Spawn the background task for initial certificate checks.
    // This runs concurrently with the web server.
    tasks::spawn_startup_check_task(app_state.clone());

    // Start the web server. This is a blocking call that will run until a shutdown signal is received.
    server::run_server(app_state).await?;

    log(LogLevel::Info, "Application has shut down gracefully.");
    Ok(())
}
