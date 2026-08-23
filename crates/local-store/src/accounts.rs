use crate::state::StoredAccount;
use crate::{AccountRecord, CreateAccountInput, Store};
use anyhow::{bail, Context, Result};
use chrono::{DateTime, Utc};
use uuid::Uuid;

const ACCOUNT_ALREADY_EXISTS: &str = "account_already_exists";

impl Store {
    pub fn list_accounts(&self) -> Result<Vec<AccountRecord>> {
        let mut accounts = self
            .read_state()?
            .accounts
            .into_iter()
            .map(AccountRecord::from)
            .collect::<Vec<_>>();
        accounts.sort_by_key(|account| account.created_at);
        Ok(accounts)
    }

    pub fn account_count(&self) -> Result<u32> {
        u32::try_from(self.read_state()?.accounts.len())
            .context("account count exceeds supported range")
    }

    pub fn account(&self, id: &str) -> Result<Option<AccountRecord>> {
        Ok(self
            .read_state()?
            .accounts
            .into_iter()
            .find(|account| account.id == id)
            .map(AccountRecord::from))
    }

    pub fn create_account(
        &self,
        input: CreateAccountInput,
        profile_id: Option<String>,
        session_token: Option<String>,
        session_expires_at: Option<DateTime<Utc>>,
    ) -> Result<AccountRecord> {
        self.update_state(|state| {
            if let Some(profile_id) = profile_id.as_deref() {
                let normalized_id = normalize_profile_id(profile_id);
                if state.accounts.iter().any(|account| {
                    account
                        .profile_id
                        .as_deref()
                        .is_some_and(|value| normalize_profile_id(value) == normalized_id)
                }) {
                    bail!(ACCOUNT_ALREADY_EXISTS);
                }
            }

            let now = Utc::now();
            let stored = StoredAccount {
                id: Uuid::new_v4().to_string(),
                username: input.username.trim().to_owned(),
                profile_id,
                auth_kind: input.auth_kind,
                credential: input.credential,
                session_token,
                session_expires_at,
                credential_checked_at: Some(now),
                server_address: input.server_address.trim().to_owned(),
                created_at: now,
                updated_at: now,
            };
            let record = AccountRecord::from(stored.clone());
            state.accounts.push(stored);
            Ok(record)
        })
    }

    pub fn delete_accounts(&self, ids: &[String]) -> Result<usize> {
        self.update_state(|state| {
            let before = state.accounts.len();
            state
                .accounts
                .retain(|account| !ids.iter().any(|id| id == &account.id));
            Ok(before - state.accounts.len())
        })
    }

    pub fn update_server_address(&self, id: &str, address: &str) -> Result<()> {
        self.update_state(|state| {
            if let Some(account) = state.accounts.iter_mut().find(|account| account.id == id) {
                account.server_address = address.trim().to_owned();
                account.updated_at = Utc::now();
            }
            Ok(())
        })
    }

    pub fn update_profile(&self, id: &str, username: &str, profile_id: Option<&str>) -> Result<()> {
        self.update_state(|state| {
            if let Some(account) = state.accounts.iter_mut().find(|account| account.id == id) {
                account.username = username.to_owned();
                if profile_id.is_some() {
                    account.profile_id = profile_id.map(str::to_owned);
                }
                account.updated_at = Utc::now();
                account.credential_checked_at = Some(account.updated_at);
            }
            Ok(())
        })
    }

    pub fn update_auth_session(
        &self,
        id: &str,
        username: &str,
        profile_id: &str,
        credential: Option<&str>,
        session_token: &str,
        expires_at: Option<DateTime<Utc>>,
    ) -> Result<()> {
        self.update_state(|state| {
            if let Some(account) = state.accounts.iter_mut().find(|account| account.id == id) {
                account.username = username.to_owned();
                account.profile_id = Some(profile_id.to_owned());
                if let Some(credential) = credential {
                    account.credential = Some(credential.to_owned());
                }
                account.session_token = Some(session_token.to_owned());
                account.session_expires_at = expires_at;
                account.updated_at = Utc::now();
                account.credential_checked_at = Some(account.updated_at);
            }
            Ok(())
        })
    }
}

impl From<StoredAccount> for AccountRecord {
    fn from(account: StoredAccount) -> Self {
        Self {
            id: account.id,
            username: account.username,
            profile_id: account.profile_id,
            auth_kind: account.auth_kind,
            credential: account.credential,
            session_token: account.session_token,
            session_expires_at: account.session_expires_at,
            credential_checked_at: account.credential_checked_at,
            server_address: account.server_address,
            created_at: account.created_at,
            updated_at: account.updated_at,
        }
    }
}

fn normalize_profile_id(profile_id: &str) -> String {
    profile_id
        .trim()
        .chars()
        .filter(|character| *character != '-')
        .flat_map(char::to_lowercase)
        .collect()
}
