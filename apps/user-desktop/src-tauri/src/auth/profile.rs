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

//! 向 Minecraft Services 取得已驗證的玩家名稱與 UUID。

use anyhow::{bail, Context, Result};
use azalea::auth as azalea_auth;

/// Minecraft Services 驗證成功後的玩家名稱與 UUID。
pub(crate) struct MinecraftProfile {
    pub(crate) id: String,
    pub(crate) name: String,
}

/// 透過 Azalea 驗證 Minecraft access token，並取得帳號基本資料。
/// 驗證 access token 並讀取玩家資料。
/// @param client 共用 HTTP client。
/// @param access_token Minecraft access token。
/// @return 玩家資料；服務拒絕或名稱缺失時回傳錯誤。
pub(crate) async fn fetch(
    client: &reqwest::Client,
    access_token: &str,
) -> Result<MinecraftProfile> {
    let profile = azalea_auth::get_profile(client, access_token)
        .await
        .context("request Minecraft profile")?;
    if profile.name.trim().is_empty() {
        bail!("Minecraft profile response is incomplete");
    }

    Ok(MinecraftProfile {
        id: profile.id.to_string(),
        name: profile.name,
    })
}
