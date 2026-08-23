use base64::engine::general_purpose::{URL_SAFE, URL_SAFE_NO_PAD};
use base64::Engine;
use chrono::{DateTime, Utc};
use serde::Deserialize;

#[derive(Deserialize)]
struct TokenClaims {
    exp: i64,
}

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
