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

use crate::state::StoredAccount;
use crate::{AccountRecord, CreateAccountInput, Store};
use anyhow::{bail, Context, Result};
use chrono::{DateTime, Utc};
use std::collections::HashSet;
use uuid::Uuid;

const ACCOUNT_ALREADY_EXISTS: &str = "account_already_exists";

impl Store {
    /// 讀取帳號快照並依建立時間排序。
    /// @return 帳號清單；序列化時排除後端憑據。
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
    /// @return 帳號數；超出 u32 或讀取失敗時回傳錯誤。
    pub fn account_count(&self) -> Result<u32> {
        u32::try_from(self.read_state()?.accounts.len())
            .context("account count exceeds supported range")
    }

    /// 以本機識別碼尋找帳號。
    /// @param id 本機帳號 UUID，並非 Minecraft UUID。
    /// @return 找到的帳號；不存在時為 None。
    pub fn account(&self, id: &str) -> Result<Option<AccountRecord>> {
        Ok(self
            .read_state()?
            .accounts
            .into_iter()
            .find(|account| account.id == id)
            .map(AccountRecord::from))
    }

    /// 拒絕重複 Minecraft UUID 後建立帳號，成功才寫回儲存。
    /// @param input 登入方式、憑據、名稱與伺服器地址。
    /// @param profile_id 已驗證的 Minecraft UUID。
    /// @param session_token 已交換取得的 Minecraft access token。
    /// @param session_expires_at 工作階段 UTC 到期時間。
    /// @return 新帳號；重複 UUID 或儲存失敗時回傳錯誤。
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
                profile_id,
                auth_kind: input.auth_kind,
                credential: input.credential,
                session_token,
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
    /// @param ids 要刪除的本機帳號 ID。
    /// @return 實際刪除筆數。
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
    /// @param id 本機帳號 ID。
    /// @param address 會去除首尾空白的伺服器地址。
    /// @return 儲存結果；帳號不存在時不新增資料。
    pub fn update_server_address(&self, id: &str, address: &str) -> Result<()> {
        self.update_state(|state| {
            if let Some(account) = state.accounts.iter_mut().find(|account| account.id == id) {
                account.server_address = address.trim().to_owned();
                account.updated_at = Utc::now();
            }
            Ok(())
        })
    }

    /// 同步登入事件帶回的玩家資料。
    /// @param id 本機帳號 ID。
    /// @param username 已驗證的玩家名稱。
    /// @param profile_id 新 Minecraft UUID；None 時保留原值。
    /// @return 儲存結果；帳號不存在時不新增資料。
    pub fn update_profile(&self, id: &str, username: &str, profile_id: Option<&str>) -> Result<()> {
        self.update_state(|state| {
            if let Some(account) = state.accounts.iter_mut().find(|account| account.id == id) {
                account.username = username.to_owned();
                if profile_id.is_some() {
                    account.profile_id = profile_id.map(str::to_owned);
                }
                account.updated_at = Utc::now();
                account.credential_checked_at = Some(account.updated_at);
            }
            Ok(())
        })
    }

    /// 一次更新玩家資料、憑據與工作階段期限。
    /// @param id 本機帳號 ID。
    /// @param username 已驗證的玩家名稱。
    /// @param profile_id 已驗證的 Minecraft UUID。
    /// @param credential 新登入憑據；None 時保留原憑據。
    /// @param session_token 新 Minecraft access token。
    /// @param expires_at UTC 到期時間。
    /// @return 儲存結果；帳號不存在時不新增資料。
    pub fn update_auth_session(
        &self,
        id: &str,
        username: &str,
        profile_id: &str,
        credential: Option<&str>,
        session_token: &str,
        expires_at: Option<DateTime<Utc>>,
    ) -> Result<()> {
        self.update_state(|state| {
            if let Some(account) = state.accounts.iter_mut().find(|account| account.id == id) {
                account.username = username.to_owned();
                account.profile_id = Some(profile_id.to_owned());
                if let Some(credential) = credential {
                    account.credential = Some(credential.to_owned());
                }
                account.session_token = Some(session_token.to_owned());
                account.session_expires_at = expires_at;
                account.updated_at = Utc::now();
                account.credential_checked_at = Some(account.updated_at);
            }
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
            credential: account.credential,
            session_token: account.session_token,
            session_expires_at: account.session_expires_at,
            credential_checked_at: account.credential_checked_at,
            server_address: account.server_address,
            created_at: account.created_at,
            updated_at: account.updated_at,
        }
    }
}

/// 正規化 UUID 的字面差異，供重複帳號比對。
/// @param profile_id 帶連字號或大小寫差異的 UUID。
/// @return 去除空白、連字號並轉成小寫的識別碼。
fn normalize_profile_id(profile_id: &str) -> String {
    profile_id
        .trim()
        .chars()
        .filter(|character| *character != '-')
        .flat_map(char::to_lowercase)
        .collect()
}
