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

//! 提供 Windows 本機帳號與設定儲存，資料經 DPAPI 加密後寫入目前使用者的 Registry。

mod accounts;
mod models;
mod protection;
mod registry;
mod settings;
mod state;

use anyhow::Result;
pub use models::{AccountRecord, AccountSession, AuthKind, CreateAccountInput, UserSettings};
use std::sync::{Arc, Mutex};

/// 目前 Windows 使用者的 DPAPI／Registry 儲存入口；讀改寫須由呼叫端序列化。
#[derive(Clone)]
pub struct Store {
    registry_path: String,
    state_lock: Arc<Mutex<()>>,
}

impl Store {
    /// 開啟既有儲存鍵，不變更已存資料。
    ///
    /// 非 Windows 或 Registry 開啟失敗時回傳錯誤。
    pub fn open() -> Result<Self> {
        let store = Self {
            registry_path: registry::DEFAULT_PATH.to_owned(),
            state_lock: Arc::new(Mutex::new(())),
        };
        registry::ensure_key(&store.registry_path)?;
        Ok(store)
    }

    #[cfg(test)]
    fn open_at(path: String) -> Result<Self> {
        let store = Self {
            registry_path: path,
            state_lock: Arc::new(Mutex::new(())),
        };
        registry::ensure_key(&store.registry_path)?;
        Ok(store)
    }
}

#[cfg(all(test, windows))]
mod tests {
    use super::{protection, registry, AuthKind, CreateAccountInput, Store};
    use chrono::{Duration, Utc};
    use uuid::Uuid;

    struct TestStore {
        store: Store,
        path: String,
    }

    impl TestStore {
        fn new() -> Self {
            let path = format!("Software\\BoostingBot\\Tests\\{}", Uuid::new_v4());
            let store = Store::open_at(path.clone()).expect("open test Registry store");
            Self { store, path }
        }
    }

    impl Drop for TestStore {
        fn drop(&mut self) {
            registry::delete_key(&self.path).expect("delete test Registry key");
        }
    }

    #[test]
    fn stores_accounts_and_settings() {
        let test = TestStore::new();
        let expires_at = Utc::now() + Duration::days(30);
        let account = test
            .store
            .create_account(
                CreateAccountInput {
                    username: "Cookie account".to_owned(),
                    auth_kind: AuthKind::Cookie,
                    credential: Some("cookie-data".to_owned()),
                    server_address: "play.example.net".to_owned(),
                },
                None,
                Some("session-token".to_owned()),
                Some(expires_at),
            )
            .expect("create account");
        test.store
            .save_user_settings([("botting-theme".to_owned(), "dark".to_owned())].into())
            .expect("save settings");
        let account_id = account.id.clone();
        let _account = test
            .store
            .account(&account_id)
            .expect("read account")
            .expect("account exists");
        let session = test
            .store
            .account_session(&account_id)
            .expect("read session")
            .expect("session exists");
        assert_eq!(session.secrets.credential.as_deref(), Some("cookie-data"));
        assert_eq!(
            session.secrets.session_token.as_deref(),
            Some("session-token")
        );
        let public_json =
            serde_json::to_string(&session.account).expect("serialize public account");
        assert!(!public_json.contains("cookie-data"));
        assert!(!public_json.contains("session-token"));
        assert_eq!(
            test.store.user_settings().expect("read settings")["botting-theme"],
            "dark"
        );
    }

    #[test]
    fn account_updates_reject_missing_accounts_and_invalid_profiles() {
        let test = TestStore::new();
        let missing = Uuid::new_v4().to_string();
        assert!(test
            .store
            .update_server_address(&missing, "play.example.net")
            .is_err());
        assert!(test
            .store
            .update_profile(&missing, "Example", None)
            .is_err());
        assert!(test
            .store
            .update_auth_session(
                &missing,
                "Example",
                "0123456789abcdef0123456789abcdef",
                None,
                "token",
                None
            )
            .is_err());

        let account = test
            .store
            .create_account(
                CreateAccountInput {
                    username: "Example".to_owned(),
                    auth_kind: AuthKind::Microsoft,
                    credential: Some("refresh-token".to_owned()),
                    server_address: "play.example.net".to_owned(),
                },
                Some("01234567-89ab-cdef-0123-456789abcdef".to_owned()),
                None,
                None,
            )
            .expect("create account");
        assert!(test
            .store
            .update_profile(&account.id, "Example", Some("not-a-uuid"))
            .is_err());
        assert!(test
            .store
            .update_auth_session(&account.id, "Example", "not-a-uuid", None, "token", None)
            .is_err());
        assert!(test.store.update_server_address(&account.id, "  ").is_err());
    }

