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

//! 建立及切換配對浮動視窗，並以事件同步其可見狀態。

use crate::app_state::CommandResult;
use tauri::{
    AppHandle, Emitter, Manager, PhysicalPosition, PhysicalSize, WebviewUrl, WebviewWindowBuilder,
};

pub(crate) const MATCHMAKING_OVERLAY_LABEL: &str = "matchmaking-overlay";
const OVERLAY_VISIBILITY_EVENT: &str = "matchmaking-overlay-visibility";
const OVERLAY_DESIGN_WIDTH: u32 = 350;
const OVERLAY_DESIGN_BASE_HEIGHT: u32 = 179;
const OVERLAY_DESIGN_BOT_HEIGHT: u32 = 57;
const OVERLAY_MAX_VISIBLE_BOTS: usize = 5;
const REFERENCE_DPI_NUMERATOR: u32 = 5;
const REFERENCE_DPI_DENOMINATOR: u32 = 4;
const OVERLAY_RIGHT_MARGIN: i32 = 30;
const OVERLAY_TOP_MARGIN: i32 = 60;

#[tauri::command]
/// 建立或重用配對浮動視窗，依 Bot 數調整大小。
/// @param app 目前 Tauri 應用控制柄。
/// @param bot_count 用於視窗高度計算的 Bot 數。
/// @return 視窗操作結果。
pub(crate) async fn show_matchmaking_overlay(
    app: AppHandle,
    bot_count: usize,
) -> CommandResult<()> {
    let overlay_size = overlay_size(bot_count);
    let overlay_position = overlay_position(&app, overlay_size);
    if let Some(window) = app.get_webview_window(MATCHMAKING_OVERLAY_LABEL) {
        window
            .set_size(overlay_size)
            .map_err(|error| error.to_string())?;
        window
            .set_position(overlay_position)
            .map_err(|error| error.to_string())?;
        emit_overlay_visibility(&app, true)?;
        window.show().map_err(|error| error.to_string())?;
        return Ok(());
    }

    let window = WebviewWindowBuilder::new(
        &app,
        MATCHMAKING_OVERLAY_LABEL,
        WebviewUrl::App("index.html?overlay=matchmaking".into()),
    )
    .title("Matchmaking")
    .decorations(false)
    .transparent(true)
    .shadow(false)
    .resizable(false)
    .maximizable(false)
    .minimizable(false)
    .always_on_top(true)
    .skip_taskbar(true)
    .focused(false)
    .visible(false)
    .build()
    .map_err(|error| error.to_string())?;
    window
        .set_size(overlay_size)
        .map_err(|error| error.to_string())?;
    window
        .set_position(overlay_position)
        .map_err(|error| error.to_string())?;
    emit_overlay_visibility(&app, true)?;
    window.show().map_err(|error| error.to_string())?;
    Ok(())
}

#[tauri::command]
/// 切換配對浮動視窗可見狀態。
/// @param app 目前 Tauri 應用控制柄。
/// @param bot_count 顯示時採用的 Bot 數。
/// @return 切換後是否可見。
pub(crate) async fn toggle_matchmaking_overlay(
    app: AppHandle,
    bot_count: usize,
) -> CommandResult<bool> {
    if let Some(window) = app.get_webview_window(MATCHMAKING_OVERLAY_LABEL) {
        if window.is_visible().map_err(|error| error.to_string())? {
            window.hide().map_err(|error| error.to_string())?;
            emit_overlay_visibility(&app, false)?;
            return Ok(false);
        }
    }

    show_matchmaking_overlay(app, bot_count).await?;
    Ok(true)
}

#[tauri::command]
/// 隱藏配對浮動視窗並發布可見狀態。
/// @param app 目前 Tauri 應用控制柄。
/// @return 隱藏及事件發送結果。
pub(crate) async fn hide_matchmaking_overlay(app: AppHandle) -> CommandResult<()> {
    if let Some(window) = app.get_webview_window(MATCHMAKING_OVERLAY_LABEL) {
        window.hide().map_err(|error| error.to_string())?;
    }
    emit_overlay_visibility(&app, false)?;
    Ok(())
}

fn emit_overlay_visibility(app: &AppHandle, visible: bool) -> CommandResult<()> {
    app.emit(OVERLAY_VISIBILITY_EVENT, visible)
        .map_err(|error| error.to_string())
}

fn overlay_position(app: &AppHandle, overlay_size: PhysicalSize<u32>) -> PhysicalPosition<i32> {
    let Ok(Some(monitor)) = app.primary_monitor() else {
        return PhysicalPosition::new(OVERLAY_RIGHT_MARGIN, OVERLAY_TOP_MARGIN);
    };
    let size = monitor.size();
    let position = monitor.position();
    let monitor_width = i32::try_from(size.width).unwrap_or(i32::MAX);
    let overlay_width = i32::try_from(overlay_size.width).unwrap_or(i32::MAX);
    let right = position.x.saturating_add(monitor_width);

    PhysicalPosition::new(
        right
            .saturating_sub(overlay_width)
            .saturating_sub(OVERLAY_RIGHT_MARGIN),
        position.y.saturating_add(OVERLAY_TOP_MARGIN),
    )
}

fn overlay_size(bot_count: usize) -> PhysicalSize<u32> {
    let visible_bots = bot_count.clamp(1, OVERLAY_MAX_VISIBLE_BOTS) as u32;
    let design_height = OVERLAY_DESIGN_BASE_HEIGHT + visible_bots * OVERLAY_DESIGN_BOT_HEIGHT;

    PhysicalSize::new(
        reference_pixels(OVERLAY_DESIGN_WIDTH),
        reference_pixels(design_height),
    )
}

fn reference_pixels(design_pixels: u32) -> u32 {
    (design_pixels
        .saturating_mul(REFERENCE_DPI_NUMERATOR)
        .saturating_add(REFERENCE_DPI_DENOMINATOR / 2))
        / REFERENCE_DPI_DENOMINATOR
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn one_bot_overlay_matches_the_reference_physical_dimensions() {
        assert_eq!(overlay_size(1), PhysicalSize::new(438, 295));
    }

    #[test]
    fn overlay_height_clamps_to_the_visible_bot_limit() {
        assert_eq!(overlay_size(0), overlay_size(1));
        assert_eq!(
            overlay_size(OVERLAY_MAX_VISIBLE_BOTS + 1),
            PhysicalSize::new(438, 580)
        );
    }
}
