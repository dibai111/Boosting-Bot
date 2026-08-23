use anyhow::{bail, Context, Result};
use azalea::auth as azalea_auth;

pub(crate) struct MinecraftProfile {
    pub(crate) id: String,
    pub(crate) name: String,
}

/// 透過 Azalea 驗證 Minecraft access token，並取得帳號基本資料。
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
