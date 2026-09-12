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

//! 將匯出的 Cookie 經 Xbox SISU 換成 Minecraft 登入工作階段。

use anyhow::{bail, Context, Result};
use base64::engine::general_purpose::{STANDARD, URL_SAFE, URL_SAFE_NO_PAD};
use base64::Engine;
use chrono::{DateTime, Utc};
use reqwest::{header, redirect::Policy, Client, Url};
use serde::Deserialize;
use std::collections::HashMap;

use super::{access_token, profile, ACCESS_TOKEN_EXPIRED, ACCESS_TOKEN_INVALID};

const USER_AGENT: &str =
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64; rv:127.0) Gecko/20100101 Firefox/127.0";
const SISU_CONNECT_URL: &str = "https://sisu.xboxlive.com/connect/XboxLive/?state=login&cobrandId=8058f65d-ce06-4c30-9559-473c9275a65d&tid=896928775&ru=https://www.minecraft.net/en-us/login&aid=1142970254";
const MINECRAFT_LOGIN_URL: &str =
    "https://api.minecraftservices.com/authentication/login_with_xbox";

#[derive(Debug, Clone)]
/// Cookie 登入交換結果；access token 僅供後端儲存與連線使用。
pub struct CookieSession {
    pub access_token: String,
    pub username: String,
    pub uuid: String,
    pub expires_at: DateTime<Utc>,
}

#[derive(Deserialize)]
struct SisuItem {
    #[serde(rename = "Item1")]
    resource: Option<String>,
    #[serde(rename = "Item2")]
    data: Option<SisuData>,
}

#[derive(Deserialize)]
struct SisuData {
    #[serde(rename = "DisplayClaims")]
    display_claims: Option<DisplayClaims>,
    #[serde(rename = "Token")]
    token: Option<String>,
}

#[derive(Deserialize)]
struct DisplayClaims {
    xui: Option<Vec<XuiClaim>>,
}

#[derive(Deserialize)]
struct XuiClaim {
    uhs: Option<String>,
}

#[derive(Deserialize)]
struct MinecraftLoginResponse {
    access_token: Option<String>,
}

/// 解析匯出 Cookie，交換 Xbox 身分與 Minecraft 工作階段。
/// @param cookie_text 以 Tab 分欄的 Cookie 匯出文字。
/// @return 玩家資料、權杖與期限；登入或格式失敗時回傳錯誤。
pub async fn exchange(cookie_text: &str) -> Result<CookieSession> {
    let cookie_header = parse_cookie_header(cookie_text)?;
    let client = Client::builder()
        .user_agent(USER_AGENT)
        .redirect(Policy::none())
        .build()
        .context("build cookie exchange client")?;

    let identity_token = request_xbox_identity_token(&client, &cookie_header).await?;
    let access_token = request_minecraft_access_token(&client, &identity_token).await?;
    let expires_at = access_token::expiry(&access_token).context(ACCESS_TOKEN_INVALID)?;
    if expires_at <= Utc::now() {
        bail!("{ACCESS_TOKEN_EXPIRED}");
    }
    let profile = profile::fetch(&client, &access_token).await?;

    Ok(CookieSession {
        access_token,
        username: profile.name,
        uuid: profile.id,
        expires_at,
    })
}

/// 沿允許的 HTTPS 登入端點追蹤最多十次重新導向。
/// @param client 停用自動重新導向的 HTTP client。
/// @param cookies 已解析的 Cookie header。
/// @return Minecraft Services 所需的 Xbox identity token。
async fn request_xbox_identity_token(client: &Client, cookies: &str) -> Result<String> {
    let mut url = Url::parse(SISU_CONNECT_URL)?;
    for _ in 0..10 {
        let response = client
            .get(url.clone())
            .header(header::COOKIE, cookies)
            .send()
            .await
            .context("request Xbox SISU login")?;
        let location = response
            .headers()
            .get(header::LOCATION)
            .and_then(|value| value.to_str().ok())
            .context("Xbox SISU did not return a login redirect")?
            .replace(' ', "%20");
        let next = url.join(&location).context("invalid Xbox SISU redirect")?;
        if !allowed_auth_host(&next) {
            bail!("Xbox SISU returned an unsupported login redirect");
        }
        if let Some(sisu_token) = url_field(&next, "accessToken") {
            return build_identity_token(&sisu_token);
        }
        url = next;
    }
    bail!("Xbox SISU redirect limit reached")
}

