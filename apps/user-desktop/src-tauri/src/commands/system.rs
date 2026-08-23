use crate::app_state::{AppState, CommandResult};
use tauri::State;

#[tauri::command]
pub(crate) async fn app_memory_bytes(state: State<'_, AppState>) -> CommandResult<u64> {
    state.runtime().app_memory_bytes().await
}
