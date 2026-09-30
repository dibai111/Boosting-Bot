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

//! 處理帳號增刪與登入工作階段更新，並以正規化 UUID 防止重複建立。

use crate::{
    models::{AccountSecrets, AccountSession},
    state::StoredAccount,
};
use crate::{AccountRecord, CreateAccountInput, Store};
use anyhow::{bail, Context, Result};
use chrono::{DateTime, Utc};
use std::collections::HashSet;
use uuid::Uuid;

const ACCOUNT_ALREADY_EXISTS: &str = "account_already_exists";

impl Store {
    /// 讀取帳號快照並依建立時間排序。
    ///
    /// 回傳資料只包含公開欄位，不包含後端憑據。
    pub fn list_accounts(&self) -> Result<Vec<AccountRecord>> {
        let mut accounts = self
            .read_state()?
            .accounts
            .into_iter()
            .map(AccountRecord::from)
            .collect::<Vec<_>>();
        accounts.sort_by_key(|account| account.created_at);
        Ok(accounts)
    }

    /// 查詢目前儲存的帳號數量。
    ///
    /// 超出 `u32` 或讀取失敗時回傳錯誤。
    pub fn account_count(&self) -> Result<u32> {
        u32::try_from(self.read_state()?.accounts.len())
            .context("account count exceeds supported range")
    }

    /// 以本機識別碼尋找帳號。
    ///
    /// `id` 是本機帳號 UUID，並非 Minecraft UUID。找不到時回傳 `None`。
    pub fn account(&self, id: &str) -> Result<Option<AccountRecord>> {
        Ok(self
            .read_state()?
            .accounts
            .into_iter()
            .find(|account| account.id == id)
            .map(AccountRecord::from))
    }

    /// 取得後端登入工作階段，僅供 runtime 使用，不能作為 IPC 回應。
    pub fn account_session(&self, id: &str) -> Result<Option<AccountSession>> {
        Ok(self
            .read_state()?
            .accounts
            .into_iter()
            .find(|account| account.id == id)
            .map(|stored| AccountSession {
                account: AccountRecord::from(stored.clone()),
                secrets: stored.secrets,
            }))
    }

    /// 拒絕重複 Minecraft UUID 後建立帳號，成功才寫回儲存。
    ///
    /// 憑據只會寫入加密的後端狀態；重複 UUID 或儲存失敗時回傳錯誤。
    pub fn create_account(
        &self,
        input: CreateAccountInput,
        profile_id: Option<String>,
        session_token: Option<String>,
        session_expires_at: Option<DateTime<Utc>>,
    ) -> Result<AccountRecord> {
        self.update_state(|state| {
            if let Some(profile_id) = profile_id.as_deref() {
                let normalized_id = normalize_profile_id(profile_id);
                validate_profile_id(&normalized_id)?;
                if state.accounts.iter().any(|account| {
                    account
                        .profile_id
                        .as_deref()
                        .is_some_and(|value| normalize_profile_id(value) == normalized_id)
                }) {
                    bail!(ACCOUNT_ALREADY_EXISTS);
                }
            }

            let now = Utc::now();
            let stored = StoredAccount {
                id: Uuid::new_v4().to_string(),
                username: input.username.trim().to_owned(),
                profile_id: profile_id.map(|value| normalize_profile_id(&value)),
                auth_kind: input.auth_kind,
                secrets: AccountSecrets {
                    credential: input.credential,
                    session_token,
                },
                session_expires_at,
                credential_checked_at: Some(now),
                server_address: input.server_address.trim().to_owned(),
                created_at: now,
                updated_at: now,
            };
            let record = AccountRecord::from(stored.clone());
            state.accounts.push(stored);
            Ok(record)
        })
    }

    /// 批次移除指定帳號，重複或不存在的 ID 不重複計數。
    ///
    /// `ids` 是待刪除的本機帳號識別碼；回傳實際移除筆數。
    pub fn delete_accounts(&self, ids: &[String]) -> Result<usize> {
        let ids: HashSet<&str> = ids.iter().map(String::as_str).collect();
        self.update_state(|state| {
            let before = state.accounts.len();
            state
                .accounts
                .retain(|account| !ids.contains(account.id.as_str()));
            Ok(before - state.accounts.len())
        })
    }

