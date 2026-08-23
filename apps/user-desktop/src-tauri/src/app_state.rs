use crate::runtime::UserRuntime;
use std::sync::Arc;

pub(crate) struct AppState {
    // Tauri state 只暴露應用能力，基礎設施由 runtime 內部管理。
    runtime: Arc<UserRuntime>,
}

pub(crate) type CommandResult<T> = Result<T, String>;

impl AppState {
    pub(crate) fn new(runtime: Arc<UserRuntime>) -> Self {
        Self { runtime }
    }

    pub(crate) fn runtime(&self) -> Arc<UserRuntime> {
        Arc::clone(&self.runtime)
    }
}
