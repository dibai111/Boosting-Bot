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

//! 驗證設定數量及字串大小，再以完整快照更新本機設定。

use crate::{Store, UserSettings};
use anyhow::{bail, Result};

const MAX_SETTINGS: usize = 64;
const MAX_SETTING_KEY_BYTES: usize = 128;
const MAX_SETTING_VALUE_BYTES: usize = 16 * 1024;

impl Store {
    /// 讀取使用者設定完整快照。
    /// @return 設定鍵值表；讀取或解密失敗時回傳錯誤。
    pub fn user_settings(&self) -> Result<UserSettings> {
        Ok(self.read_state()?.settings)
    }

    /// 驗證設定數量與字串長度後替換整份設定。
    /// @param settings 完整設定快照；呼叫端需避免舊快照覆蓋新值。
    /// @return 驗證及儲存結果。
    pub fn save_user_settings(&self, settings: UserSettings) -> Result<()> {
        if settings.len() > MAX_SETTINGS {
            bail!("too many local settings")
        }
        if settings.iter().any(|(key, value)| {
            key.is_empty()
                || key.len() > MAX_SETTING_KEY_BYTES
                || value.len() > MAX_SETTING_VALUE_BYTES
        }) {
            bail!("local setting is too large or has an empty key")
        }
        self.update_state(|state| {
            state.settings = settings;
            Ok(())
        })
    }
}
