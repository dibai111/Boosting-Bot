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

//! 管理配對工作階段的啟停、事件分派與快照發布，細部策略分散至子模組。

#[cfg(test)]
mod behavior_tests;
mod effects;
mod flow;
mod input;
mod lifecycle;
mod queue;
mod retry;
mod round;
mod state;
mod verification;

use self::{
    flow::{selected_bot_ids, waiting_bot},
    input::{SessionInput, TimerKind},
    state::SessionState,
};
use super::{
    parser::{parse_player_log_line, PlayerLogEvent},
    player_log::{PlayerLogMessage, PlayerLogTailer},
};
use super::{MatchmakingPhase, MatchmakingPlan, MatchmakingSnapshot};
use crate::bot_runtime::{BotEvent, BotPhase, BotRuntime};
use anyhow::{Context, Result};
use serde::Serialize;
use std::{path::PathBuf, sync::Arc};
use tokio::sync::{broadcast, mpsc, watch, Mutex};
use uuid::Uuid;

#[derive(Clone, Serialize)]
/// 前端可訂閱的配對診斷訊息，可選擇綁定單一 Bot。
pub(crate) struct MatchmakingDiagnostic {
    pub bot_id: Option<String>,
    pub message: String,
}

/// 多 Bot 配對協調器，持有計畫、每輪狀態、日誌追蹤及快照通道。
pub struct MatchmakingSession {
    effects: Arc<dyn effects::MatchmakingEffects>,
    lifecycle: Mutex<()>,
    state: Mutex<SessionState>,
    tailer: Mutex<Option<PlayerLogTailer>>,
    snapshots: watch::Sender<MatchmakingSnapshot>,
    diagnostics: broadcast::Sender<MatchmakingDiagnostic>,
}

impl MatchmakingSession {
    /// 建立空閒配對工作階段與有界通知通道。
    ///
    /// # 參數
    ///
    /// - `bots` 發送配對業務指令的 Bot runtime。
    ///
    /// # 回傳
    ///
    /// 尚未啟動日誌追蹤的協調器。
    pub fn new(bots: Arc<BotRuntime>) -> Self {
        Self::with_effects(Arc::new(effects::RuntimeEffects(bots)))
    }

    fn with_effects(effects: Arc<dyn effects::MatchmakingEffects>) -> Self {
        let snapshot = MatchmakingSnapshot::idle();
        let (snapshots, _) = watch::channel(snapshot.clone());
        let (diagnostics, _) = broadcast::channel(128);
        Self {
            effects,
            lifecycle: Mutex::new(()),
            state: Mutex::new(SessionState::new(snapshot)),
            tailer: Mutex::new(None),
            snapshots,
            diagnostics,
        }
    }

    /// 訂閱最新配對快照。
    ///
    /// # 回傳
    ///
    /// watch 接收端；中間快照可能被較新狀態取代。
    pub fn subscribe(&self) -> watch::Receiver<MatchmakingSnapshot> {
        self.snapshots.subscribe()
    }

    /// 訂閱配對診斷事件。
    ///
    /// # 回傳
    ///
    /// 有界廣播接收端。
    pub(crate) fn subscribe_diagnostics(&self) -> broadcast::Receiver<MatchmakingDiagnostic> {
        self.diagnostics.subscribe()
    }

    /// 發布供日誌顯示的配對診斷。
    ///
    /// # 參數
    ///
    /// - `bot_id` 診斷所屬 Bot，整體訊息為 None。
    /// - `message` 不含憑據的診斷文字。
    ///
    /// 無訂閱者可丟棄。
    pub(crate) fn diagnose(&self, bot_id: Option<&str>, message: impl Into<String>) {
        let _ = self.diagnostics.send(MatchmakingDiagnostic {
            bot_id: bot_id.map(str::to_owned),
            message: message.into(),
        });
    }

    /// 複製最後發布的快照，包含事件、命令回覆與初始讀取共用的版本。
    ///
    /// # 回傳
    ///
    /// 獨立的最新快照。
    pub async fn snapshot(&self) -> MatchmakingSnapshot {
        self.snapshots.borrow().clone()
    }

