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

//! 在 Cargo 建置時產生 Tauri 必需的資源，設定變更後重新執行。

/// 產生 Tauri 建置所需的資源與權限資料。
/// @return 完成 Cargo build script；錯誤由 tauri-build 回報。
fn main() {
    println!("cargo:rerun-if-changed=tauri.conf.json");
    tauri_build::build()
}
