/* src/state.rs */

use crate::config::AppConfig;
use parking_lot::RwLock;
use std::collections::HashMap;
use std::sync::Arc;

#[derive(Clone, Debug)]
pub enum DomainStatus {
    Acquiring,
    Ready,
    Failed(String),
}

#[derive(Clone)]
pub struct AppState {
    pub config: Arc<AppConfig>,
    pub task_running: Arc<RwLock<bool>>,
    pub domains: Arc<RwLock<HashMap<String, DomainStatus>>>,
    pub is_acquiring: Arc<RwLock<bool>>,
}

impl AppState {
    pub fn new(config: AppConfig) -> Self {
        Self {
            config: Arc::new(config),
            task_running: Arc::new(RwLock::new(false)),
            domains: Arc::new(RwLock::new(HashMap::new())),
            is_acquiring: Arc::new(RwLock::new(false)),
        }
    }
}
