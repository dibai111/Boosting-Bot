use crate::{
    app_state::{AppState, CommandResult},
    matchmaking::{MatchmakingSnapshot, StartMatchmakingInput},
};
use tauri::State;

#[tauri::command]
pub(crate) async fn start_matchmaking(
    input: StartMatchmakingInput,
    state: State<'_, AppState>,
) -> CommandResult<MatchmakingSnapshot> {
    state.runtime().start_matchmaking(input).await
}

#[tauri::command]
pub(crate) async fn stop_matchmaking(
    state: State<'_, AppState>,
) -> CommandResult<MatchmakingSnapshot> {
    Ok(state.runtime().stop_matchmaking().await)
}

#[tauri::command]
pub(crate) async fn get_matchmaking_snapshot(
    state: State<'_, AppState>,
) -> CommandResult<MatchmakingSnapshot> {
    Ok(state.runtime().matchmaking_snapshot().await)
}
