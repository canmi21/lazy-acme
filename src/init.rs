/* src/init.rs */

use crate::config::AppConfig;
use fancy_log::{LogLevel, log};
use std::path::Path;
use tokio::fs;

const DEFAULT_CONFIG_TOML: &str = r#"
# This file maps your domains to a DNS provider configuration.
# The `dns_provider` value should correspond to a `[provider_name].dns.toml` file.

# [[domains]]
# name = "example.com"
# dns_provider = "cloudflare"

# [[domains]]
# name = "another.dev"
# dns_provider = "cloudflare_zerossl"
"#;

const DEFAULT_CLOUDFLARE_DNS_TOML: &str = r#"
# Configuration for a DNS provider, e.g., 'cloudflare'.
# You can copy this file to create configs for different CAs,
# e.g., 'cloudflare_zerossl.dns.toml'.

# The command to run when initially acquiring a certificate.
cmd = "CLOUDFLARE_DNS_API_TOKEN={{API_KEY}} lego --email {{EMAIL}} --server {{CA}} --dns cloudflare -d '*.{{DOMAIN}}' -d {{DOMAIN}} run"

# The command to run when renewing a certificate.
renew = "CLOUDFLARE_DNS_API_TOKEN={{API_KEY}} lego --email {{EMAIL}} --server {{CA}} --dns cloudflare -d '*.{{DOMAIN}}' -d {{DOMAIN}} renew --days 30"

# --- Your Credentials ---
api_key = "YOUR_CLOUDFLARE_API_TOKEN_HERE"
email = "your-email@example.com"

# --- Certificate Authority (CA) ---
# Let's Encrypt (Production): https://acme-v02.api.letsencrypt.org/directory
# Let's Encrypt (Staging): https://acme-staging-v02.api.letsencrypt.org/directory
# ZeroSSL (requires EAB): https://acme.zerossl.com/v2/DV90
# Buypass Go SSL: https://api.buypass.com/acme/directory
ca = "https://acme-v02.api.letsencrypt.org/directory"
"#;

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
                "Creating default DNS provider example: {:?}",
                cf_dns_toml_path
            ),
        );
        fs::write(&cf_dns_toml_path, DEFAULT_CLOUDFLARE_DNS_TOML).await?;
        is_first_run = true;
    }

    Ok(is_first_run)
}

async fn path_exists(path: &Path) -> bool {
    fs::metadata(path).await.is_ok()
}
