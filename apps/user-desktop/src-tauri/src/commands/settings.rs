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

//! 提供本機使用者設定的整批讀寫介面。

use crate::{app_state::AppState, app_state::CommandResult};
use local_store::UserSettings;
use tauri::State;

#[tauri::command]
/// 讀取本機設定完整快照。
/// @param state Tauri 管理的應用狀態。
/// @return 設定鍵值表。
pub(crate) async fn get_user_settings(state: State<'_, AppState>) -> CommandResult<UserSettings> {
    state.runtime().user_settings().await
}

#[tauri::command]
/// 驗證並替換完整使用者設定。
/// @param settings 所有使用者設定的快照。
/// @param state Tauri 管理的應用狀態。
/// @return 儲存結果。
pub(crate) async fn save_user_settings(
    settings: UserSettings,
    state: State<'_, AppState>,
) -> CommandResult<()> {
    state.runtime().save_user_settings(settings).await
}
