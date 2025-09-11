/* src/init.rs */

use crate::config::AppConfig;
use fancy_log::{LogLevel, log};
use std::path::Path;
use tokio::fs;

const DEFAULT_CONFIG_TOML: &str = r#"
# This file maps your domains to a DNS provider configuration.
# Example:
# [[domains]]
# name = "example.com"
# dns_provider = "cloudflare"
"#;

const DEFAULT_CLOUDFLARE_DNS_TOML: &str = r#"
# Configuration for the 'cloudflare' DNS provider.
# You can get your API token from the Cloudflare dashboard.
# https://dash.cloudflare.com/profile/api-tokens

# The command that will be executed by lazy-acme.
# Placeholders like {{API_KEY}}, {{EMAIL}}, and {{DOMAIN}} will be replaced.
cmd = "CLOUDFLARE_DNS_API_TOKEN={{API_KEY}} lego --email {{EMAIL}} --dns cloudflare -d '*.{{DOMAIN}}' -d {{DOMAIN}} run"

# --- Your Credentials ---
api_key = "YOUR_CLOUDFLARE_API_TOKEN"
email = "your-email@example.com"
"#;

/// Checks for necessary directories and config files, creating them if they don't exist.
/// Returns `Ok(true)` if it's a first-time setup, `Ok(false)` otherwise.
pub async fn initialize_app(config: &AppConfig) -> Result<bool, std::io::Error> {
    let mut is_first_run = false;

    fs::create_dir_all(&config.dir_path).await?;
    log(
        LogLevel::Debug,
        &format!("Data directory is at: {:?}", config.dir_path),
    );

    let lego_path = config.dir_path.join(".lego");
    fs::create_dir_all(&lego_path).await?;

    let config_toml_path = config.dir_path.join("config.toml");
    if !path_exists(&config_toml_path).await {
        log(
            LogLevel::Warn,
            &format!("Creating default config: {:?}", config_toml_path),
        );
        fs::write(&config_toml_path, DEFAULT_CONFIG_TOML).await?;
        is_first_run = true;
    }

    let cf_dns_toml_path = config.dir_path.join("cloudflare.dns.toml");
    if !path_exists(&cf_dns_toml_path).await {
        log(
            LogLevel::Warn,
            &format!(
                "Creating default DNS provider config: {:?}",
                cf_dns_toml_path
            ),
        );
        fs::write(&cf_dns_toml_path, DEFAULT_CLOUDFLARE_DNS_TOML).await?;
        is_first_run = true;
    }

    Ok(is_first_run)
}

/// Asynchronously check if a path exists.
async fn path_exists(path: &Path) -> bool {
    fs::metadata(path).await.is_ok()
}
