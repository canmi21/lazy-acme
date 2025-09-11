/* src/handlers.rs */

use crate::{
    acme, response,
    state::{AppState, DomainStatus},
};
use axum::{
    Json,
    extract::{Path, Query, State}, // Query is new
    http::StatusCode,
    response::{IntoResponse, Response},
};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use fancy_log::{LogLevel, log};
use serde::Deserialize;
use serde_json::json;
use std::path::PathBuf;
use tokio::fs;

/// GET /v1/task - Returns the health status of the background renewal task.
pub async fn get_task_status(State(state): State<AppState>) -> Response {
    let is_running = *state.task_running.read();
    response::success(Some(json!({ "running": is_running })))
}

// NEW: Define a struct for query parameters
#[derive(Deserialize)]
pub struct CertQuery {
    #[serde(default)]
    wildcard: bool,
}

/// GET /v1/certificate/{domain} - Returns certificate status or content.
pub async fn get_certificate(
    State(state): State<AppState>,
    Path(domain): Path<String>,
    Query(query): Query<CertQuery>, // NEW: Extract query parameters
) -> Response {
    let domain_status = state.domains.read().get(domain.trim()).cloned();

    match domain_status {
        Some(DomainStatus::Ready) => {
            let cert_dir = state.config.dir_path.join(".lego/certificates");
            let domain_name = domain.trim();

            let paths_to_try: Vec<PathBuf> = if query.wildcard {
                // If wildcard=true, only try the wildcard path
                vec![cert_dir.join(format!("_.{}.crt", domain_name))]
            } else {
                // Otherwise, try wildcard first, then exact match
                vec![
                    cert_dir.join(format!("_.{}.crt", domain_name)),
                    cert_dir.join(format!("{}.crt", domain_name)),
                ]
            };

            // Iterate through the paths and try to read the first one that exists
            for path in paths_to_try {
                if let Ok(content_bytes) = fs::read(&path).await {
                    let encoded_cert = STANDARD.encode(&content_bytes);
                    return response::success(Some(json!({ "certificate_base64": encoded_cert })));
                }
            }

            // If no path was readable, return an error
            response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "Certificate file is missing despite being marked as ready.",
            )
        }
        Some(DomainStatus::Acquiring) => (
            StatusCode::ACCEPTED,
            Json(
                json!({"status": "Accepted", "message": "Certificate acquisition is in progress."}),
            ),
        )
            .into_response(),
        Some(DomainStatus::Failed(reason)) => response::error(
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Certificate acquisition failed: {}", reason),
        ),
        None => response::error(
            StatusCode::NOT_FOUND,
            "Certificate for this domain is not managed or found.",
        ),
    }
}

/// GET /v1/certificate/{domain}/key - Returns the private key.
pub async fn get_certificate_key(
    State(state): State<AppState>,
    Path(domain): Path<String>,
    Query(query): Query<CertQuery>, // NEW: Extract query parameters
) -> Response {
    if !matches!(
        state.domains.read().get(domain.trim()),
        Some(DomainStatus::Ready)
    ) {
        return response::error(
            StatusCode::NOT_FOUND,
            "Certificate is not ready or does not exist.",
        );
    }

    let cert_dir = state.config.dir_path.join(".lego/certificates");
    let domain_name = domain.trim();

    let paths_to_try: Vec<PathBuf> = if query.wildcard {
        vec![cert_dir.join(format!("_.{}.key", domain_name))]
    } else {
        vec![
            cert_dir.join(format!("_.{}.key", domain_name)),
            cert_dir.join(format!("{}.key", domain_name)),
        ]
    };

    for path in paths_to_try {
        if let Ok(content_bytes) = fs::read(&path).await {
            let encoded_key = STANDARD.encode(&content_bytes);
            return response::success(Some(json!({ "key_base64": encoded_key })));
        }
    }

    response::error(StatusCode::INTERNAL_SERVER_ERROR, "Key file is missing.")
}

#[derive(Deserialize)]
pub struct CreateCertRequest {
    pub domain: String,
    pub dns: String,
}

/// POST /v1/certificate - Requests a new certificate.
pub async fn create_certificate(
    State(state): State<AppState>,
    Json(payload): Json<CreateCertRequest>,
) -> Response {
    let domain = payload.domain.trim();
    let dns_provider = payload.dns.trim();

    // --- LOCKING LOGIC ---
    {
        let domains = state.domains.read();
        // Check 1: Is this specific domain already being processed or ready?
        if let Some(status) = domains.get(domain) {
            match status {
                DomainStatus::Acquiring => {
                    return response::error(
                        StatusCode::CONFLICT, // 409 Conflict is more appropriate here
                        "Certificate acquisition for this domain is already in progress.",
                    );
                }
                DomainStatus::Ready => {
                    return response::error(
                        StatusCode::BAD_REQUEST,
                        "Certificate for this domain already exists.",
                    );
                }
                DomainStatus::Failed(_) => {
                    // Allow retrying a failed domain, so we proceed
                }
            }
        }

        // Check 2: Is there ANY other acquisition process running globally?
        let mut is_acquiring_lock = state.is_acquiring.write();
        if *is_acquiring_lock {
            return response::error(
                StatusCode::SERVICE_UNAVAILABLE, // 503 is good for temporary unavailability
                "Another certificate acquisition is currently in progress. Please try again later.",
            );
        }
        // If not, acquire the lock
        *is_acquiring_lock = true;
        log(LogLevel::Debug, "Global acquisition lock acquired.");
    } // Release read/write locks before any .await calls

    let dns_config_path = state
        .config
        .dir_path
        .join(format!("{}.dns.toml", dns_provider));
    if !tokio::fs::metadata(dns_config_path).await.is_ok() {
        // Important: Release the lock if we fail early
        *state.is_acquiring.write() = false;
        log(
            LogLevel::Debug,
            "DNS config not found, releasing global acquisition lock.",
        );
        return response::error(
            StatusCode::BAD_REQUEST,
            "Specified DNS provider configuration not found.",
        );
    }

    // Spawn a background task to handle the actual acquisition
    tokio::spawn(acme::acquire_certificate(
        state.clone(),
        domain.to_string(),
        dns_provider.to_string(),
        true, // Persist on success
    ));

    (
        StatusCode::ACCEPTED,
        Json(json!({"status": "Accepted", "message": "Certificate acquisition process started."})),
    )
        .into_response()
}