    /// 序列化啟停操作，開啟玩家日誌並建立新的配對工作階段。
    ///
    /// # 參數
    ///
    /// - `plan` 已驗證的不可變配對計畫。
    ///
    /// # 回傳
    ///
    /// 等待玩家轉服的快照，或路徑／追蹤啟動錯誤。
    pub async fn start(self: &Arc<Self>, plan: MatchmakingPlan) -> Result<MatchmakingSnapshot> {
        let _lifecycle = self.lifecycle.lock().await;
        let path = tokio::fs::canonicalize(PathBuf::from(plan.log_path()))
            .await
            .context("open player Minecraft log")?;

        self.stop_inner().await;
        let mode = plan.mode();
        let required_matches = plan.required_matches();
        let bot_ids = plan.bot_ids().map(str::to_owned).collect::<Vec<_>>();
        let (messages, receiver) = mpsc::channel(128);
        let tailer = PlayerLogTailer::start(path, messages).await?;

        let snapshot = {
            let mut state = self.state.lock().await;
            state.plan = Some(plan);
            state.clear_round_tracking();
            state.snapshot = MatchmakingSnapshot {
                revision: 0,
                session_id: Some(Uuid::new_v4().to_string()),
                round_id: None,
                phase: MatchmakingPhase::AwaitingPlayer,
                mode: Some(mode),
                player_server: None,
                required_matches,
                matched_bots: 0,
                bots: bot_ids.iter().map(|id| waiting_bot(id)).collect(),
                message: Some("Waiting for the player server transfer".to_owned()),
            };
            state.snapshot.clone()
        };
        *self.tailer.lock().await = Some(tailer);
        let snapshot = self.publish(snapshot);
        let _consumer = self.spawn_log_consumer(
            snapshot.session_id.clone().expect("started session id"),
            receiver,
        );
        Ok(snapshot)
    }

    /// 停止日誌追蹤並撤回所選 Bot。
    ///
    /// # 回傳
    ///
    /// 停止完成後返回。
    pub async fn stop(&self) {
        let _lifecycle = self.lifecycle.lock().await;
        self.stop_inner().await;
    }

    /// 終止已失敗的工作階段，保留最後快照供畫面及診斷使用。
    pub(crate) async fn finish_if_terminal(&self) -> bool {
        let _lifecycle = self.lifecycle.lock().await;
        if !matches!(
            self.state.lock().await.snapshot.phase,
            MatchmakingPhase::Idle | MatchmakingPhase::Failed
        ) {
            return false;
        }
        if let Some(tailer) = self.tailer.lock().await.take() {
            tailer.stop().await;
        }
        let ids = {
            let mut state = self.state.lock().await;
            let ids = selected_bot_ids(&state);
            state.reset_round();
            state.plan = None;
            ids
        };
        self.withdraw_bots(&ids).await;
        true
    }

    async fn stop_inner(&self) {
        if let Some(tailer) = self.tailer.lock().await.take() {
            tailer.stop().await;
        }
        let bot_ids = {
            let mut state = self.state.lock().await;
            let ids = selected_bot_ids(&state);
            state.reset_round();
            state.plan = None;
            state.snapshot = MatchmakingSnapshot::idle();
            self.publish(state.snapshot.clone());
            ids
        };
        self.withdraw_bots(&bot_ids).await;
    }

    /// 按事件種類分派嘗試結果、佇列、驗證及連線變更。
    ///
    /// # 參數
    ///
    /// - `event` Bot runtime 發布的業務事件。
    ///
    /// 有效變更會發布快照。
    pub async fn handle_bot_event(self: &Arc<Self>, event: &BotEvent) {
        self.handle_input(SessionInput::Bot(event.clone())).await;
    }

    async fn apply_bot_event(self: &Arc<Self>, event: &BotEvent) {
        match event {
            BotEvent::MatchAttemptResult {
                bot_id,
                session_id,
                round_id,
                target_generation,
                attempt_id,
                server,
            } => {
                self.handle_attempt_result(
                    bot_id,
                    session_id,
                    round_id,
                    *target_generation,
                    attempt_id,
                    server,
                )
                .await;
            }
            BotEvent::MatchAttemptFailed {
                bot_id,
                round_id,
                target_generation,
                attempt_id,
                code,
                message,
                ..
            } => {
                self.handle_attempt_failure(
                    bot_id,
                    round_id,
                    *target_generation,
                    attempt_id,
                    code,
                    message,
                )
                .await;
            }
            BotEvent::MatchRetryReady { bot_id, request_id } => {
                self.handle_match_retry_ready(bot_id, request_id).await;
            }
            BotEvent::MatchRetryPreparationFailed {
                bot_id,
                request_id,
                message,
            } => {
                self.handle_match_retry_preparation_failure(bot_id, request_id, message)
                    .await;
            }
            BotEvent::QueueProgress {
                bot_id,
                current,
                total,
            } => {
                self.handle_bot_queue_progress(bot_id, *current, *total)
                    .await;
            }
            BotEvent::DuelPitchObserved {
                bot_id,
                round_id,
                target_generation,
                attempt_id,
                direction,
                pitch,
            } => {
                self.handle_duel_pitch_observed(
                    bot_id,
                    round_id,
                    *target_generation,
                    attempt_id,
                    direction,
                    *pitch,
                )
                .await;
            }
            BotEvent::BotGameState {
                bot_id,
                round_id,
                target_generation,
                state,
            } => {
                self.handle_bot_game_state(bot_id, round_id, *target_generation, *state)
                    .await;
            }
            BotEvent::ChatMessage { .. } => {}
            BotEvent::Status {
                bot_id,
                phase: BotPhase::Offline,
                ..
            } => {
                self.mark_unavailable(bot_id, "Bot disconnected").await;
            }
            BotEvent::Error {
                bot_id: Some(bot_id),
                code,
                message,
            } if is_terminal_bot_error(code) => {
                self.mark_unavailable(bot_id, message).await;
            }
            _ => {}
        }
    }

