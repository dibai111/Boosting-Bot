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

//! 提供單一 Bot 的啟動與停止指令。

use crate::app_state::{AppState, CommandResult};
use tauri::State;

#[tauri::command]
/// 啟動已保存帳號的 Bot。
/// @param id 本機帳號 ID。
/// @param state Tauri 管理的應用狀態。
/// @return 啟動請求結果；連線進度由事件回報。
pub(crate) async fn start_bot(id: String, state: State<'_, AppState>) -> CommandResult<()> {
    state.runtime().start_bot(id).await
}

#[tauri::command]
/// 停止指定 Bot 工作階段。
/// @param id 本機帳號 ID。
/// @param state Tauri 管理的應用狀態。
/// @return 停止結果。
pub(crate) async fn stop_bot(id: String, state: State<'_, AppState>) -> CommandResult<()> {
    state.runtime().stop_bot(id).await
}
