/* src/state.rs */

use crate::config::AppConfig;
use parking_lot::RwLock;
use std::collections::HashMap;
use std::sync::Arc;

/// Represents the current status of a domain's certificate.
#[derive(Clone, Debug)]
pub enum DomainStatus {
    Acquiring,      // Certificate acquisition is in progress.
    Ready,          // Certificate is available.
    Failed(String), // Acquisition failed with an error message.
}

/// The global, thread-safe state for the entire application.
#[derive(Clone)]
pub struct AppState {
    // The loaded application config, shared across all tasks.
    pub config: Arc<AppConfig>,
    // Tracks whether the periodic renewal task is active.
    pub task_running: Arc<RwLock<bool>>,
    // Tracks the status of each managed domain.
    pub domains: Arc<RwLock<HashMap<String, DomainStatus>>>,
}

impl AppState {
    pub fn new(config: AppConfig) -> Self {
        Self {
            config: Arc::new(config),
            task_running: Arc::new(RwLock::new(false)),
            domains: Arc::new(RwLock::new(HashMap::new())),
        }
    }
}
