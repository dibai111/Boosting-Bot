use azalea::{
    account::AccountTrait,
    auth::{
        certs::Certificates,
        sessionserver::{self, ClientSessionServerError, SessionServerJoinOpts},
    },
};
use std::{fmt, future::Future, pin::Pin, sync::Mutex};
use uuid::Uuid;

pub(super) struct MinecraftAccessTokenAccount {
    username: String,
    uuid: Uuid,
    access_token: String,
    certificates: Mutex<Option<Certificates>>,
}

impl MinecraftAccessTokenAccount {
    pub(super) fn new(username: String, uuid: Uuid, access_token: String) -> Self {
        Self {
            username,
            uuid,
            access_token,
            certificates: Mutex::new(None),
        }
    }
}

impl fmt::Debug for MinecraftAccessTokenAccount {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("MinecraftAccessTokenAccount")
            .field("username", &self.username)
            .field("uuid", &self.uuid)
            .field("access_token", &"[redacted]")
            .finish()
    }
}

impl AccountTrait for MinecraftAccessTokenAccount {
    fn username(&self) -> &str {
        &self.username
    }

    fn uuid(&self) -> Uuid {
        self.uuid
    }

    fn access_token(&self) -> Option<String> {
        Some(self.access_token.clone())
    }

    fn certs(&self) -> Option<Certificates> {
        self.certificates
            .lock()
            .ok()
            .and_then(|certificates| certificates.clone())
    }

    fn set_certs(&self, certificates: Certificates) {
        if let Ok(mut stored) = self.certificates.lock() {
            *stored = Some(certificates);
        }
    }

    fn join<'a>(
        &'a self,
        public_key: &'a [u8],
        private_key: &'a [u8; 16],
        server_id: &'a str,
        proxy: Option<reqwest::Proxy>,
    ) -> Pin<Box<dyn Future<Output = Result<(), ClientSessionServerError>> + Send + 'a>> {
        Box::pin(async move {
            sessionserver::join(SessionServerJoinOpts {
                access_token: &self.access_token,
                public_key,
                private_key,
                uuid: &self.uuid,
                server_id,
                proxy,
            })
            .await
        })
    }
}
