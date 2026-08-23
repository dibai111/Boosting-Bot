use crate::app_state::{AppState, CommandResult};
use tauri::State;

#[tauri::command]
pub(crate) async fn start_bot(id: String, state: State<'_, AppState>) -> CommandResult<()> {
    state.runtime().start_bot(id).await
}

#[tauri::command]
pub(crate) async fn stop_bot(id: String, state: State<'_, AppState>) -> CommandResult<()> {
    state.runtime().stop_bot(id).await
}
