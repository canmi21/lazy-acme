/* src/main.rs */

use fancy_log::{LogLevel, log, set_log_level};
use lazy_motd::lazy_motd;

// Declare the new acme module
mod acme;
mod config;
mod init;
mod server;
mod tasks;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // --- Initialization ---
    let config = config::AppConfig::load();
    set_log_level(config.log_level);
    lazy_motd!();

    // --- First-time setup check ---
    let is_first_run = init::initialize_app(&config).await?;
    if is_first_run {
        log(
            LogLevel::Info,
            "First-time setup complete. Please edit the configuration files and restart.",
        );
        log(
            LogLevel::Info,
            &format!("Configuration directory: {:?}", config.dir_path),
        );
        return Ok(());
    }

    // --- Startup Certificate Acquisition ---
    log(LogLevel::Info, "Checking for missing certificates...");
    if let Err(e) = acme::check_and_acquire_certs_on_startup(&config).await {
        log(
            LogLevel::Error,
            &format!(
                "An error occurred during certificate acquisition: {}. Halting.",
                e
            ),
        );
        // We return the error to stop the application if certificate acquisition fails.
        return Err(e);
    }
    log(LogLevel::Info, "Certificate check complete.");

    // --- Start Application Logic (if not first run) ---
    log(LogLevel::Info, "Configuration loaded. Starting services...");

    // Spawn background tasks
    tasks::spawn_cert_check_task(config.clone());

    // Start the web server (this is a blocking call until shutdown)
    server::run_server(config).await?;

    log(LogLevel::Info, "Application has shut down gracefully.");
    Ok(())
}