    /// 更新帳號的目標伺服器及修改時間。
    ///
    /// `address` 會去除首尾空白；帳號不存在或地址為空時回傳錯誤。
    pub fn update_server_address(&self, id: &str, address: &str) -> Result<()> {
        let address = address.trim();
        if address.is_empty() {
            bail!("server address is required");
        }
        self.update_state(|state| {
            let account = state
                .accounts
                .iter_mut()
                .find(|account| account.id == id)
                .ok_or_else(|| anyhow::anyhow!("account_not_found"))?;
            account.server_address = address.to_owned();
            account.updated_at = Utc::now();
            Ok(())
        })
    }

    /// 同步登入事件帶回的玩家資料。
    ///
    /// `profile_id` 為 `None` 時保留原值；帳號不存在或資料無效時回傳錯誤。
    pub fn update_profile(&self, id: &str, username: &str, profile_id: Option<&str>) -> Result<()> {
        let username = username.trim();
        if username.is_empty() {
            bail!("username is required");
        }
        let profile_id = profile_id.map(normalize_profile_id);
        self.update_state(|state| {
            let index = state
                .accounts
                .iter()
                .position(|account| account.id == id)
                .ok_or_else(|| anyhow::anyhow!("account_not_found"))?;
            if let Some(profile_id) = profile_id {
                validate_profile_id(&profile_id)?;
                ensure_unique_profile_id(state, id, &profile_id)?;
                state.accounts[index].profile_id = Some(profile_id);
            }
            let account = &mut state.accounts[index];
            account.username = username.to_owned();
            account.updated_at = Utc::now();
            account.credential_checked_at = Some(account.updated_at);
            Ok(())
        })
    }

    /// 一次更新玩家資料、憑據與工作階段期限。
    ///
    /// `credential` 為 `None` 時保留原憑據；`expires_at` 使用 UTC。
    /// 帳號不存在、UUID 無效或 session token 為空時回傳錯誤。
    pub fn update_auth_session(
        &self,
        id: &str,
        username: &str,
        profile_id: &str,
        credential: Option<&str>,
        session_token: &str,
        expires_at: Option<DateTime<Utc>>,
    ) -> Result<()> {
        let username = username.trim();
        let profile_id = normalize_profile_id(profile_id);
        if username.is_empty() {
            bail!("username is required");
        }
        validate_profile_id(&profile_id)?;
        if session_token.trim().is_empty() {
            bail!("session token is required");
        }
        self.update_state(|state| {
            let index = state
                .accounts
                .iter()
                .position(|account| account.id == id)
                .ok_or_else(|| anyhow::anyhow!("account_not_found"))?;
            ensure_unique_profile_id(state, id, &profile_id)?;
            let account = &mut state.accounts[index];
            account.username = username.to_owned();
            account.profile_id = Some(profile_id);
            if let Some(credential) = credential {
                account.secrets.credential = Some(credential.to_owned());
            }
            account.secrets.session_token = Some(session_token.to_owned());
            account.session_expires_at = expires_at;
            account.updated_at = Utc::now();
            account.credential_checked_at = Some(account.updated_at);
            Ok(())
        })
    }
}

impl From<StoredAccount> for AccountRecord {
    fn from(account: StoredAccount) -> Self {
        Self {
            id: account.id,
            username: account.username,
            profile_id: account.profile_id,
            auth_kind: account.auth_kind,
            session_expires_at: account.session_expires_at,
            credential_checked_at: account.credential_checked_at,
            server_address: account.server_address,
            created_at: account.created_at,
            updated_at: account.updated_at,
        }
    }
}

fn validate_profile_id(profile_id: &str) -> Result<()> {
    Uuid::parse_str(profile_id).context("invalid Minecraft profile UUID")?;
    Ok(())
}

fn ensure_unique_profile_id(
    state: &crate::state::PersistedState,
    id: &str,
    profile_id: &str,
) -> Result<()> {
    if state.accounts.iter().any(|account| {
        account.id != id
            && account
                .profile_id
                .as_deref()
                .is_some_and(|existing| normalize_profile_id(existing) == profile_id)
    }) {
        bail!(ACCOUNT_ALREADY_EXISTS);
    }
    Ok(())
}

/// 正規化 UUID 的字面差異，供重複帳號比對。
///
/// 移除空白與連字號並轉成小寫。
fn normalize_profile_id(profile_id: &str) -> String {
    profile_id
        .trim()
        .chars()
        .filter(|character| *character != '-')
        .flat_map(char::to_lowercase)
        .collect()
}
