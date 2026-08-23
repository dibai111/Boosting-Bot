mod accounts;
mod bots;
mod matchmaking;
mod settings;
mod system;

use crate::{
    app_state::CommandResult,
    bot_runtime::{BotEvent, BotEventBus, BotRuntime},
    matchmaking::{MatchmakingDiagnostic, MatchmakingSession},
};
use local_store::Store;
use std::sync::{Arc, Mutex as StdMutex};
use tokio::{
    sync::{broadcast, watch},
    task,
};

pub(crate) struct UserRuntime {
    store: Arc<StdMutex<Store>>,
    pub(crate) bots: Arc<BotRuntime>,
    pub(crate) events: BotEventBus,
    http_client: reqwest::Client,
    matchmaking: Arc<MatchmakingSession>,
}

impl UserRuntime {
    pub(crate) fn new(
        store: Store,
        bots: Arc<BotRuntime>,
        events: BotEventBus,
        http_client: reqwest::Client,
        matchmaking: Arc<MatchmakingSession>,
    ) -> Self {
        Self {
            store: Arc::new(StdMutex::new(store)),
            bots,
            events,
            http_client,
            matchmaking,
        }
    }

    pub(crate) fn subscribe_bot_events(&self) -> broadcast::Receiver<BotEvent> {
        self.events.subscribe()
    }

    pub(crate) fn subscribe_matchmaking(
        &self,
    ) -> watch::Receiver<crate::matchmaking::MatchmakingSnapshot> {
        self.matchmaking.subscribe()
    }

    pub(crate) fn subscribe_matchmaking_diagnostics(
        &self,
    ) -> broadcast::Receiver<MatchmakingDiagnostic> {
        self.matchmaking.subscribe_diagnostics()
    }

    pub(crate) async fn handle_bot_event(&self, event: &BotEvent) {
        if let BotEvent::Profile {
            bot_id,
            username,
            uuid,
        } = event
        {
            let bot_id = bot_id.clone();
            let username = username.clone();
            let uuid = uuid.clone();
            let _ = self
                .with_store(move |store| {
                    store
                        .update_profile(&bot_id, &username, uuid.as_deref())
                        .map_err(|error| error.to_string())
                })
                .await;
        }
        self.matchmaking.handle_bot_event(event).await;
    }

    pub(crate) async fn shutdown(&self) {
        self.matchmaking.stop().await;
        self.bots.shutdown().await;
    }

    async fn with_store<T, F>(&self, operation: F) -> CommandResult<T>
    where
        T: Send + 'static,
        F: FnOnce(&Store) -> CommandResult<T> + Send + 'static,
    {
        let store = Arc::clone(&self.store);
        task::spawn_blocking(move || {
            let store = store
                .lock()
                .map_err(|_| "local store lock poisoned".to_owned())?;
            operation(&store)
        })
        .await
        .map_err(|error| format!("local store task failed: {error}"))?
    }
}
