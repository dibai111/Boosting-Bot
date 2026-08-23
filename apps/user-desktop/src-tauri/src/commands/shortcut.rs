use crate::{app_state::CommandResult, platform::shortcuts::MatchmakingShortcuts};
use tauri::State;

#[tauri::command]
pub(crate) fn configure_matchmaking_shortcuts(
    shortcuts: State<'_, MatchmakingShortcuts>,
    stop_shortcut: String,
    show_overlay_shortcut: String,
) -> CommandResult<()> {
    shortcuts.configure(&stop_shortcut, &show_overlay_shortcut)
}
