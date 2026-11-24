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

//! 浮動視窗命令共用同一 controller，回覆及事件提供帶版本的實際狀態。
pub(crate) use crate::platform::overlay::MATCHMAKING_OVERLAY_LABEL;
use crate::{
    app_state::CommandResult,
    platform::overlay::{OverlayController, OverlayOperation, OverlaySnapshot},
};
use tauri::{AppHandle, Manager};

#[tauri::command]
/// 顯示視窗並回傳成功後的實際狀態。
pub(crate) async fn show_matchmaking_overlay(
    app: AppHandle,
    bot_count: usize,
) -> CommandResult<OverlaySnapshot> {
    app.state::<OverlayController>()
        .execute(&app, OverlayOperation::Show(bot_count))
        .await
}
#[tauri::command]
/// 切換視窗，與其他視窗操作共用序列化入口。
pub(crate) async fn toggle_matchmaking_overlay(
    app: AppHandle,
    bot_count: usize,
) -> CommandResult<OverlaySnapshot> {
    app.state::<OverlayController>()
        .execute(&app, OverlayOperation::Toggle(bot_count))
        .await
}
#[tauri::command]
/// 命令及關閉視窗事件都以此入口隱藏並同步狀態。
pub(crate) async fn hide_matchmaking_overlay(app: AppHandle) -> CommandResult<OverlaySnapshot> {
    app.state::<OverlayController>()
        .execute(&app, OverlayOperation::Hide)
        .await
}
#[tauri::command]
/// 讀取目前視窗實際狀態，與事件使用相同版本。
pub(crate) async fn get_overlay_snapshot(app: AppHandle) -> CommandResult<OverlaySnapshot> {
    app.state::<OverlayController>()
        .execute(&app, OverlayOperation::Read)
        .await
}
