use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub type UserSettings = BTreeMap<String, String>;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AuthKind {
    Microsoft,
    AccessToken,
    Cookie,
}

impl AuthKind {
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
pub struct CreateAccountInput {
    pub username: String,
    pub auth_kind: AuthKind,
    pub credential: Option<String>,
    pub server_address: String,
}
