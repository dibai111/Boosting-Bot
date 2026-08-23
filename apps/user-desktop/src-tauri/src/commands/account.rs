use crate::app_state::{AppState, CommandResult};
use local_store::{AccountRecord, CreateAccountInput};
use tauri::State;

#[tauri::command]
pub(crate) async fn list_accounts(state: State<'_, AppState>) -> CommandResult<Vec<AccountRecord>> {
    state.runtime().list_accounts().await
}

#[tauri::command]
pub(crate) async fn add_account(
    input: CreateAccountInput,
    state: State<'_, AppState>,
) -> CommandResult<AccountRecord> {
    state.runtime().add_account(input).await
}

#[tauri::command]
pub(crate) async fn delete_accounts(
    ids: Vec<String>,
    state: State<'_, AppState>,
) -> CommandResult<usize> {
    state.runtime().delete_accounts(ids).await
}

#[tauri::command]
pub(crate) async fn update_server_address(
    id: String,
    server_address: String,
    state: State<'_, AppState>,
) -> CommandResult<()> {
    state
        .runtime()
        .update_server_address(id, server_address)
        .await
}
