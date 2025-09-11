/* src/tasks.rs */

use crate::{
    acme::{self, CommandType},
    config,
    state::{AppState, DomainStatus},
};
use fancy_log::{LogLevel, log};
use tokio::time;

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
                return;
            }
        };

        for domain in &domain_config.domains {
            let domain_name = domain.name.trim().to_string();
            if acme::certificate_exists(&domain_name, &config).await {
                app_state
                    .domains
                    .write()
                    .insert(domain_name, DomainStatus::Ready);
            }
        }

        let mut all_successful = true;
        let domains_to_check = domain_config.domains.clone();
        for domain in domains_to_check {
            let domain_name = domain.name.trim();
            if !acme::certificate_exists(domain_name, &config).await {
                acme::acquire_or_renew_certificate(
                    app_state.clone(),
                    domain.name.clone(),
                    domain.dns_provider.clone(),
                    false,
                    CommandType::Run,
                )
                .await;
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
        interval.tick().await;

        loop {
            interval.tick().await;
            log(
                LogLevel::Info,
                "Running scheduled certificate renewal check...",
            );

            let config_path = app_state.config.dir_path.join("config.toml");
            let domain_config = match config::load_domain_config(&config_path).await {
                Ok(c) => c,
                Err(e) => {
                    log(
                        LogLevel::Error,
                        &format!("Renewal task: Failed to load config.toml: {}", e),
                    );
                    continue;
                }
            };

            for domain_entry in domain_config.domains {
                let domain_name = domain_entry.name.trim();

                if *app_state.is_acquiring.read() {
                    log(
                        LogLevel::Warn,
                        "Another task is already running, postponing renewal check cycle.",
                    );
                    break;
                }

                match acme::needs_renewal(domain_name, &app_state.config, 30).await {
                    Ok(true) => {
                        log(
                            LogLevel::Warn,
                            &format!("Proceeding with renewal for '{}'...", domain_name),
                        );

                        *app_state.is_acquiring.write() = true;

                        acme::acquire_or_renew_certificate(
                            app_state.clone(),
                            domain_name.to_string(),
                            domain_entry.dns_provider,
                            false,
                            CommandType::Renew,
                        )
                        .await;
                    }
                    Ok(false) => {}
                    Err(e) => {
                        log(
                            LogLevel::Error,
                            &format!("Error checking renewal status for '{}': {}", domain_name, e),
                        );
                    }
                }
            }
        }
    });
}
