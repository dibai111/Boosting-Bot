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

//! 封裝 Microsoft 裝置代碼登入與權杖更新，保留舊 Launcher 的相容流程。

use super::{access_token, profile, ACCESS_TOKEN_EXPIRED, ACCESS_TOKEN_INVALID};
use anyhow::{bail, Context, Result};
use azalea::auth as azalea_auth;
use chrono::{DateTime, Utc};

const LEGACY_PUBLIC_CLIENT_ID: &str = "00000000402b5328";
const LEGACY_SCOPE: &str = "service::user.auth.xboxlive.com::MBI_SSL";

/// Microsoft 更新權杖及其對應的 Minecraft 工作階段；僅供後端使用。
pub(crate) struct MicrosoftSession {
    pub(crate) refresh_token: String,
    pub(crate) access_token: String,
    pub(crate) username: String,
    pub(crate) uuid: String,
    pub(crate) expires_at: DateTime<Utc>,
}

/// 啟動裝置代碼登入，取得使用者需完成的驗證資訊。
/// @param client 共用 HTTP client。
/// @return 裝置代碼、驗證網址與輪詢期限。
pub(crate) async fn request_device_code(
    client: &reqwest::Client,
) -> Result<azalea_auth::DeviceCodeResponse> {
    azalea_auth::get_ms_link_code(client, None, None)
        .await
        .context("request Microsoft device code")
}

/// 等待裝置驗證完成，再交換並驗證 Minecraft 工作階段。
/// @param client 共用 HTTP client。
/// @param code 同一次登入取得的裝置代碼回應。
/// @return 可保存的更新權杖與 Minecraft 工作階段。
pub(crate) async fn complete_device_login(
    client: &reqwest::Client,
    code: azalea_auth::DeviceCodeResponse,
) -> Result<MicrosoftSession> {
    let microsoft = azalea_auth::get_ms_auth_token(client, code, None)
        .await
        .context("complete Microsoft device login")?;
    session_from_microsoft_token(
        client,
        &microsoft.data.access_token,
        microsoft.data.refresh_token,
        false,
    )
    .await
}

/// 將舊版輸入的 Microsoft refresh token 換成目前可用的 Minecraft session。
/// 沿用舊 Launcher 的 client ID 與 scope 更新登入。
/// @param client 共用 HTTP client。
/// @param refresh_token 舊 Launcher 提供的 Microsoft refresh token。
/// @return 更新後的 Minecraft 工作階段。
pub(crate) async fn refresh_legacy(
    client: &reqwest::Client,
    refresh_token: &str,
) -> Result<MicrosoftSession> {
    let microsoft = azalea_auth::refresh_ms_auth_token(
        client,
        refresh_token,
        Some(LEGACY_PUBLIC_CLIENT_ID),
        Some(LEGACY_SCOPE),
    )
    .await
    .context("refresh Microsoft login")?;
    session_from_microsoft_token(
        client,
        &microsoft.data.access_token,
        microsoft.data.refresh_token,
        true,
    )
    .await
}

/// 以 Azalea device-code flow 產生的 refresh token 取得新的 Minecraft session。
/// 更新 Azalea 裝置登入產生的工作階段。
/// @param client 共用 HTTP client。
/// @param refresh_token 已保存的 Microsoft refresh token。
/// @return 新權杖、玩家資料與期限。
pub(crate) async fn refresh(
    client: &reqwest::Client,
    refresh_token: &str,
) -> Result<MicrosoftSession> {
    let microsoft = azalea_auth::refresh_ms_auth_token(client, refresh_token, None, None)
        .await
        .context("refresh Microsoft login")?;
    session_from_microsoft_token(
        client,
        &microsoft.data.access_token,
        microsoft.data.refresh_token,
        false,
    )
    .await
}

/// 完成 Xbox／Minecraft 交換並驗證期限與玩家資料。
/// @param client 共用 HTTP client。
/// @param microsoft_access_token Microsoft access token。
/// @param refresh_token 需保存在新工作階段的更新權杖。
/// @param legacy_client 是否先嘗試舊版 RPS ticket 格式。
/// @return 已驗證的 MicrosoftSession。
async fn session_from_microsoft_token(
    client: &reqwest::Client,
    microsoft_access_token: &str,
    refresh_token: String,
    legacy_client: bool,
) -> Result<MicrosoftSession> {
    let minecraft = if legacy_client {
        // 舊版 Launcher refresh token 使用不同 client id；先保留原本的 RPS ticket 格式。
        match azalea_auth::get_minecraft_token(client, &format!("t={microsoft_access_token}")).await
        {
            Ok(token) => Ok(token),
            Err(_) => azalea_auth::get_minecraft_token(client, microsoft_access_token).await,
        }
    } else {
        azalea_auth::get_minecraft_token(client, microsoft_access_token).await
    }
    .context("exchange Microsoft token for Minecraft token")?;

    let access_token = minecraft.minecraft_access_token;
    let expires_at = access_token::expiry(&access_token).context(ACCESS_TOKEN_INVALID)?;
    if expires_at <= Utc::now() {
        bail!("{ACCESS_TOKEN_EXPIRED}");
    }
    let profile = profile::fetch(client, &access_token).await?;

    Ok(MicrosoftSession {
        refresh_token,
        access_token,
        username: profile.name,
        uuid: profile.id,
        expires_at,
    })
}
