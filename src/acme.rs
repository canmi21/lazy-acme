/* src/acme.rs */

use crate::{
    config::{self, AppConfig, add_domain_to_config},
    state::{AppState, DomainStatus},
};
use chrono::{DateTime, Utc};
use fancy_log::{LogLevel, log};
use fancy_regex::Regex;
use std::path::{Path, PathBuf};
use tokio::{
    fs,
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    process::Command,
};
use x509_parser::prelude::*;

pub async fn certificate_exists(domain: &str, config: &AppConfig) -> bool {
    let domain_name = domain.trim();
    let cert_dir = config.dir_path.join(".lego/certificates");
    let key_path_exact = cert_dir.join(format!("{}.key", domain_name));
    let key_path_wildcard = cert_dir.join(format!("_.{}.key", domain_name));
    if tokio::fs::metadata(key_path_wildcard).await.is_ok() {
        return true;
    }
    if tokio::fs::metadata(key_path_exact).await.is_ok() {
        return true;
    }
    false
}

#[derive(Clone, Copy)]
pub enum CommandType {
    Run,
    Renew,
}

pub async fn acquire_or_renew_certificate(
    app_state: AppState,
    domain: String,
    dns_provider: String,
    persist: bool,
    command_type: CommandType,
) {
    let config = app_state.config.clone();
    let domain_name = domain.trim();

    app_state
        .domains
        .write()
        .insert(domain_name.to_string(), DomainStatus::Acquiring);

    let result = do_execute_lego(domain_name, &dns_provider, &config, command_type).await;

    match result {
        Ok(_) => {
            let success_msg = match command_type {
                CommandType::Run => "Successfully acquired certificate for",
                CommandType::Renew => "Successfully renewed certificate for",
            };
            log(
                LogLevel::Info,
                &format!("{} '{}'", success_msg, domain_name),
            );
            app_state
                .domains
                .write()
                .insert(domain_name.to_string(), DomainStatus::Ready);
            if persist {
                let config_path = config.dir_path.join("config.toml");
                if let Err(e) = add_domain_to_config(&config_path, domain_name, &dns_provider).await
                {
                    log(
                        LogLevel::Error,
                        &format!("Failed to persist domain to config.toml: {}", e),
                    );
                }
            }
        }
        Err(e) => {
            let err_msg = e.to_string();
            log(
                LogLevel::Error,
                &format!(
                    "Failed to acquire/renew certificate for '{}': {}",
                    domain_name, err_msg
                ),
            );
            app_state
                .domains
                .write()
                .insert(domain_name.to_string(), DomainStatus::Failed(err_msg));
        }
    }

    *app_state.is_acquiring.write() = false;
    log(LogLevel::Debug, "Global acquisition lock released.");
}

