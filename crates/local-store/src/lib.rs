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
pub use models::{AccountRecord, AuthKind, CreateAccountInput, UserSettings};

/// 目前 Windows 使用者的 DPAPI／Registry 儲存入口；讀改寫須由呼叫端序列化。
pub struct Store {
    registry_path: String,
}

impl Store {
    /// 開啟既有儲存鍵，不變更已存資料。
    /// @return 可用的 Store；非 Windows 或 Registry 開啟失敗時回傳錯誤。
    pub fn open() -> Result<Self> {
        let store = Self {
            registry_path: registry::DEFAULT_PATH.to_owned(),
        };
        registry::ensure_key(&store.registry_path)?;
        Ok(store)
    }

    #[cfg(test)]
    fn open_at(path: String) -> Result<Self> {
        let store = Self {
            registry_path: path,
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
        let account = test
            .store
            .account(&account.id)
            .expect("read account")
            .expect("account exists");
        assert_eq!(account.credential.as_deref(), Some("cookie-data"));
        assert_eq!(
            test.store.user_settings().expect("read settings")["botting-theme"],
            "dark"
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
