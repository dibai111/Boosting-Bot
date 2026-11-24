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
mod work_mode;

use crate::{
    app_state::CommandResult,
    bot_runtime::{BotEvent, BotEventBus, BotRuntime, RuntimeMode, RuntimeModeSnapshot},
    matchmaking::{MatchmakingDiagnostic, MatchmakingSession},
};
use local_store::Store;
use std::sync::{Arc, Mutex as StdMutex};
use tokio::{
    sync::{broadcast, mpsc, watch, Mutex},
    task,
};

pub(crate) use nick::StartNickRollerInput;

/// 協調儲存、登入、Bot 與互斥工作模式的應用服務。
pub(crate) struct UserRuntime {
    store: Arc<StdMutex<Store>>,
    bots: Arc<BotRuntime>,
    events: BotEventBus,
    http_client: reqwest::Client,
    matchmaking: Arc<MatchmakingSession>,
    work: Mutex<work_mode::WorkState>,
}

impl UserRuntime {
    /// 組合應用依賴，初始工作模式為 Idle。
    ///
    /// # 參數
    ///
    /// - `store` 目前使用者的本機儲存。
    /// - `bots` 共用 Bot runtime。
    /// - `events` Bot 業務輸入與處理後通知佇列。
    /// - `http_client` 登入流程共用的 HTTP client。
    /// - `matchmaking` 多 Bot 配對協調器。
    ///
    /// # 回傳
    ///
    /// 已啟動唯一業務消費任務的共用 runtime。
    pub(crate) fn new(
        store: Store,
        bots: Arc<BotRuntime>,
        events: BotEventBus,
        http_client: reqwest::Client,
        matchmaking: Arc<MatchmakingSession>,
    ) -> Arc<Self> {
        let runtime = Arc::new(Self {
            store: Arc::new(StdMutex::new(store)),
            bots,
            events,
            http_client,
            matchmaking,
            work: Mutex::new(work_mode::WorkState::new()),
        });
        runtime.start_event_processing();
        runtime
    }

    /// 訂閱之後發布的 Bot 事件。
    ///
    /// # 回傳
    ///
    /// 唯一桌面通知接收端，保留完整處理順序。
    pub(crate) fn take_bot_notifications(&self) -> mpsc::UnboundedReceiver<BotEvent> {
        self.events.take_notifications()
    }

    /// 訂閱目前與後續配對快照。
    ///
    /// # 回傳
    ///
    /// 保留最新配對狀態的 watch 接收端。
    pub(crate) fn subscribe_matchmaking(
        &self,
    ) -> watch::Receiver<crate::matchmaking::MatchmakingSnapshot> {
        self.matchmaking.subscribe()
    }

    /// 訂閱配對診斷事件。
    ///
    /// # 回傳
    ///
    /// 有界診斷廣播接收端。
    pub(crate) fn subscribe_matchmaking_diagnostics(
        &self,
    ) -> broadcast::Receiver<MatchmakingDiagnostic> {
        self.matchmaking.subscribe_diagnostics()
    }

    /// 啟動唯一業務事件消費流程，弱參照避免背景任務延長 runtime 壽命。
    fn start_event_processing(self: &Arc<Self>) {
        let mut events = self.events.take_business_events();
        let runtime = Arc::downgrade(self);
        tauri::async_runtime::spawn(async move {
            while let Some(event) = events.recv().await {
                let Some(runtime) = runtime.upgrade() else {
                    break;
                };
                if runtime.handle_bot_event(&event).await {
                    runtime.events.notify(event);
                }
            }
        });
    }

    /// 在生命週期鎖內處理業務事件，過期 Nick 事件不轉送。
    async fn handle_bot_event(&self, event: &BotEvent) -> bool {
        let mut work = self.work.lock().await;
        let nick_run = match event {
            BotEvent::NickRollerState { run_id, .. }
            | BotEvent::NickCandidate { run_id, .. }
            | BotEvent::NickVerification { run_id, .. }
            | BotEvent::NickAttention { run_id, .. } => Some(*run_id),
            _ => None,
        };
        if nick_run.is_some_and(|id| !work.accepts_nick(id)) {
            return false;
        }
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
                        .map_err(|e| e.to_string())
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
        self.matchmaking.handle_bot_event(event).await;
        if let BotEvent::NickRollerState {
            run_id,
            bot_id,
            phase,
            ..
        } = event
        {
            if phase.is_terminal() {
                work.complete_nick_bot(*run_id, bot_id, &self.events);
            }
        }
        self.reconcile_work(&mut work).await;
        true
    }

    /// 拒絕後續工作並在同一生命週期鎖內完成所有停止操作。
    pub(crate) async fn shutdown(&self) {
        let mut work = self.work.lock().await;
        let nick = work.begin_shutdown(&self.events);
        self.matchmaking.stop().await;
        if let Some((run_id, bot_ids)) = nick {
            for bot_id in bot_ids {
                let _ = self
                    .bots
                    .command(
                        &bot_id,
                        crate::bot_runtime::BotCommand::StopNickRoller { run_id },
                    )
                    .await;
            }
        }
        self.bots.shutdown().await;
    }

    /// 取得完整啟停操作之後的工作模式。
    pub(crate) async fn active_mode(&self) -> RuntimeModeSnapshot {
        let mut work = self.work.lock().await;
        self.reconcile_work(&mut work).await;
        work.snapshot()
    }

    /// 配對結束時保留結果，但停止追蹤，避免日誌重啟已釋放的工作模式。
    async fn reconcile_work(&self, work: &mut work_mode::WorkState) {
        if work.mode() == RuntimeMode::Matching && self.matchmaking.finish_if_terminal().await {
            work.finish_matching(&self.events);
        }
    }

    // 鎖住整次讀改寫，並移到 blocking worker，避免 Registry 操作阻塞非同步執行緒。
    /// 在 blocking worker 內持鎖執行完整儲存操作。
    ///
    /// # 參數
    ///
    /// - `operation` 讀取或更新 Store 的閉包，須擁有跨執行緒所需資料。
    ///
    /// # 回傳
    ///
    /// 閉包結果；鎖中毒或 worker 失敗時回傳錯誤。
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