    /// 以弱參照消費日誌事件，避免背景任務維持協調器存活。
    ///
    /// # 參數
    ///
    /// - `session_id` 日誌開啟時的工作階段識別，整個 consumer 生命週期固定。
    /// - `receiver` 此日誌追蹤器的訊息接收端。
    ///
    /// # 回傳
    ///
    /// 消費任務；舊工作階段的排隊輸入會在共同入口被拒絕。
    fn spawn_log_consumer(
        self: &Arc<Self>,
        session_id: String,
        mut receiver: mpsc::Receiver<PlayerLogMessage>,
    ) -> tokio::task::JoinHandle<()> {
        let coordinator = Arc::downgrade(self);
        tokio::spawn(async move {
            while let Some(message) = receiver.recv().await {
                let Some(coordinator) = coordinator.upgrade() else {
                    break;
                };
                coordinator.handle_player_log(&session_id, message).await;
            }
        })
    }

    /// 解析玩家聊天日誌並推進配對輪次。
    ///
    /// # 參數
    ///
    /// - `line` 單行玩家日誌。
    ///
    /// 非配對訊息忽略。
    async fn apply_player_line(self: &Arc<Self>, line: &str) {
        match parse_player_log_line(line) {
            Some(PlayerLogEvent::ServerTransfer(server)) => self.handle_player_server(server).await,
            Some(PlayerLogEvent::QueueProgress {
                username,
                current,
                total,
            }) => {
                self.handle_player_queue_progress(&username, current, total)
                    .await
            }
            Some(PlayerLogEvent::ChatMessage { username, message }) => {
                self.handle_player_chat(&username, &message).await
            }
            Some(PlayerLogEvent::LobbyJoined) => self.handle_player_lobby_joined().await,
            Some(PlayerLogEvent::GameStarted) => self.handle_game_started().await,
            Some(PlayerLogEvent::GameEnded) => self.reset_for_lobby("Game ended").await,
            None => {}
        }
    }

    /// 替換 watch 通道內的最新快照。
    ///
    /// # 參數
    ///
    /// - `snapshot` 完整配對快照。
    ///
    /// # 回傳
    ///
    /// 帶新版本的快照；尚無接收端仍保留最新值。
    fn publish(&self, mut snapshot: MatchmakingSnapshot) -> MatchmakingSnapshot {
        self.snapshots.send_modify(|current| {
            snapshot.revision = current
                .revision
                .checked_add(1)
                .expect("snapshot revision exhausted");
            *current = snapshot.clone();
        });
        snapshot
    }
}

fn is_terminal_bot_error(code: &str) -> bool {
    matches!(
        code,
        "banned"
            | "connection_error"
            | "kicked"
            | "match_not_ready"
            | "session_panic"
            | "start_failed"
    )
}

#[cfg(test)]
mod tests {
    use super::super::BotMatchPhase;
    use super::super::{
        modes::{bedwars, duels},
        MatchmakingPlan, StartMatchmakingInput,
    };
    use super::flow::{
        matched_count, possible_matches, random_command_spam_retry_delay,
        should_withdraw_after_commit,
    };
    use super::state::{PendingRetry, PresenceCheck};
    use super::*;
    use crate::{bot_runtime::GameMode, hypixel};
    use std::collections::{HashMap, HashSet};
    use std::time::{Duration, Instant};

    fn plan(bot_ids: &[&str], required_matches: usize) -> MatchmakingPlan {
        let input = StartMatchmakingInput {
            mode: GameMode::BedwarsDoubles,
            bot_ids: bot_ids.iter().map(|id| (*id).to_owned()).collect(),
            verify_presence: true,
            verify_duel_pitch: true,
            required_matches,
            log_path: "latest.log".to_owned(),
        };
        MatchmakingPlan::compile(
            input,
            bot_ids
                .iter()
                .map(|id| ((*id).to_owned(), format!("Player_{id}"))),
        )
        .expect("valid plan")
    }

