/* src/tasks.rs */

use crate::{
    acme, config,
    state::{AppState, DomainStatus},
};
use fancy_log::{LogLevel, log};
use tokio::time;

/// Spawns the initial startup task to check certificates from config.toml.
pub fn spawn_startup_check_task(app_state: AppState) {
    tokio::spawn(async move {
        log(LogLevel::Info, "Starting initial certificate check...");
        let config = app_state.config.clone();
        let domain_config_path = config.dir_path.join("config.toml");

        let domain_config = match config::load_domain_config(&domain_config_path).await {
            Ok(c) => c,
            Err(e) => {
                log(
                    LogLevel::Error,
                    &format!("Failed to load domain config on startup: {}", e),
                );
                return; // Cannot proceed
            }
        };

        // Populate the initial state for domains that already have certificates
        for domain in &domain_config.domains {
            let domain_name = domain.name.trim().to_string();
            if acme::certificate_exists(&domain_name, &config).await {
                app_state
                    .domains
                    .write()
                    .insert(domain_name, DomainStatus::Ready);
            }
        }

        // Now, start acquisition for any missing certs
        let mut all_successful = true;
        for domain in domain_config.domains {
            let domain_name = domain.name.trim();
            if !acme::certificate_exists(domain_name, &config).await {
                // This call will run in the foreground of this task and block its progress
                acme::acquire_certificate(
                    app_state.clone(),
                    domain.name.clone(), // <-- FIX: Clone the string to move it
                    domain.dns_provider.clone(), // <-- FIX: Clone the string to move it
                    false,               // Do not persist, it's already in the config
                )
                .await;
                // Check the result after the attempt
                if let Some(DomainStatus::Failed(_)) = app_state.domains.read().get(domain_name) {
                    all_successful = false;
                }
            }
        }

        log(LogLevel::Info, "Initial certificate check complete.");

        if all_successful {
            log(
                LogLevel::Info,
                "All startup checks passed. Activating periodic renewal task.",
            );
            *app_state.task_running.write() = true;
            spawn_periodic_renewal_task(app_state);
        } else {
            log(
                LogLevel::Warn,
                "One or more startup certificate checks failed. Periodic renewal task will NOT be activated.",
            );
        }
    });
}

/// Spawns the background task that periodically checks certificate status.
fn spawn_periodic_renewal_task(app_state: AppState) {
    tokio::spawn(async move {
        log(
            LogLevel::Info,
            &format!(
                "Certificate renewal task scheduled to run every {:?}",
                app_state.config.update_interval
            ),
        );
        let mut interval = time::interval(app_state.config.update_interval);
        interval.tick().await; // Wait for the first interval

        loop {
            interval.tick().await;
            log(
                LogLevel::Info,
                "Running scheduled certificate renewal check...",
            );
            // TODO: Implement the logic to check SSL certificate expiry and renew if necessary.
        }
    });
}
