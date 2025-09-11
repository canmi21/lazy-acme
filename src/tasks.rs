/* src/tasks.rs */

use crate::config::AppConfig;
use fancy_log::{LogLevel, log};
use tokio::time;

/// Spawns a background task that periodically checks certificate status.
pub fn spawn_cert_check_task(config: AppConfig) {
    tokio::spawn(async move {
        log(
            LogLevel::Info,
            &format!(
                "Certificate check task scheduled to run every {:?}",
                config.update_interval
            ),
        );
        let mut interval = time::interval(config.update_interval);

        // The first tick fires immediately, which we might not want.
        // Let's wait for the first interval to pass.
        interval.tick().await;

        loop {
            interval.tick().await;
            log(LogLevel::Info, "Running scheduled certificate check...");
            // TODO: Implement the logic to check SSL certificate status and renew if necessary.
        }
    });
}
