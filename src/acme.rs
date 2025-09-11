/* src/acme.rs */

use crate::{
    config::{self, AppConfig, add_domain_to_config},
    state::{AppState, DomainStatus},
};
use fancy_log::{LogLevel, log};
// FIX: Use `fancy_regex` instead of `regex`
use fancy_regex::Regex;
use std::path::Path;
use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::Command;

pub async fn certificate_exists(domain: &str, config: &AppConfig) -> bool {
    let cert_key_path = config
        .dir_path
        .join(".lego/certificates")
        .join(format!("_.{}.key", domain.trim()));
    tokio::fs::metadata(cert_key_path).await.is_ok()
}
pub async fn acquire_certificate(
    app_state: AppState,
    domain: String,
    dns_provider: String,
    persist: bool,
) {
    let config = app_state.config.clone();
    let domain_name = domain.trim();
    app_state
        .domains
        .write()
        .insert(domain_name.to_string(), DomainStatus::Acquiring);
    let result = do_acquire_certificate(domain_name, &dns_provider, &config).await;
    match result {
        Ok(_) => {
            log(
                LogLevel::Info,
                &format!("Successfully acquired certificate for '{}'", domain_name),
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
                    "Failed to acquire certificate for '{}': {}",
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

/// A helper function to sanitize the command for logging.
/// It replaces values of assignments before "lego" with asterisks.
fn sanitize_command_for_log(command: &str) -> String {
    // FIX: Revert to the powerful regex that uses backreferences and look-aheads,
    // as it is now supported by the `fancy-regex` crate.
    let re = Regex::new(r#"(?i)([^=\s]+)=(['"]?)[^'"\s]+\2(?=\s+lego)"#).unwrap();
    re.replace_all(command, "$1=***").to_string()
}

/// Internal helper that contains the actual command-building and execution logic.
async fn do_acquire_certificate(
    domain: &str,
    dns_provider: &str,
    config: &AppConfig,
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
    // This Regex for placeholder replacement is simple and does not need fancy-regex
    let placeholder_re = ::regex::Regex::new(r"\{\{([a-zA-Z0-9_]+)\}\}")?;
    let mut final_cmd = provider_config.cmd.clone();

    for cap in placeholder_re.captures_iter(&provider_config.cmd) {
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
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
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
