use super::UserRuntime;
use crate::{
    app_state::CommandResult,
    auth::{access_token, cookie, microsoft, ACCESS_TOKEN_EXPIRED, ACCESS_TOKEN_INVALID},
    bot_runtime::BotConfig,
    input_limits::{validate_length, MAX_BOT_ID_BYTES},
};
use chrono::{Duration, Utc};
use local_store::{AccountRecord, AuthKind};
use uuid::Uuid;

impl UserRuntime {
    pub(crate) async fn start_bot(&self, account_id: String) -> CommandResult<()> {
        validate_length(&account_id, "bot id", MAX_BOT_ID_BYTES)
            .map_err(|error| error.to_string())?;
        let mut account = self
            .with_store(move |store| {
                store
                    .account(&account_id)
                    .map_err(|error| error.to_string())?
                    .ok_or_else(|| "account not found".to_owned())
            })
            .await?;
        self.prepare_session(&mut account).await?;

        let profile_id = account
            .profile_id
            .as_deref()
            .ok_or_else(|| "Minecraft profile is missing".to_owned())?;
        let uuid = Uuid::parse_str(profile_id)
            .map_err(|_| "Minecraft profile UUID is invalid".to_owned())?;
        let access_token = active_access_token(&account)?;
        let config = BotConfig {
            bot_id: account.id,
            username: account.username,
            uuid,
            access_token,
            server_address: account.server_address,
        };
        self.bots
            .start(config)
            .await
            .map_err(|error| format!("start bot: {error:#}"))
    }

    pub(crate) async fn stop_bot(&self, id: String) -> CommandResult<()> {
        validate_length(&id, "bot id", MAX_BOT_ID_BYTES).map_err(|error| error.to_string())?;
        self.bots.stop(&id).await.map_err(|error| error.to_string())
    }

    async fn prepare_session(&self, account: &mut AccountRecord) -> CommandResult<()> {
        match account.auth_kind {
            AuthKind::Microsoft => self.prepare_microsoft(account).await,
            AuthKind::AccessToken => self.prepare_access_token(account).await,
            AuthKind::Cookie => self.prepare_cookie(account).await,
        }
    }

    async fn prepare_microsoft(&self, account: &mut AccountRecord) -> CommandResult<()> {
        if session_is_current(account) {
            return Ok(());
        }
        let session = microsoft::refresh(
            &self.http_client,
            account.credential.as_deref().unwrap_or_default(),
        )
        .await
        .map_err(|_| ACCESS_TOKEN_INVALID.to_owned())?;
        self.save_microsoft_session(account, session).await
    }

    async fn prepare_access_token(&self, account: &mut AccountRecord) -> CommandResult<()> {
        if account.session_token.is_some() {
            if session_is_current(account) {
                return Ok(());
            }
            let session = microsoft::refresh_legacy(
                &self.http_client,
                account.credential.as_deref().unwrap_or_default(),
            )
            .await
            .map_err(|_| ACCESS_TOKEN_INVALID.to_owned())?;
            return self.save_microsoft_session(account, session).await;
        }

        let expires_at = account
            .session_expires_at
            .or_else(|| account.credential.as_deref().and_then(access_token::expiry))
            .ok_or_else(|| ACCESS_TOKEN_INVALID.to_owned())?;
        if expires_at <= Utc::now() {
            return Err(ACCESS_TOKEN_EXPIRED.to_owned());
        }
        Ok(())
    }

    async fn prepare_cookie(&self, account: &mut AccountRecord) -> CommandResult<()> {
        if session_is_current(account) {
            return Ok(());
        }

        let session = cookie::exchange(account.credential.as_deref().unwrap_or_default())
            .await
            .map_err(|error| error.to_string())?;
        let account_id = account.id.clone();
        let username = session.username.clone();
        let uuid = session.uuid.clone();
        let access_token = session.access_token.clone();
        let expires_at = session.expires_at;
        self.with_store(move |store| {
            store
                .update_auth_session(
                    &account_id,
                    &username,
                    &uuid,
                    None,
                    &access_token,
                    Some(expires_at),
                )
                .map_err(|error| error.to_string())
        })
        .await?;
        account.username = session.username;
        account.profile_id = Some(session.uuid);
        account.session_token = Some(session.access_token);
        account.session_expires_at = Some(session.expires_at);
        Ok(())
    }

    async fn save_microsoft_session(
        &self,
        account: &mut AccountRecord,
        session: microsoft::MicrosoftSession,
    ) -> CommandResult<()> {
        let account_id = account.id.clone();
        let username = session.username.clone();
        let uuid = session.uuid.clone();
        let refresh_token = session.refresh_token.clone();
        let access_token = session.access_token.clone();
        let expires_at = session.expires_at;
        self.with_store(move |store| {
            store
                .update_auth_session(
                    &account_id,
                    &username,
                    &uuid,
                    Some(&refresh_token),
                    &access_token,
                    Some(expires_at),
                )
                .map_err(|error| error.to_string())
        })
        .await?;
        account.username = session.username;
        account.profile_id = Some(session.uuid);
        account.credential = Some(session.refresh_token);
        account.session_token = Some(session.access_token);
        account.session_expires_at = Some(session.expires_at);
        Ok(())
    }
}

fn active_access_token(account: &AccountRecord) -> CommandResult<String> {
    account
        .session_token
        .clone()
        .or_else(|| account.credential.clone())
        .filter(|token| !token.trim().is_empty())
        .ok_or_else(|| ACCESS_TOKEN_INVALID.to_owned())
}

fn session_is_current(account: &AccountRecord) -> bool {
    account.session_token.is_some()
        && account
            .session_expires_at
            .is_some_and(|expires_at| expires_at > Utc::now() + Duration::minutes(1))
}
