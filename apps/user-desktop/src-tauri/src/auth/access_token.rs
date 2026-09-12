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

//! 讀取 JWT 的到期時間；此處只解碼宣告，權杖有效性仍須由服務端驗證。

use base64::engine::general_purpose::{URL_SAFE, URL_SAFE_NO_PAD};
use base64::Engine;
use chrono::{DateTime, Utc};
use serde::Deserialize;

#[derive(Deserialize)]
struct TokenClaims {
    exp: i64,
}

/// 解析 JWT exp 宣告；不驗證簽章，登入有效性仍由服務端判定。
/// @param token 可能含首尾空白的 JWT。
/// @return UTC 到期時間；格式、編碼或時間戳無效時為 None。
pub fn expiry(token: &str) -> Option<DateTime<Utc>> {
    let payload = token.trim().split('.').nth(1)?;
    let bytes = URL_SAFE_NO_PAD
        .decode(payload)
        .or_else(|_| URL_SAFE.decode(payload))
        .ok()?;
    let claims: TokenClaims = serde_json::from_slice(&bytes).ok()?;
    DateTime::from_timestamp(claims.exp, 0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_expiry_from_jwt_payload() {
        let token = "header.eyJleHAiOjE3ODQ5OTE4MDR9.signature";
        assert_eq!(
            expiry(token).map(|value| value.timestamp()),
            Some(1_784_991_804)
        );
    }

    #[test]
    fn rejects_malformed_token() {
        assert_eq!(expiry("not-a-jwt"), None);
    }
}
