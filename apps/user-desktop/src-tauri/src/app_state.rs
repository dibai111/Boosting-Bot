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

//! Tauri 管理的應用程式狀態，只提供共用 runtime 的參照。

use crate::runtime::UserRuntime;
use std::sync::Arc;

/// Tauri 共用的應用服務參照，不直接擁有業務資料。
pub(crate) struct AppState {
    // Tauri state 只暴露應用能力，基礎設施由 runtime 內部管理。
    runtime: Arc<UserRuntime>,
}

pub(crate) type CommandResult<T> = Result<T, String>;

impl AppState {
    /// 包裝已建立的應用 runtime。
    /// @param runtime 共用 UserRuntime。
    /// @return 可由 Tauri manage 的狀態。
    pub(crate) fn new(runtime: Arc<UserRuntime>) -> Self {
        Self { runtime }
    }

    /// 取得可跨非同步工作使用的 runtime 參照。
    /// @return 增加參照計數的 Arc 副本。
    pub(crate) fn runtime(&self) -> Arc<UserRuntime> {
        Arc::clone(&self.runtime)
    }
}