async fn request_minecraft_access_token(client: &Client, identity_token: &str) -> Result<String> {
    let response = client
        .post(MINECRAFT_LOGIN_URL)
        .json(&serde_json::json!({ "identityToken": identity_token }))
        .send()
        .await
        .context("request Minecraft access token")?;
    if !response.status().is_success() {
        bail!("Minecraft Services rejected the Xbox identity token");
    }
    response
        .json::<MinecraftLoginResponse>()
        .await
        .context("read Minecraft login response")?
        .access_token
        .filter(|token| !token.is_empty())
        .context("Minecraft login response did not contain an access token")
}

/// 從匯出文字最後兩欄取得 Cookie 名稱及值。
/// @param cookie_text 每行最後兩欄為名稱及值的 Tab 分欄文字。
/// @return Cookie header；沒有可用條目時回傳錯誤。
fn parse_cookie_header(cookie_text: &str) -> Result<String> {
    let mut cookies = HashMap::new();
    for line in cookie_text.lines() {
        let fields: Vec<&str> = line.trim_end_matches('\r').split('\t').collect();
        if fields.len() < 2 {
            continue;
        }
        let name = fields[fields.len() - 2].trim();
        let value = fields[fields.len() - 1].trim();
        if !name.is_empty() && !value.is_empty() {
            cookies.insert(name, value);
        }
    }
    if cookies.is_empty() {
        bail!("no cookie entries were provided");
    }
    Ok(cookies
        .into_iter()
        .map(|(name, value)| format!("{name}={value}"))
        .collect::<Vec<_>>()
        .join("; "))
}

/// 從 SISU JSON 選取 Minecraft Services 對應的 Xbox claims。
/// @param sisu_token Base64 編碼的 SISU token。
/// @return XBL3.0 identity token；缺少資源或 claims 時回傳錯誤。
fn build_identity_token(sisu_token: &str) -> Result<String> {
    let items: Vec<SisuItem> = decode_base64_json(sisu_token)?;
    let data = items
        .into_iter()
        .find(|item| item.resource.as_deref() == Some("rp://api.minecraftservices.com/"))
        .and_then(|item| item.data)
        .context("SISU token did not contain Minecraft Services data")?;
    let user_hash = data
        .display_claims
        .and_then(|claims| claims.xui)
        .and_then(|claims| claims.first().and_then(|claim| claim.uhs.clone()))
        .context("SISU token did not contain a user hash")?;
    let token = data
        .token
        .context("SISU token did not contain an Xbox token")?;
    Ok(format!("XBL3.0 x={user_hash};{token}"))
}

fn url_field(url: &Url, key: &str) -> Option<String> {
    url.query_pairs()
        .find(|(name, _)| name == key)
        .map(|(_, value)| value.into_owned())
        .or_else(|| {
            let fragment = url.fragment()?;
            let mut parsed = Url::parse("https://fragment.invalid/").ok()?;
            parsed.set_query(Some(fragment));
            parsed
                .query_pairs()
                .find(|(name, _)| name == key)
                .map(|(_, value)| value.into_owned())
        })
}

/// 判斷重新導向是否仍位於允許的 HTTPS 登入端點。
/// @param url 解析後的重新導向網址。
/// @return 僅 HTTPS、443 埠且主機符合允許清單時為 true。
fn allowed_auth_host(url: &Url) -> bool {
    // Cookie 僅能送往允許的 HTTPS 端點，重新導向不得降級為明文傳輸。
    if url.scheme() != "https" || url.port_or_known_default() != Some(443) {
        return false;
    }
    let Some(host) = url.host_str() else {
        return false;
    };
    host == "login.live.com"
        || host.ends_with(".login.live.com")
        || host == "sisu.xboxlive.com"
        || host == "www.minecraft.net"
}

fn decode_base64_json<T: for<'de> Deserialize<'de>>(value: &str) -> Result<T> {
    let bytes = URL_SAFE_NO_PAD
        .decode(value)
        .or_else(|_| URL_SAFE.decode(value))
        .or_else(|_| STANDARD.decode(value))
        .context("decode SISU token")?;
    serde_json::from_slice(&bytes).context("parse SISU token")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_cookie_header_from_last_two_columns() {
        let header = parse_cookie_header(
            ".live.com\tTRUE\t/\tTRUE\t0\tMSPAuth\tvalue\n# comment\n.live.com\tTRUE\t/\tTRUE\t0\tMSPProf\tsecond",
        )
        .unwrap();
        assert!(header.contains("MSPAuth=value"));
        assert!(header.contains("MSPProf=second"));
    }
}
