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

//! Windows 桌面執行入口；將初始化交給函式庫並隱藏額外的主控台視窗。

#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

/// 啟動桌面應用程式的 Tauri 入口。
/// @return 應用事件迴圈結束後返回。
fn main() {
    botting_user_lib::run();
}
