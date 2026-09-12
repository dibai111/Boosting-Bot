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

//! 協調登入、儲存與互斥工作模式；同步儲存操作移至 blocking worker 執行。

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

/// 協調儲存、登入、Bot 與互斥工作模式的應用服務。
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
    /// 組合應用依賴，初始工作模式為 Idle。
    /// @param store 目前使用者的本機儲存。
    /// @param bots 共用 Bot runtime。
    /// @param events Bot 事件廣播匯流排。
    /// @param http_client 登入流程共用的 HTTP client。
    /// @param matchmaking 多 Bot 配對協調器。
    /// @return 持有所有應用服務的 runtime。
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

    /// 訂閱之後發布的 Bot 事件。
    /// @return 有界廣播接收端；落後時需處理 Lagged。
    pub(crate) fn subscribe_bot_events(&self) -> broadcast::Receiver<BotEvent> {
        self.events.subscribe()
    }

    /// 訂閱目前與後續配對快照。
    /// @return 保留最新配對狀態的 watch 接收端。
    pub(crate) fn subscribe_matchmaking(
        &self,
    ) -> watch::Receiver<crate::matchmaking::MatchmakingSnapshot> {
        self.matchmaking.subscribe()
    }

    /// 訂閱配對診斷事件。
    /// @return 有界診斷廣播接收端。
    pub(crate) fn subscribe_matchmaking_diagnostics(
        &self,
    ) -> broadcast::Receiver<MatchmakingDiagnostic> {
        self.matchmaking.subscribe_diagnostics()
    }

    /// 同步玩家資料並將事件交給配對與工作模式追蹤。
    /// @param event 目前連線 generation 所發布的 Bot 事件。
    /// @return 無回傳值；儲存失敗會發布錯誤事件。
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

    /// 停止配對、Nick 篩選及所有 Bot 工作階段。
    /// @return 所有停止流程返回後完成。
    pub(crate) async fn shutdown(&self) {
        self.matchmaking.stop().await;
        self.leave_mode(RuntimeMode::Matching).await;
        let _ = self.stop_nick_roller().await;
        self.bots.shutdown().await;
    }

    /// 先對照實際狀態，再取得目前工作模式。
    /// @return Idle、Matching 或 NickRoller。
    pub(crate) async fn active_mode(&self) -> RuntimeMode {
        self.reconcile_active_mode().await;
        *self.active_mode.lock().await
    }

    /// 在 Idle 時宣告工作模式並發布模式事件。
    /// @param requested 要進入的工作模式。
    /// @return 成功或既有模式衝突錯誤；實際啟動由呼叫端完成。
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

    /// 只退出指定的目前模式，避免清除另一種模式。
    /// @param mode_to_leave 預期要結束的模式。
    /// @return 無回傳值；模式有變更才發布事件。
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

    /// 對照配對階段與 Nick 追蹤集合，釋放已結束模式。
    /// @return 無回傳值；必要時將模式改為 Idle。
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

    // 鎖住整次讀改寫，並移到 blocking worker，避免 Registry 操作阻塞非同步執行緒。
    /// 在 blocking worker 內持鎖執行完整儲存操作。
    /// @param operation 讀取或更新 Store 的閉包，須擁有跨執行緒所需資料。
    /// @return 閉包結果；鎖中毒或 worker 失敗時回傳錯誤。
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
