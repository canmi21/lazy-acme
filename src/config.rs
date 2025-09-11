/* src/config.rs */

use fancy_log::{LogLevel, log};
use serde::Deserialize;
use std::env;
use std::path::{Path, PathBuf};
use std::time::Duration;
use tokio::fs;
use toml_edit::{DocumentMut, Table, value};

#[derive(Debug, Clone)]
pub struct AppConfig {
    pub log_level: LogLevel,
    pub update_interval: Duration,
    pub dir_path: PathBuf,
    pub bind_port: u16,
}

impl AppConfig {
    pub fn load() -> Self {
        dotenvy::dotenv().ok();
        let log_level_str = env::var("LOG_LEVEL").unwrap_or_else(|_| "info".to_string());
        let log_level = match log_level_str.to_lowercase().as_str() {
            "debug" => LogLevel::Debug,
            "warn" => LogLevel::Warn,
            "error" => LogLevel::Error,
            _ => LogLevel::Info,
        };
        let update_hours = env::var("UPDATE_INTERVAL_HOURS")
            .unwrap_or_else(|_| "24".to_string())
            .parse::<u64>()
            .unwrap_or(24);
        let update_interval = Duration::from_secs(update_hours * 3600);
        let dir_path_str = env::var("DIR_PATH").unwrap_or_else(|_| "~/lazy-acme".to_string());
        let dir_path = PathBuf::from(shellexpand::tilde(&dir_path_str).into_owned());
        let bind_port = env::var("BIND_PORT")
            .unwrap_or_else(|_| "33301".to_string())
            .parse::<u16>()
            .unwrap_or(33301);
        Self {
            log_level,
            update_interval,
            dir_path,
            bind_port,
        }
    }
}

#[derive(Deserialize, Debug, Clone)]
pub struct DomainEntry {
    pub name: String,
    pub dns_provider: String,
}

#[derive(Deserialize, Debug)]
pub struct DomainConfig {
    #[serde(default, rename = "domains")]
    pub domains: Vec<DomainEntry>,
}

#[derive(Deserialize, Debug)]
pub struct DnsProviderConfig {
    pub cmd: String,
    pub renew: Option<String>,
    #[serde(flatten)]
    pub vars: toml::map::Map<String, toml::Value>,
}

pub async fn load_domain_config(
    path: &Path,
) -> Result<DomainConfig, Box<dyn std::error::Error + Send + Sync>> {
    let content = fs::read_to_string(path).await?;
    Ok(toml::from_str(&content)?)
}

pub async fn load_dns_provider_config(
    path: &Path,
) -> Result<DnsProviderConfig, Box<dyn std::error::Error + Send + Sync>> {
    let content = fs::read_to_string(path).await?;
    Ok(toml::from_str(&content)?)
}

pub async fn add_domain_to_config(
    config_path: &Path,
    domain: &str,
    dns_provider: &str,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    log(
        LogLevel::Info,
        &format!("Persisting new domain '{}' to config.toml", domain),
    );
    let content = fs::read_to_string(config_path).await?;
    let mut doc = content.parse::<DocumentMut>()?;

    let domains_array = doc["domains"]
        .as_array_of_tables_mut()
        .ok_or("config.toml is missing 'domains' array of tables")?;

    let mut new_domain_table = Table::new();
    new_domain_table["name"] = value(domain);
    new_domain_table["dns_provider"] = value(dns_provider);

    domains_array.push(new_domain_table);

    fs::write(config_path, doc.to_string()).await?;
    Ok(())
}
