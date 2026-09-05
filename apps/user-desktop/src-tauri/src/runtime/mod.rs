mod accounts;
mod bots;
mod matchmaking;
mod nick;
mod settings;
mod system;

use crate::{
    app_state::CommandResult,
    bot_runtime::{BotEvent, BotEventBus, BotRuntime, RuntimeMode},
    matchmaking::{MatchmakingDiagnostic, MatchmakingSession},
};
use local_store::Store;
use std::{
    collections::HashSet,
    sync::{Arc, Mutex as StdMutex},
};
use tokio::{
    sync::{broadcast, watch, Mutex},
    task,
};

pub(crate) use nick::StartNickRollerInput;

pub(crate) struct UserRuntime {
    store: Arc<StdMutex<Store>>,
    pub(crate) bots: Arc<BotRuntime>,
    pub(crate) events: BotEventBus,
    http_client: reqwest::Client,
    matchmaking: Arc<MatchmakingSession>,
    active_mode: Mutex<RuntimeMode>,
    nick_bot_ids: Mutex<HashSet<String>>,
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
            active_mode: Mutex::new(RuntimeMode::Idle),
            nick_bot_ids: Mutex::new(HashSet::new()),
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
            let stored_bot_id = bot_id.clone();
            let username = username.clone();
            let uuid = uuid.clone();
            if let Err(error) = self
                .with_store(move |store| {
                    store
                        .update_profile(&stored_bot_id, &username, uuid.as_deref())
                        .map_err(|error| error.to_string())
                })
                .await
            {
                self.events.publish(BotEvent::Error {
                    bot_id: Some(bot_id.clone()),
                    code: "profile_persist_failed".to_owned(),
                    message: format!("save bot profile: {error}"),
                });
            }
        }
        let previous_matchmaking_phase = self.matchmaking.phase().await;
        self.matchmaking.handle_bot_event(event).await;
        let matching_done = if is_matchmaking_lifecycle_event(event)
            && !matches!(
                previous_matchmaking_phase,
                crate::matchmaking::MatchmakingPhase::Idle
                    | crate::matchmaking::MatchmakingPhase::Failed
            ) {
            let mode = *self.active_mode.lock().await;
            mode == RuntimeMode::Matching
                && matches!(
                    self.matchmaking.phase().await,
                    crate::matchmaking::MatchmakingPhase::Idle
                        | crate::matchmaking::MatchmakingPhase::Failed
                )
        } else {
            false
        };
        if matching_done {
            self.leave_mode(RuntimeMode::Matching).await;
        }
        if let BotEvent::NickRollerState { bot_id, phase, .. } = event {
            if phase.is_terminal() {
                let should_leave = {
                    let mut bot_ids = self.nick_bot_ids.lock().await;
                    bot_ids.remove(bot_id);
                    bot_ids.is_empty()
                };
                if should_leave {
                    self.leave_mode(RuntimeMode::NickRoller).await;
                }
            }
        }
    }

    pub(crate) async fn shutdown(&self) {
        self.matchmaking.stop().await;
        self.leave_mode(RuntimeMode::Matching).await;
        let _ = self.stop_nick_roller().await;
        self.bots.shutdown().await;
    }

    pub(crate) async fn active_mode(&self) -> RuntimeMode {
        self.reconcile_active_mode().await;
        *self.active_mode.lock().await
    }

    pub(crate) async fn enter_mode(&self, requested: RuntimeMode) -> CommandResult<()> {
        let mut mode = self.active_mode.lock().await;
        if *mode != RuntimeMode::Idle {
            return Err(mode_conflict_message(*mode));
        }
        *mode = requested;
        drop(mode);
        self.events
            .publish(BotEvent::RuntimeMode { mode: requested });
        Ok(())
    }

    pub(crate) async fn leave_mode(&self, mode_to_leave: RuntimeMode) {
        let changed = {
            let mut mode = self.active_mode.lock().await;
            if *mode == mode_to_leave {
                *mode = RuntimeMode::Idle;
                true
            } else {
                false
            }
        };
        if changed {
            self.events.publish(BotEvent::RuntimeMode {
                mode: RuntimeMode::Idle,
            });
        }
    }

    pub(crate) async fn reconcile_active_mode(&self) {
        let mode = *self.active_mode.lock().await;
        match mode {
            RuntimeMode::Matching => {
                let phase = self.matchmaking.phase().await;
                if matches!(
                    phase,
                    crate::matchmaking::MatchmakingPhase::Idle
                        | crate::matchmaking::MatchmakingPhase::Failed
                ) {
                    self.leave_mode(RuntimeMode::Matching).await;
                }
            }
            RuntimeMode::NickRoller => {
                if self.nick_bot_ids.lock().await.is_empty() {
                    self.leave_mode(RuntimeMode::NickRoller).await;
                }
            }
            RuntimeMode::Idle => {}
        }
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

fn is_matchmaking_lifecycle_event(event: &BotEvent) -> bool {
    matches!(
        event,
        BotEvent::MatchAttemptResult { .. }
            | BotEvent::MatchAttemptFailed { .. }
            | BotEvent::MatchRetryReady { .. }
            | BotEvent::MatchRetryPreparationFailed { .. }
            | BotEvent::QueueProgress { .. }
            | BotEvent::DuelPitchObserved { .. }
            | BotEvent::BotGameState { .. }
            | BotEvent::Status {
                phase: crate::bot_runtime::BotPhase::Offline,
                ..
            }
            | BotEvent::Error { .. }
    )
}

fn mode_conflict_message(mode: RuntimeMode) -> String {
    match mode {
        RuntimeMode::Matching => {
            "Matching is currently active; stop it before starting Nick Roller".to_owned()
        }
        RuntimeMode::NickRoller => {
            "Nick Roller is currently active; stop it before starting Matching".to_owned()
        }
        RuntimeMode::Idle => "another user operation is currently active".to_owned(),
    }
}
