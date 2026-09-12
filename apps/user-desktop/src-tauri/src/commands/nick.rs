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

//! 提供 Nick 篩選啟停、候選決定及目前執行模式的指令。

use crate::{
    app_state::{AppState, CommandResult},
    bot_runtime::RuntimeMode,
    runtime::StartNickRollerInput,
};
use tauri::State;

#[tauri::command]
/// 驗證目標並啟動 Nick 篩選。
/// @param input Bot ID 與篩選規則。
/// @param state Tauri 管理的應用狀態。
/// @return 執行模式或啟動錯誤。
pub(crate) async fn start_nick_roller(
    input: StartNickRollerInput,
    state: State<'_, AppState>,
) -> CommandResult<RuntimeMode> {
    state.runtime().start_nick_roller(input).await
}

#[tauri::command]
/// 停止本次追蹤的 Nick 篩選。
/// @param state Tauri 管理的應用狀態。
/// @return 停止請求結果。
pub(crate) async fn stop_nick_roller(state: State<'_, AppState>) -> CommandResult<()> {
    state.runtime().stop_nick_roller().await
}

#[tauri::command]
/// 將目前候選的接受或跳過決策交回狀態機。
/// @param bot_id 候選所屬 Bot ID。
/// @param candidate_id 候選事件中的編號。
/// @param take 是否接受候選。
/// @param state Tauri 管理的應用狀態。
/// @return 指令發送結果；舊候選由後端忽略。
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
/// 對照實際工作狀態後回傳執行模式。
/// @param state Tauri 管理的應用狀態。
/// @return 目前 RuntimeMode。
pub(crate) async fn get_active_mode(state: State<'_, AppState>) -> CommandResult<RuntimeMode> {
    Ok(state.runtime().active_mode().await)
}
