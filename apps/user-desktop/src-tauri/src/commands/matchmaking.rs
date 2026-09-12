/*
 * SPDX-License-Identifier: AGPL-3.0-only
 * Copyright (C) 2026 baibai and Botting contributors
 *
 * Botting is free software: you can redistribute it and/or modify it under
 * the GNU Affero General Public License version 3, as published by the
 * Free Software Foundation. This program comes WITHOUT ANY WARRANTY;
 * without even the implied warranty of MERCHANTABILITY or FITNESS FOR A
 * PARTICULAR PURPOSE. See the LICENSE file for the complete terms.
 * Copyleft: covered modifications must retain these license obligations.
 * https://www.gnu.org/licenses/agpl-3.0.html
 */

//! 提供配對啟停與目前快照的 Tauri 指令。

use crate::{
    app_state::{AppState, CommandResult},
    matchmaking::{MatchmakingSnapshot, StartMatchmakingInput},
};
use tauri::State;

#[tauri::command]
/// 建立配對計畫並開始玩家日誌追蹤。
/// @param input 配對模式、Bot 選擇及日誌路徑。
/// @param state Tauri 管理的應用狀態。
/// @return 啟動後的配對快照。
pub(crate) async fn start_matchmaking(
    input: StartMatchmakingInput,
    state: State<'_, AppState>,
) -> CommandResult<MatchmakingSnapshot> {
    state.runtime().start_matchmaking(input).await
}

#[tauri::command]
/// 停止配對並撤回參與 Bot。
/// @param state Tauri 管理的應用狀態。
/// @return 停止後的配對快照。
pub(crate) async fn stop_matchmaking(
    state: State<'_, AppState>,
) -> CommandResult<MatchmakingSnapshot> {
    Ok(state.runtime().stop_matchmaking().await)
}

#[tauri::command]
/// 查詢配對狀態，不變更執行模式。
/// @param state Tauri 管理的應用狀態。
/// @return 目前配對快照。
pub(crate) async fn get_matchmaking_snapshot(
    state: State<'_, AppState>,
) -> CommandResult<MatchmakingSnapshot> {
    Ok(state.runtime().matchmaking_snapshot().await)
}
