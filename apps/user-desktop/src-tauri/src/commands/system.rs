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

//! 提供應用程式及其子程序的記憶體統計指令。

use crate::app_state::{AppState, CommandResult};
use tauri::State;

#[tauri::command]
/// 查詢應用及子程序的專用工作集。
/// @param state Tauri 管理的應用狀態。
/// @return 位元組數或平台讀取錯誤。
pub(crate) async fn app_memory_bytes(state: State<'_, AppState>) -> CommandResult<u64> {
    state.runtime().app_memory_bytes().await
}
