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

//! 定義公開帳號與設定模型；傳回前端時不序列化登入憑據及工作階段權杖。

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub type UserSettings = BTreeMap<String, String>;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
/// 持久化及 IPC 共用的登入方式，序列化名稱保持 snake_case。
pub enum AuthKind {
    Microsoft,
    AccessToken,
    Cookie,
}

impl AuthKind {
    /// 取得登入方式的固定傳輸名稱。
    /// @return 與 serde 契約一致的靜態字串。
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Microsoft => "microsoft",
            Self::AccessToken => "access_token",
            Self::Cookie => "cookie",
        }
    }
}

impl TryFrom<&str> for AuthKind {
    type Error = String;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value {
            "microsoft" => Ok(Self::Microsoft),
            "access_token" => Ok(Self::AccessToken),
            "cookie" => Ok(Self::Cookie),
            _ => Err(format!("unsupported auth kind: {value}")),
        }
    }
}

/// 前端可見的帳號資料；憑據只供後端使用，透過 serde 排除於輸出。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccountRecord {
    pub id: String,
    pub username: String,
    pub profile_id: Option<String>,
    pub auth_kind: AuthKind,
    #[serde(skip_serializing)]
    pub credential: Option<String>,
    #[serde(skip_serializing)]
    pub session_token: Option<String>,
    pub session_expires_at: Option<DateTime<Utc>>,
    pub credential_checked_at: Option<DateTime<Utc>>,
    pub server_address: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Deserialize)]
/// 建立帳號的輸入；憑據只在後端驗證與保存。
pub struct CreateAccountInput {
    pub username: String,
    pub auth_kind: AuthKind,
    pub credential: Option<String>,
    pub server_address: String,
}