    #[test]
    fn reads_legacy_flat_account_secrets() {
        let json = r#"{
            "version": 1,
            "accounts": [{
                "id": "account-id",
                "username": "Example",
                "profile_id": "01234567-89ab-cdef-0123-456789abcdef",
                "auth_kind": "microsoft",
                "credential": "refresh-token",
                "session_token": "access-token",
                "session_expires_at": null,
                "server_address": "play.example.net",
                "created_at": "2026-01-01T00:00:00Z",
                "updated_at": "2026-01-01T00:00:00Z",
                "credential_checked_at": null
            }],
            "settings": {}
        }"#;
        let state: super::state::PersistedState =
            serde_json::from_str(json).expect("decode existing flat account format");
        let secrets = &state.accounts[0].secrets;
        assert_eq!(secrets.credential.as_deref(), Some("refresh-token"));
        assert_eq!(secrets.session_token.as_deref(), Some("access-token"));
        let encoded = serde_json::to_value(state).expect("encode account state");
        let account = &encoded["accounts"][0];
        assert_eq!(account["credential"], "refresh-token");
        assert_eq!(account["session_token"], "access-token");
        assert!(account.get("secrets").is_none());
    }

    #[test]
    fn cloned_stores_serialize_read_modify_write_updates() {
        let test = TestStore::new();
        let account = test
            .store
            .create_account(
                CreateAccountInput {
                    username: "Example".to_owned(),
                    auth_kind: AuthKind::Cookie,
                    credential: Some("cookie-data".to_owned()),
                    server_address: "play.example.net".to_owned(),
                },
                None,
                Some("session-token".to_owned()),
                None,
            )
            .expect("create account");
        let first = test.store.clone();
        let second = test.store.clone();
        let first_id = account.id.clone();
        let second_id = account.id;
        let first_update = std::thread::spawn(move || {
            first.save_user_settings([("first".to_owned(), "1".to_owned())].into())
        });
        let second_update = std::thread::spawn(move || {
            second.update_server_address(&second_id, "other.example.net")
        });
        first_update
            .join()
            .expect("first update thread")
            .expect("save settings");
        second_update
            .join()
            .expect("second update thread")
            .expect("update address");
        let settings = test.store.user_settings().expect("read settings");
        assert_eq!(settings.get("first").map(String::as_str), Some("1"));
        assert_eq!(
            test.store
                .account(&first_id)
                .expect("read account")
                .expect("account exists")
                .server_address,
            "other.example.net"
        );
    }

    #[test]
    fn rejects_duplicate_minecraft_profile() {
        let test = TestStore::new();
        let input = || CreateAccountInput {
            username: "ExamplePlayer".to_owned(),
            auth_kind: AuthKind::Microsoft,
            credential: Some("refresh-token".to_owned()),
            server_address: "play.example.net".to_owned(),
        };
        test.store
            .create_account(
                input(),
                Some("01234567-89ab-cdef-0123-456789abcdef".to_owned()),
                None,
                None,
            )
            .expect("create first account");
        let error = test
            .store
            .create_account(
                input(),
                Some("0123456789ABCDEF0123456789ABCDEF".to_owned()),
                None,
                None,
            )
            .expect_err("duplicate profile should fail");
        assert!(error.to_string().contains("account_already_exists"));
    }

    #[test]
    fn tampered_dpapi_payload_is_rejected() {
        let mut protected = protection::protect(b"test-state").expect("protect payload");
        protected[0] ^= 0xff;
        assert!(protection::unprotect(&protected).is_err());
    }
}