fn sanitize_command_for_log(command: &str) -> String {
    let re = Regex::new(r#"(?i)([^=\s]+)=(['"]?)[^'"\s]+\2(?=\s+lego)"#).unwrap();
    re.replace_all(command, "$1=***").to_string()
}

async fn do_execute_lego(
    domain: &str,
    dns_provider: &str,
    config: &AppConfig,
    command_type: CommandType,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let provider_config_path = config
        .dir_path
        .join(format!("{}.dns.toml", dns_provider.trim()));

    if !provider_config_path.exists() {
        return Err(format!(
            "DNS provider config not found at {:?}",
            provider_config_path
        )
        .into());
    }

    let provider_config = config::load_dns_provider_config(&provider_config_path).await?;
    let placeholder_re = ::regex::Regex::new(r"\{\{([a-zA-Z0-9_]+)\}\}")?;

    let command_template = match command_type {
        CommandType::Run => provider_config.cmd.clone(),
        CommandType::Renew => provider_config
            .renew
            .clone()
            .unwrap_or_else(|| provider_config.cmd.clone()),
    };

    let mut final_cmd = command_template;
    for cap in placeholder_re.captures_iter(&final_cmd.clone()) {
        let placeholder = &cap[0];
        let key = &cap[1];
        let value = if key.eq_ignore_ascii_case("DOMAIN") {
            domain.to_string()
        } else {
            provider_config
                .vars
                .iter()
                .find(|(k, _)| k.eq_ignore_ascii_case(key))
                .and_then(|(_, v)| v.as_str().map(ToString::to_string))
                .unwrap_or_default()
        };
        final_cmd = final_cmd.replace(placeholder, &value);
    }

    let sanitized_cmd = sanitize_command_for_log(&final_cmd);
    log(
        LogLevel::Debug,
        &format!("Executing command: {}", sanitized_cmd),
    );

    execute_lego_command(&final_cmd, &config.dir_path).await
}

async fn execute_lego_command(
    command: &str,
    working_dir: &Path,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let mut cmd = Command::new("sh");
    cmd.arg("-c")
        .arg(command)
        .current_dir(working_dir)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());

    let mut child = cmd.spawn()?;
    let mut stdin = child.stdin.take().expect("Failed to open stdin");
    let stdout = child.stdout.take().expect("Failed to open stdout");
    let stderr = child.stderr.take().expect("Failed to open stderr");
    let mut stdout_reader = BufReader::new(stdout).lines();
    let mut stderr_reader = BufReader::new(stderr).lines();

    loop {
        tokio::select! {
            result = stdout_reader.next_line() => {
                match result {
                    Ok(Some(line)) => {
                        log(LogLevel::Info, &line);
                        if line.contains("Do you accept the TOS? Y/n") {
                            log(LogLevel::Warn, "TOS prompt detected. Responding with 'y'.");
                            stdin.write_all(b"y\n").await?;
                        }
                    },
                    Ok(None) => break,
                    Err(e) => log(LogLevel::Error, &e.to_string()),
                }
            },
            result = stderr_reader.next_line() => {
                  match result {
                    Ok(Some(line)) => log(LogLevel::Error, &line),
                    Ok(None) => {},
                    Err(e) => log(LogLevel::Error, &e.to_string()),
                }
            },
            status = child.wait() => {
                let exit_status = status?;
                if exit_status.success() {
                    log(LogLevel::Info, "Lego command finished successfully.");
                } else {
                    let err_msg = format!("Lego command failed with status: {}", exit_status);
                    log(LogLevel::Error, &err_msg);
                    return Err(err_msg.into());
                }
            }
        }
    }
    Ok(())
}

pub async fn needs_renewal(
    domain: &str,
    config: &AppConfig,
    days_before_expiry: i64,
) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
    let domain_name = domain.trim();
    let cert_dir = config.dir_path.join(".lego/certificates");

    let cert_path = find_cert_file(domain_name, &cert_dir)
        .await
        .ok_or("Certificate file not found for renewal check.")?;

    let cert_data = fs::read(&cert_path).await?;
    let pem = ::pem::parse(&cert_data)?;
    let (_, x509_cert) = X509Certificate::from_der(pem.contents())?;

    let not_after_str = x509_cert
        .validity()
        .not_after
        .to_rfc2822()
        .map_err(|e| e.to_string())?;
    let expiry_date = DateTime::parse_from_rfc2822(&not_after_str)?.with_timezone(&Utc);

    let now = Utc::now();
    let threshold = chrono::Duration::days(days_before_expiry);

    let needs_renew = expiry_date - now < threshold;
    if needs_renew {
        log(
            LogLevel::Warn,
            &format!(
                "Certificate for '{}' expires on {} (in less than {} days). Renewal required.",
                domain, expiry_date, days_before_expiry
            ),
        );
    } else {
        log(
            LogLevel::Info,
            &format!(
                "Certificate for '{}' is valid until {}. No renewal needed.",
                domain, expiry_date
            ),
        );
    }

    Ok(needs_renew)
}

async fn find_cert_file(domain: &str, cert_dir: &Path) -> Option<PathBuf> {
    let wildcard_path = cert_dir.join(format!("_.{}.crt", domain));
    if tokio::fs::metadata(&wildcard_path).await.is_ok() {
        return Some(wildcard_path);
    }
    let exact_path = cert_dir.join(format!("{}.crt", domain));
    if tokio::fs::metadata(&exact_path).await.is_ok() {
        return Some(exact_path);
    }
    None
}
