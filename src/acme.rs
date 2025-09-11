/* src/acme.rs */

use crate::config::{self, AppConfig, DomainEntry};
use fancy_log::{LogLevel, log};
use regex::Regex;
use std::path::Path;
use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::Command;

/// The main entry point for the startup certificate check.
pub async fn check_and_acquire_certs_on_startup(
    config: &AppConfig,
) -> Result<(), Box<dyn std::error::Error>> {
    let domain_config_path = config.dir_path.join("config.toml");
    let domain_config = config::load_domain_config(&domain_config_path).await?;

    if domain_config.domains.is_empty() {
        log(
            LogLevel::Info,
            "No domains configured in config.toml, skipping certificate check.",
        );
        return Ok(());
    }

    log(
        LogLevel::Info,
        &format!("Found {} domain(s) to check.", domain_config.domains.len()),
    );

    for domain_entry in &domain_config.domains {
        let domain_name = domain_entry.name.trim();

        if certificate_exists_for_domain(domain_name, config).await {
            log(
                LogLevel::Info,
                &format!("Certificate found for '{}'.", domain_name),
            );
        } else {
            log(
                LogLevel::Warn,
                &format!(
                    "Certificate NOT found for '{}'. Attempting to acquire...",
                    domain_name
                ),
            );
            acquire_certificate(domain_entry, config).await?;
        }
    }

    Ok(())
}

/// Checks if a certificate file exists for the given domain.
async fn certificate_exists_for_domain(domain: &str, config: &AppConfig) -> bool {
    // --- THE FINAL FIX ---
    // The format string must be "_.{domain}.key" to match lego's output.
    // My previous version was missing the dot after the underscore.
    let cert_key_path = config
        .dir_path
        .join(".lego/certificates")
        .join(format!("_.{}.key", domain)); // <-- CORRECTED LINE

    let metadata_result = tokio::fs::metadata(&cert_key_path).await;
    log(
        LogLevel::Debug,
        &format!(
            "Metadata check for {:?}: {:?}",
            cert_key_path, metadata_result
        ),
    );

    metadata_result.is_ok()
}

/// Handles the full process of acquiring a certificate for a single domain.
async fn acquire_certificate(
    domain: &DomainEntry,
    config: &AppConfig,
) -> Result<(), Box<dyn std::error::Error>> {
    let trimmed_domain_name = domain.name.trim();

    let provider_config_path = config
        .dir_path
        .join(format!("{}.dns.toml", domain.dns_provider.trim()));

    if !provider_config_path.exists() {
        let err_msg = format!(
            "DNS provider config not found for '{}' at {:?}.",
            domain.dns_provider, provider_config_path
        );
        log(LogLevel::Error, &err_msg);
        return Err(err_msg.into());
    }

    let provider_config = config::load_dns_provider_config(&provider_config_path).await?;

    let re = Regex::new(r"\{\{([a-zA-Z0-9_]+)\}\}")?;
    let mut final_cmd = provider_config.cmd.clone();

    for cap in re.captures_iter(&provider_config.cmd) {
        let placeholder = &cap[0];
        let key_from_placeholder = &cap[1];

        let value = if key_from_placeholder.eq_ignore_ascii_case("DOMAIN") {
            trimmed_domain_name.to_string()
        } else {
            provider_config
                .vars
                .iter()
                .find(|(toml_key, _)| toml_key.eq_ignore_ascii_case(key_from_placeholder))
                .and_then(|(_, toml_value)| toml_value.as_str().map(ToString::to_string))
                .unwrap_or_else(|| "".to_string())
        };

        final_cmd = final_cmd.replace(placeholder, &value);
    }

    log(
        LogLevel::Debug,
        &format!("Executing command: {}", final_cmd),
    );
    execute_lego_command(&final_cmd, &config.dir_path).await
}

/// Executes the lego command, handles interactive prompts, and streams output.
async fn execute_lego_command(
    command: &str,
    working_dir: &Path,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut cmd = Command::new("sh");
    cmd.arg("-c").arg(command);
    cmd.current_dir(working_dir);

    cmd.stdin(Stdio::piped());
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());

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
                    log(LogLevel::Error, &format!("Lego command failed with status: {}", exit_status));
                    return Err(format!("Lego process exited with non-zero status: {}", exit_status).into());
                }
            }
        }
    }

    Ok(())
}
