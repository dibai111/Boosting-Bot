mod account;
mod afk;
mod detection;
mod engine;
mod events;
mod model;
mod nick_roller;
mod session;

use anyhow::{bail, Context, Result};
use std::{
    collections::HashMap,
    sync::{Arc, Mutex, Weak},
};

pub(crate) use events::BotEventBus;
pub(crate) use model::{
    BotCommand, BotConfig, BotEvent, BotGamePhase, BotPhase, DuelPitchDirection, GameKind,
    GameMode, MatchAttempt, RuntimeMode,
};
pub(crate) use nick_roller::NickRollerConfig;

pub(crate) struct BotRuntime {
    inner: Arc<RuntimeInner>,
}

struct RuntimeInner {
    state: Mutex<RuntimeState>,
    events: BotEventBus,
}

#[derive(Default)]
struct RuntimeState {
    generations: HashMap<String, u64>,
    sessions: HashMap<String, engine::SessionHandle>,
    shutting_down: bool,
}

#[derive(Clone)]
pub(super) struct SessionEmitter {
    bot_id: String,
    generation: u64,
    runtime: Weak<RuntimeInner>,
}

impl SessionEmitter {
    pub(super) fn bot_id(&self) -> &str {
        &self.bot_id
    }

    pub(super) fn publish(&self, event: BotEvent) {
        let Some(runtime) = self.runtime.upgrade() else {
            return;
        };
        let is_current = runtime
            .state
            .lock()
            .ok()
            .and_then(|state| state.generations.get(&self.bot_id).copied())
            == Some(self.generation);
        if is_current {
            runtime.events.publish(event);
        }
    }
}

impl BotRuntime {
    pub(crate) fn new(events: BotEventBus) -> Self {
        Self {
            inner: Arc::new(RuntimeInner {
                state: Mutex::new(RuntimeState::default()),
                events,
            }),
        }
    }

    pub(crate) async fn start(&self, config: BotConfig) -> Result<()> {
        let bot_id = config.bot_id.clone();
        let (generation, previous) = {
            let mut state = self.lock_state()?;
            if state.shutting_down {
                bail!("bot runtime is shutting down");
            }
            let generation = state
                .generations
                .get(&bot_id)
                .copied()
                .unwrap_or_default()
                .wrapping_add(1);
            state.generations.insert(bot_id.clone(), generation);
            (generation, state.sessions.remove(&bot_id))
        };

        if let Some(previous) = previous {
            previous.stop("Restarting").await;
        }

        let emitter = SessionEmitter {
            bot_id: bot_id.clone(),
            generation,
            runtime: Arc::downgrade(&self.inner),
        };
        emitter.publish(BotEvent::Status {
            bot_id: bot_id.clone(),
            phase: BotPhase::Starting,
            message: None,
        });

        let mut handle = match engine::spawn(config, generation, emitter.clone()) {
            Ok(handle) => Some(handle),
            Err(error) => {
                let message = format!("start bot session: {error:#}");
                emitter.publish(BotEvent::Error {
                    bot_id: Some(bot_id.clone()),
                    code: "start_failed".to_owned(),
                    message: message.clone(),
                });
                emitter.publish(BotEvent::Status {
                    bot_id,
                    phase: BotPhase::Offline,
                    message: Some(message),
                });
                return Err(error);
            }
        };
        let rejected = {
            let mut state = self.lock_state()?;
            if state.shutting_down || state.generations.get(&bot_id).copied() != Some(generation) {
                true
            } else if let Some(handle) = handle.take() {
                state.sessions.insert(bot_id, handle);
                false
            } else {
                true
            }
        };
        if rejected {
            if let Some(handle) = handle {
                handle.stop("Bot start was superseded").await;
            }
            bail!("bot start was superseded");
        }
        Ok(())
    }

    pub(crate) async fn stop(&self, bot_id: &str) -> Result<()> {
        self.stop_with_reason(bot_id, "Stopped from Botting").await
    }

    pub(crate) async fn remove(&self, bot_id: &str) -> Result<()> {
        self.stop_with_reason(bot_id, "Account removed").await
    }

    pub(crate) async fn command(&self, bot_id: &str, command: BotCommand) -> Result<()> {
        let sender = {
            let state = self.lock_state()?;
            state
                .sessions
                .get(bot_id)
                .map(|session| session.commands.clone())
                .context("bot is not running")?
        };
        sender
            .send(session::SessionCommand::Command(command))
            .await
            .context("bot session command channel closed")
    }

    pub(crate) async fn shutdown(&self) {
        let sessions = {
            let Ok(mut state) = self.inner.state.lock() else {
                return;
            };
            if state.shutting_down {
                return;
            }
            state.shutting_down = true;
            state
                .sessions
                .drain()
                .map(|(_, session)| session)
                .collect::<Vec<_>>()
        };
        for session in sessions {
            session.stop("Botting is closing").await;
        }
    }

    async fn stop_with_reason(&self, bot_id: &str, reason: &str) -> Result<()> {
        let session = self.lock_state()?.sessions.remove(bot_id);
        let Some(session) = session else {
            return Ok(());
        };
        let emitter = SessionEmitter {
            bot_id: bot_id.to_owned(),
            generation: session.generation,
            runtime: Arc::downgrade(&self.inner),
        };
        emitter.publish(BotEvent::Status {
            bot_id: bot_id.to_owned(),
            phase: BotPhase::Stopping,
            message: None,
        });
        session.stop(reason).await;
        Ok(())
    }

    fn lock_state(&self) -> Result<std::sync::MutexGuard<'_, RuntimeState>> {
        self.inner
            .state
            .lock()
            .map_err(|_| anyhow::anyhow!("bot runtime state lock poisoned"))
    }
}
