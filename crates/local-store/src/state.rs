use crate::{protection, registry, Store};
use anyhow::{bail, Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

const STATE_VERSION: u32 = 1;

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct PersistedState {
    version: u32,
    #[serde(default)]
    pub(crate) accounts: Vec<StoredAccount>,
    #[serde(default)]
    pub(crate) settings: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct StoredAccount {
    pub id: String,
    pub username: String,
    pub profile_id: Option<String>,
    pub auth_kind: crate::AuthKind,
    pub credential: Option<String>,
    pub session_token: Option<String>,
    pub session_expires_at: Option<DateTime<Utc>>,
    pub server_address: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub credential_checked_at: Option<DateTime<Utc>>,
}

impl Default for PersistedState {
    fn default() -> Self {
        Self {
            version: STATE_VERSION,
            accounts: Vec::new(),
            settings: BTreeMap::new(),
        }
    }
}

impl PersistedState {
    fn validate(&self) -> Result<()> {
        if self.version != STATE_VERSION {
            bail!("unsupported local store version: {}", self.version)
        }
        Ok(())
    }
}

impl Store {
    pub(crate) fn read_state(&self) -> Result<PersistedState> {
        let Some(protected) = registry::read_value(&self.registry_path)? else {
            return Ok(PersistedState::default());
        };
        let plaintext = protection::unprotect(&protected)?;
        if plaintext.len() > registry::MAX_VALUE_BYTES {
            bail!("decrypted local state is larger than the supported limit")
        }
        let state: PersistedState =
            serde_json::from_slice(&plaintext).context("decode encrypted local state")?;
        state.validate()?;
        Ok(state)
    }

    pub(crate) fn write_state(&self, state: &PersistedState) -> Result<()> {
        state.validate()?;
        let plaintext = serde_json::to_vec(state).context("encode local state")?;
        if plaintext.len() > registry::MAX_VALUE_BYTES {
            bail!("local state is larger than the supported limit")
        }
        let protected = protection::protect(&plaintext)?;
        registry::write_value(&self.registry_path, &protected)
    }

    pub(crate) fn update_state<T, F>(&self, update: F) -> Result<T>
    where
        F: FnOnce(&mut PersistedState) -> Result<T>,
    {
        let mut state = self.read_state()?;
        let result = update(&mut state)?;
        self.write_state(&state)?;
        Ok(result)
    }
}
