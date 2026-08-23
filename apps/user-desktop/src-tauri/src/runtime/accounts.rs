use super::UserRuntime;
use crate::{
    app_state::CommandResult,
    auth::{access_token, cookie, microsoft, profile, ACCESS_TOKEN_EXPIRED, ACCESS_TOKEN_INVALID},
    bot_runtime::BotEvent,
    input_limits::{
        validate_length, MAX_BOT_ID_BYTES, MAX_CREDENTIAL_BYTES, MAX_SERVER_ADDRESS_BYTES,
        MAX_USERNAME_BYTES,
    },
};
use chrono::Utc;
use local_store::{AccountRecord, AuthKind, CreateAccountInput};
use uuid::Uuid;

impl UserRuntime {
    pub(crate) async fn list_accounts(&self) -> CommandResult<Vec<AccountRecord>> {
        let mut accounts = self
            .with_store(|store| store.list_accounts().map_err(|error| error.to_string()))
            .await?;
        for account in &mut accounts {
            if matches!(account.auth_kind, AuthKind::AccessToken)
                && account.session_expires_at.is_none()
            {
                account.session_expires_at =
                    account.credential.as_deref().and_then(access_token::expiry);
            }
        }
        Ok(accounts)
    }

    pub(crate) async fn add_account(
        &self,
        mut input: CreateAccountInput,
    ) -> CommandResult<AccountRecord> {
        validate_input(&input)?;

        let (profile_id, session_token, session_expires_at) = match input.auth_kind {
            AuthKind::Microsoft => {
                let request_id = Uuid::new_v4().to_string();
                let device_code = microsoft::request_device_code(&self.http_client)
                    .await
                    .map_err(|error| error.to_string())?;
                self.events.publish(BotEvent::DeviceCode {
                    request_id: request_id.clone(),
                    user_code: device_code.user_code.clone(),
                    verification_uri: device_code.verification_uri.clone(),
                    expires_in: Some(device_code.expires_in),
                });

                let result = microsoft::complete_device_login(&self.http_client, device_code).await;
                match result {
                    Ok(session) => {
                        self.events.publish(BotEvent::MicrosoftAuthResult {
                            request_id,
                            username: Some(session.username.clone()),
                            uuid: Some(session.uuid.clone()),
                            error: None,
                        });
                        input.username = session.username;
                        input.credential = Some(session.refresh_token);
                        (
                            Some(session.uuid),
                            Some(session.access_token),
                            Some(session.expires_at),
                        )
                    }
                    Err(error) => {
                        let message = error.to_string();
                        self.events.publish(BotEvent::MicrosoftAuthResult {
                            request_id,
                            username: None,
                            uuid: None,
                            error: Some(message.clone()),
                        });
                        return Err(message);
                    }
                }
            }
            AuthKind::AccessToken => {
                let token = input
                    .credential
                    .as_deref()
                    .unwrap_or_default()
                    .trim()
                    .to_owned();
                if let Some(expires_at) = access_token::expiry(&token) {
                    if expires_at <= Utc::now() {
                        return Err(ACCESS_TOKEN_EXPIRED.to_owned());
                    }
                    let profile = profile::fetch(&self.http_client, &token)
                        .await
                        .map_err(|_| ACCESS_TOKEN_INVALID.to_owned())?;
                    input.username = profile.name;
                    input.credential = Some(token);
                    (Some(profile.id), None, Some(expires_at))
                } else {
                    let session = microsoft::refresh_legacy(&self.http_client, &token)
                        .await
                        .map_err(|_| ACCESS_TOKEN_INVALID.to_owned())?;
                    input.username = session.username;
                    input.credential = Some(session.refresh_token);
                    (
                        Some(session.uuid),
                        Some(session.access_token),
                        Some(session.expires_at),
                    )
                }
            }
            AuthKind::Cookie => {
                let session = cookie::exchange(input.credential.as_deref().unwrap_or_default())
                    .await
                    .map_err(|error| error.to_string())?;
                input.username = session.username;
                (
                    Some(session.uuid),
                    Some(session.access_token),
                    Some(session.expires_at),
                )
            }
        };

        self.with_store(move |store| {
            store
                .create_account(input, profile_id, session_token, session_expires_at)
                .map_err(|error| error.to_string())
        })
        .await
    }

    pub(crate) async fn delete_accounts(&self, ids: Vec<String>) -> CommandResult<usize> {
        for id in &ids {
            validate_length(id, "account id", MAX_BOT_ID_BYTES)
                .map_err(|error| error.to_string())?;
        }
        for id in &ids {
            self.bots
                .remove(id)
                .await
                .map_err(|error| format!("remove bot {id}: {error}"))?;
        }

        self.with_store(move |store| {
            store
                .delete_accounts(&ids)
                .map_err(|error| error.to_string())
        })
        .await
    }

    pub(crate) async fn update_server_address(
        &self,
        id: String,
        server_address: String,
    ) -> CommandResult<()> {
        validate_length(&id, "account id", MAX_BOT_ID_BYTES).map_err(|error| error.to_string())?;
        validate_length(&server_address, "server address", MAX_SERVER_ADDRESS_BYTES)
            .map_err(|error| error.to_string())?;
        if server_address.trim().is_empty() {
            return Err("server address is required".to_owned());
        }
        self.with_store(move |store| {
            store
                .update_server_address(&id, &server_address)
                .map_err(|error| error.to_string())
        })
        .await
    }
}

fn validate_input(input: &CreateAccountInput) -> CommandResult<()> {
    validate_length(&input.username, "username", MAX_USERNAME_BYTES)
        .map_err(|error| error.to_string())?;
    validate_length(
        &input.server_address,
        "server address",
        MAX_SERVER_ADDRESS_BYTES,
    )
    .map_err(|error| error.to_string())?;
    if let Some(credential) = input.credential.as_deref() {
        validate_length(credential, "login credential", MAX_CREDENTIAL_BYTES)
            .map_err(|error| error.to_string())?;
    }
    if input.server_address.trim().is_empty() {
        return Err("server address is required".to_owned());
    }
    if matches!(input.auth_kind, AuthKind::AccessToken | AuthKind::Cookie)
        && input
            .credential
            .as_deref()
            .unwrap_or_default()
            .trim()
            .is_empty()
    {
        return Err("login credential is required".to_owned());
    }
    Ok(())
}
