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

//! 透過共用儲存鎖讀寫設定，避免同時更新遺失資料。

use super::UserRuntime;
use crate::app_state::CommandResult;
use local_store::UserSettings;

impl UserRuntime {
    /// 在共用儲存鎖內讀取設定。
    /// @return 完整設定快照或儲存錯誤。
    pub(crate) async fn user_settings(&self) -> CommandResult<UserSettings> {
        self.with_store(|store| store.user_settings().map_err(|error| error.to_string()))
            .await
    }

    /// 以完整快照更新使用者設定。
    /// @param settings 已收集的所有使用者設定。
    /// @return 驗證與儲存結果。
    pub(crate) async fn save_user_settings(&self, settings: UserSettings) -> CommandResult<()> {
        self.with_store(move |store| {
            store
                .save_user_settings(settings)
                .map_err(|error| error.to_string())
        })
        .await
    }
}
