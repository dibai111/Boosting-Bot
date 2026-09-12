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

//! 將帳號查詢、建立、刪除及地址更新指令轉交 runtime。

use crate::app_state::{AppState, CommandResult};
use local_store::{AccountRecord, CreateAccountInput};
use tauri::State;

#[tauri::command]
/// 查詢前端可見帳號清單。
/// @param state Tauri 管理的應用狀態。
/// @return 帳號快照；序列化排除憑據。
pub(crate) async fn list_accounts(state: State<'_, AppState>) -> CommandResult<Vec<AccountRecord>> {
    state.runtime().list_accounts().await
}

#[tauri::command]
/// 驗證並新增帳號，Microsoft 登入期間透過事件通知裝置代碼。
/// @param input 帳號登入輸入。
/// @param state Tauri 管理的應用狀態。
/// @return 保存後的帳號或登入／儲存錯誤。
pub(crate) async fn add_account(
    input: CreateAccountInput,
    state: State<'_, AppState>,
) -> CommandResult<AccountRecord> {
    state.runtime().add_account(input).await
}

#[tauri::command]
/// 停止指定 Bot 並刪除本機帳號。
/// @param ids 待刪除的帳號 ID。
/// @param state Tauri 管理的應用狀態。
/// @return 實際刪除數量。
pub(crate) async fn delete_accounts(
    ids: Vec<String>,
    state: State<'_, AppState>,
) -> CommandResult<usize> {
    state.runtime().delete_accounts(ids).await
}

#[tauri::command]
/// 驗證並更新帳號的目標伺服器。
/// @param id 本機帳號 ID。
/// @param server_address 新伺服器地址。
/// @param state Tauri 管理的應用狀態。
/// @return 更新結果。
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
