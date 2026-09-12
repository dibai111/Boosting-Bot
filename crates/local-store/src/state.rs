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

//! 管理儲存版本、JSON 編解碼及讀取後修改再寫入的流程；呼叫端需序列化更新。

use crate::{protection, registry, Store};
use anyhow::{bail, Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

const STATE_VERSION: u32 = 1;

#[derive(Debug, Serialize, Deserialize)]
/// 含格式版本的持久化根物件；帳號與設定以同一份加密資料保存。
pub(crate) struct PersistedState {
    version: u32,
    #[serde(default)]
    pub(crate) accounts: Vec<StoredAccount>,
    #[serde(default)]
    pub(crate) settings: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
/// 僅供後端持久化的帳號資料，含不可傳送至前端的登入憑據。
pub(crate) struct StoredAccount {
    pub id: String,
    pub username: String,
    pub profile_id: Option<String>,
    pub auth_kind: crate::AuthKind,
    pub credential: Option<String>,
    pub session_token: Option<String>,
    pub session_expires_at: Option<DateTime<Utc>>,
    pub server_address: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub credential_checked_at: Option<DateTime<Utc>>,
}

impl Default for PersistedState {
    fn default() -> Self {
        Self {
            version: STATE_VERSION,
            accounts: Vec::new(),
            settings: BTreeMap::new(),
        }
    }
}

impl PersistedState {
    fn validate(&self) -> Result<()> {
        if self.version != STATE_VERSION {
            bail!("unsupported local store version: {}", self.version)
        }
        Ok(())
    }
}

impl Store {
    /// 讀取加密值，解密後檢查大小、JSON 及格式版本。
    /// @return 已驗證狀態；首次使用回傳空狀態。
    pub(crate) fn read_state(&self) -> Result<PersistedState> {
        let Some(protected) = registry::read_value(&self.registry_path)? else {
            return Ok(PersistedState::default());
        };
        let plaintext = protection::unprotect(&protected)?;
        if plaintext.len() > registry::MAX_VALUE_BYTES {
            bail!("decrypted local state is larger than the supported limit")
        }
        let state: PersistedState =
            serde_json::from_slice(&plaintext).context("decode encrypted local state")?;
        state.validate()?;
        Ok(state)
    }

    /// 驗證與序列化狀態，經 DPAPI 加密後寫入 Registry。
    /// @param state 完整且版本相容的待存狀態。
    /// @return 寫入結果；過大資料或加密失敗時回傳錯誤。
    pub(crate) fn write_state(&self, state: &PersistedState) -> Result<()> {
        state.validate()?;
        let plaintext = serde_json::to_vec(state).context("encode local state")?;
        if plaintext.len() > registry::MAX_VALUE_BYTES {
            bail!("local state is larger than the supported limit")
        }
        let protected = protection::protect(&plaintext)?;
        registry::write_value(&self.registry_path, &protected)
    }

    // 呼叫端須持有 Store 的共用鎖；更新失敗時不寫入任何部分修改。
    /// 在同一次讀改寫中套用更新；呼叫端必須持有共用 Store 鎖。
    /// @param update 修改快照的閉包；回傳錯誤時不寫入。
    /// @return 更新結果；只有寫回成功才回傳成功值。
    pub(crate) fn update_state<T, F>(&self, update: F) -> Result<T>
    where
        F: FnOnce(&mut PersistedState) -> Result<T>,
    {
        let mut state = self.read_state()?;
        let result = update(&mut state)?;
        self.write_state(&state)?;
        Ok(result)
    }
}
