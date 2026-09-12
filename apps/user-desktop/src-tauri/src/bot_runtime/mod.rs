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

//! 管理每個 Bot 的獨立工作階段；generation 用來隔離重新啟動前的舊事件。

mod account;
mod afk;
mod detection;
mod engine;
mod events;
mod match_controller;
mod model;
mod nick_roller;
mod nick_rules;
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

/// 持有每個帳號的獨立連線，透過 generation 隔離重新啟動前的事件。
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
/// 使用弱參照及連線 generation 過濾舊工作階段事件。
pub(super) struct SessionEmitter {
    bot_id: String,
    generation: u64,
    runtime: Weak<RuntimeInner>,
}

impl SessionEmitter {
    /// 取得事件發送器所屬帳號。
    /// @return 本機 Bot ID 的借用字串。
    pub(super) fn bot_id(&self) -> &str {
        &self.bot_id
    }

    /// 只有事件仍屬目前連線 generation 時才廣播。
    /// @param event 此 Bot 工作階段產生的事件。
    /// @return 無回傳值；runtime 已釋放或事件過期時丟棄。
    pub(super) fn publish(&self, event: BotEvent) {
        let Some(runtime) = self.runtime.upgrade() else {
            return;
        };
        // 舊執行緒可能晚於重新啟動結束；只轉送目前 generation 的事件。
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
    /// 建立空的 Bot runtime。
    /// @param events 提供給每個工作階段的事件匯流排。
    /// @return 可接收啟動要求的 runtime。
    pub(crate) fn new(events: BotEventBus) -> Self {
        Self {
            inner: Arc::new(RuntimeInner {
                state: Mutex::new(RuntimeState::default()),
                events,
            }),
        }
    }

    /// 取代指定 Bot 的舊連線，並拒絕被較新啟動要求取代的結果。
    /// @param config 已驗證的帳號、權杖及伺服器設定。
    /// @return 工作階段建立結果；連線狀態另由事件發布。
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

    /// 以使用者停止原因關閉 Bot。
    /// @param bot_id 本機帳號 ID。
    /// @return 停止結果；不存在的工作階段視為成功。
    pub(crate) async fn stop(&self, bot_id: &str) -> Result<()> {
        self.stop_with_reason(bot_id, "Stopped from Botting").await
    }

    /// 以帳號移除原因關閉 Bot。
    /// @param bot_id 即將刪除的本機帳號 ID。
    /// @return 停止結果。
    pub(crate) async fn remove(&self, bot_id: &str) -> Result<()> {
        self.stop_with_reason(bot_id, "Account removed").await
    }

    /// 複製目前指令通道後釋放同步鎖，再等待通道容量。
    /// @param bot_id 要接收指令的 Bot ID。
    /// @param command 配對、聊天或 Nick 業務指令。
    /// @return 指令入列結果，不代表服務端已執行。
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

    /// 拒絕新連線並依序等待所有工作階段停止。
    /// @return 已接管的工作階段全部返回後完成。
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
