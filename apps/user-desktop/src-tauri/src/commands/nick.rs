use crate::{
    app_state::{AppState, CommandResult},
    bot_runtime::RuntimeMode,
    runtime::StartNickRollerInput,
};
use tauri::State;

#[tauri::command]
pub(crate) async fn start_nick_roller(
    input: StartNickRollerInput,
    state: State<'_, AppState>,
) -> CommandResult<RuntimeMode> {
    state.runtime().start_nick_roller(input).await
}

#[tauri::command]
pub(crate) async fn stop_nick_roller(state: State<'_, AppState>) -> CommandResult<()> {
    state.runtime().stop_nick_roller().await
}

#[tauri::command]
pub(crate) async fn answer_nick_decision(
    bot_id: String,
    candidate_id: u64,
    take: bool,
    state: State<'_, AppState>,
) -> CommandResult<()> {
    state
        .runtime()
        .answer_nick_decision(bot_id, candidate_id, take)
        .await
}

#[tauri::command]
pub(crate) async fn get_active_mode(state: State<'_, AppState>) -> CommandResult<RuntimeMode> {
    Ok(state.runtime().active_mode().await)
}
