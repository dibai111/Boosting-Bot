use super::{access_token, profile, ACCESS_TOKEN_EXPIRED, ACCESS_TOKEN_INVALID};
use anyhow::{bail, Context, Result};
use azalea::auth as azalea_auth;
use chrono::{DateTime, Utc};

const LEGACY_PUBLIC_CLIENT_ID: &str = "00000000402b5328";
const LEGACY_SCOPE: &str = "service::user.auth.xboxlive.com::MBI_SSL";

pub(crate) struct MicrosoftSession {
    pub(crate) refresh_token: String,
    pub(crate) access_token: String,
    pub(crate) username: String,
    pub(crate) uuid: String,
    pub(crate) expires_at: DateTime<Utc>,
}

pub(crate) async fn request_device_code(
    client: &reqwest::Client,
) -> Result<azalea_auth::DeviceCodeResponse> {
    azalea_auth::get_ms_link_code(client, None, None)
        .await
        .context("request Microsoft device code")
}

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
