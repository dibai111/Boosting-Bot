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

//! 以已取得的 Minecraft access token 實作 Azalea 帳號介面及憑證快取。

use azalea::{
    account::AccountTrait,
    auth::{
        certs::Certificates,
        sessionserver::{self, ClientSessionServerError, SessionServerJoinOpts},
    },
};
use std::{fmt, future::Future, pin::Pin, sync::Mutex};
use uuid::Uuid;

/// 將已驗證 Minecraft 權杖接入 Azalea 帳號介面，Debug 輸出遮蔽權杖。
pub(super) struct MinecraftAccessTokenAccount {
    username: String,
    uuid: Uuid,
    access_token: String,
    certificates: Mutex<Option<Certificates>>,
}

impl MinecraftAccessTokenAccount {
    /// 建立帳號介面並初始化空的憑證快取。
    /// @param username Minecraft 玩家名稱。
    /// @param uuid Minecraft profile UUID。
    /// @param access_token 已驗證的 Minecraft access token。
    /// @return 供 Azalea 連線使用的帳號。
    pub(super) fn new(username: String, uuid: Uuid, access_token: String) -> Self {
        Self {
            username,
            uuid,
            access_token,
            certificates: Mutex::new(None),
        }
    }
}

impl fmt::Debug for MinecraftAccessTokenAccount {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("MinecraftAccessTokenAccount")
            .field("username", &self.username)
            .field("uuid", &self.uuid)
            .field("access_token", &"[redacted]")
            .finish()
    }
}

impl AccountTrait for MinecraftAccessTokenAccount {
    fn username(&self) -> &str {
        &self.username
    }

    fn uuid(&self) -> Uuid {
        self.uuid
    }

    fn access_token(&self) -> Option<String> {
        Some(self.access_token.clone())
    }

    fn certs(&self) -> Option<Certificates> {
        self.certificates
            .lock()
            .ok()
            .and_then(|certificates| certificates.clone())
    }

    fn set_certs(&self, certificates: Certificates) {
        if let Ok(mut stored) = self.certificates.lock() {
            *stored = Some(certificates);
        }
    }

    /// 完成 Minecraft session server 的加入驗證。
    /// @param public_key 遊戲伺服器提供的公鑰。
    /// @param private_key 本次連線的共享密鑰。
    /// @param server_id 伺服器握手識別字串。
    /// @param proxy 選用的 HTTP proxy。
    /// @return 可等待的 session server 驗證結果。
    fn join<'a>(
        &'a self,
        public_key: &'a [u8],
        private_key: &'a [u8; 16],
        server_id: &'a str,
        proxy: Option<reqwest::Proxy>,
    ) -> Pin<Box<dyn Future<Output = Result<(), ClientSessionServerError>> + Send + 'a>> {
        Box::pin(async move {
            sessionserver::join(SessionServerJoinOpts {
                access_token: &self.access_token,
                public_key,
                private_key,
                uuid: &self.uuid,
                server_id,
                proxy,
            })
            .await
        })
    }
}
