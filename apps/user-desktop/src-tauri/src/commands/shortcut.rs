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

//! 將前端設定的快捷鍵交給平台層解析及套用。

use crate::{app_state::CommandResult, platform::shortcuts::MatchmakingShortcuts};
use tauri::State;

#[tauri::command]
/// 解析並套用配對快捷鍵，拒絕衝突組合。
/// @param shortcuts Tauri 管理的快捷鍵服務。
/// @param stop_shortcut 配對啟停按鍵字串。
/// @param show_overlay_shortcut 浮動視窗切換按鍵字串。
/// @return 設定結果或穩定錯誤代碼。
pub(crate) fn configure_matchmaking_shortcuts(
    shortcuts: State<'_, MatchmakingShortcuts>,
    stop_shortcut: String,
    show_overlay_shortcut: String,
) -> CommandResult<()> {
    shortcuts.configure(&stop_shortcut, &show_overlay_shortcut)
}