    #[test]
    fn counts_only_bots_that_can_still_match() {
        let mut snapshot = MatchmakingSnapshot::idle();
        snapshot.bots = ["matched", "queued", "returning", "unavailable"]
            .into_iter()
            .map(waiting_bot)
            .collect();
        snapshot.bots[0].phase = BotMatchPhase::Matched;
        snapshot.bots[1].phase = BotMatchPhase::Queued;
        snapshot.bots[2].phase = BotMatchPhase::Returning;
        snapshot.bots[3].phase = BotMatchPhase::Unavailable;

        assert_eq!(matched_count(&snapshot), 1);
        assert_eq!(possible_matches(&snapshot), 3);
    }

    #[test]
    fn clears_every_piece_of_round_tracking() {
        let mut state = SessionState {
            snapshot: MatchmakingSnapshot::idle(),
            plan: Some(plan(&["bot-a"], 1)),
            target_generation: 1,
            active_attempts: HashMap::from([("bot-a".to_owned(), "attempt".to_owned())]),
            retry_requests: HashMap::from([(
                "bot-a".to_owned(),
                PendingRetry {
                    request_id: "retry".to_owned(),
                    generation: 1,
                    retry_delay: Duration::ZERO,
                },
            )]),
            round_started_at: Some(Instant::now()),
            player_queue: Some(duels::next_observation(None, 1, 2, Instant::now())),
            player_bedwars_queue: Some(bedwars::observe_queue(1, 8, Instant::now())),
            player_queue_usernames: HashSet::from(["Player".to_owned()]),
            bot_queues: HashMap::from([(
                "bot-a".to_owned(),
                duels::next_observation(None, 1, 2, Instant::now()),
            )]),
            bot_bedwars_queues: HashMap::from([(
                "bot-a".to_owned(),
                bedwars::observe_queue(1, 8, Instant::now()),
            )]),
            bot_transfers: HashSet::from(["bot-a".to_owned()]),
            presence_checks: HashMap::from([(
                "bot-a".to_owned(),
                PresenceCheck {
                    username: "Player".to_owned(),
                    generation: 1,
                    attempt_id: "attempt".to_owned(),
                    server: "mini1".to_owned(),
                },
            )]),
            duel_pitch_checks: HashMap::new(),
            duel_pitch_observations: HashMap::new(),
        };

        state.clear_round_tracking();

        assert!(state.active_attempts.is_empty());
        assert!(state.retry_requests.is_empty());
        assert!(state.round_started_at.is_none());
        assert!(state.player_queue.is_none());
        assert!(state.player_bedwars_queue.is_none());
        assert!(state.player_queue_usernames.is_empty());
        assert!(state.bot_queues.is_empty());
        assert!(state.bot_bedwars_queues.is_empty());
        assert!(state.bot_transfers.is_empty());
        assert!(state.presence_checks.is_empty());
        assert!(state.duel_pitch_checks.is_empty());
        assert!(state.duel_pitch_observations.is_empty());
    }

    #[test]
    fn reset_round_invalidates_previous_worker_events() {
        let mut state = SessionState::new(MatchmakingSnapshot::idle());

        state.reset_round();
        assert_eq!(state.target_generation, 1);

        state.reset_round();
        assert_eq!(state.target_generation, 2);
    }

    #[test]
    fn unavailable_bot_is_not_withdrawn_after_commit() {
        assert!(should_withdraw_after_commit(BotMatchPhase::Queued));
        assert!(!should_withdraw_after_commit(BotMatchPhase::Matched));
        assert!(!should_withdraw_after_commit(BotMatchPhase::Unavailable));
    }

    #[test]
    fn recognizes_game_server_ids_without_treating_lobbies_as_targets() {
        assert!(hypixel::is_game_server("mini198Q"));
        assert!(hypixel::is_game_server("MINI5D"));
        assert!(!hypixel::is_game_server("bedwarslobby1"));
        assert!(!hypixel::is_game_server("mini"));
    }

    #[test]
    fn command_spam_retry_delay_stays_inside_the_jitter_range() {
        for _ in 0..100 {
            let delay = random_command_spam_retry_delay();
            assert!(delay >= Duration::from_millis(800));
            assert!(delay <= Duration::from_millis(1_000));
        }
    }

    #[test]
    fn recognizes_terminal_bot_errors() {
        for code in [
            "banned",
            "connection_error",
            "kicked",
            "match_not_ready",
            "session_panic",
            "start_failed",
        ] {
            assert!(is_terminal_bot_error(code), "{code}");
        }
        assert!(!is_terminal_bot_error("command_spam"));
    }
}
