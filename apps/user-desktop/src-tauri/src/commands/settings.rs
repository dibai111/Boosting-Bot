use crate::{app_state::AppState, app_state::CommandResult};
use local_store::UserSettings;
use tauri::State;

#[tauri::command]
pub(crate) async fn get_user_settings(state: State<'_, AppState>) -> CommandResult<UserSettings> {
    state.runtime().user_settings().await
}

#[tauri::command]
pub(crate) async fn save_user_settings(
    settings: UserSettings,
    state: State<'_, AppState>,
) -> CommandResult<()> {
    state.runtime().save_user_settings(settings).await
}
