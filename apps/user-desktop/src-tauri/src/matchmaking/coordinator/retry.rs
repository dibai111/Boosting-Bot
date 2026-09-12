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

//! 安排 Limbo 準備及延後重試，以 generation 和 request_id 過濾過期回覆。

use super::super::{BotMatchPhase, MatchmakingPhase};
use super::flow::{possible_matches, retry_message};
use super::state::PendingRetry;
use super::MatchmakingSession;
use crate::bot_runtime::{BotCommand, MatchAttempt};
use std::sync::Arc;
use std::time::Duration;
use uuid::Uuid;

impl MatchmakingSession {
    /// 移除目前嘗試並安排立即重試或 Limbo 準備。
    /// @param bot_id 本機 Bot ID。
    /// @param server 上次觀察到的伺服器。
    /// @param message 重試原因。
    /// @param retry_delay 重新排隊前等待時間。
    /// @param prepare_limbo 是否先等待 Limbo 確認。
    /// @return 無回傳值；已提交或沒有有效嘗試時忽略。
    pub(crate) async fn schedule_retry(
        self: &Arc<Self>,
        bot_id: &str,
        server: Option<String>,
        message: &str,
        retry_delay: Duration,
        prepare_limbo: bool,
    ) {
        let (should_fail, request_id, generation, snapshot) = {
            let mut state = self.state.lock().await;
            // 逾時檢查與取得此鎖之間可能已提交或撤回；不可重新排入已結束的嘗試。
            if state.snapshot.phase != MatchmakingPhase::Matching
                || !state.active_attempts.contains_key(bot_id)
            {
                return;
            }
            state.active_attempts.remove(bot_id);
            state.presence_checks.remove(bot_id);
            state.duel_pitch_checks.remove(bot_id);
            state.duel_pitch_observations.remove(bot_id);
            let generation = state.target_generation;
            let Some(bot) = state
                .snapshot
                .bots
                .iter_mut()
                .find(|bot| bot.bot_id == bot_id)
            else {
                return;
            };
            bot.server = server;
            bot.message = Some(if prepare_limbo {
                format!("{message}; entering limbo")
            } else {
                retry_message(message, retry_delay)
            });
            bot.phase = if prepare_limbo {
                BotMatchPhase::Returning
            } else {
                BotMatchPhase::Queued
            };
            let should_fail = possible_matches(&state.snapshot) < state.snapshot.required_matches;
            let request_id = Uuid::new_v4().to_string();
            if !should_fail && prepare_limbo {
                state.retry_requests.insert(
                    bot_id.to_owned(),
                    PendingRetry {
                        request_id: request_id.clone(),
                        generation,
                        retry_delay,
                    },
                );
            }
            (should_fail, request_id, generation, state.snapshot.clone())
        };
        self.publish(snapshot);

        if should_fail {
            self.fail_round("Not enough available bots to reach the minimum")
                .await;
            return;
        }
        if !prepare_limbo {
            self.spawn_retry(bot_id.to_owned(), generation, retry_delay);
            return;
        }
        let _ = self
            .bots
            .command(bot_id, BotCommand::PrepareMatchRetry { request_id })
            .await;
    }

    /// 核對重試 request_id 及世代，再排程下一次嘗試。
    /// @param bot_id 本機 Bot ID。
    /// @param request_id Bot 回報已完成準備的 request ID。
    /// @return 無回傳值；過期回應忽略。
    pub(crate) async fn handle_match_retry_ready(self: &Arc<Self>, bot_id: &str, request_id: &str) {
        let (generation, retry_delay, snapshot) = {
            let mut state = self.state.lock().await;
            let Some(pending) = state.retry_requests.get(bot_id) else {
                return;
            };
            if pending.request_id != request_id
                || pending.generation != state.target_generation
                || state.snapshot.phase != MatchmakingPhase::Matching
            {
                return;
            }
            let generation = pending.generation;
            let retry_delay = pending.retry_delay;
            state.retry_requests.remove(bot_id);
            if let Some(bot) = state
                .snapshot
                .bots
                .iter_mut()
                .find(|bot| bot.bot_id == bot_id)
            {
                bot.message = Some(retry_message("Limbo ready", retry_delay));
            }
            (generation, retry_delay, state.snapshot.clone())
        };
        self.publish(snapshot);
        self.spawn_retry(bot_id.to_owned(), generation, retry_delay);
    }

    /// 以弱參照建立延遲重試任務。
    /// @param bot_id 本機 Bot ID。
    /// @param generation 排程時的目標世代。
    /// @param retry_delay 重試前等待時間。
    /// @return 無回傳值；協調器已釋放時不執行。
    pub(crate) fn spawn_retry(
        self: &Arc<Self>,
        bot_id: String,
        generation: u64,
        retry_delay: Duration,
    ) {
        let coordinator = Arc::downgrade(self);
        tokio::spawn(async move {
            tokio::time::sleep(retry_delay).await;
            if let Some(coordinator) = coordinator.upgrade() {
                coordinator.retry_bot(&bot_id, generation).await;
            }
        });
    }

    /// 只接受目前重試要求的失敗回應。
    /// @param bot_id 本機 Bot ID。
    /// @param request_id 準備要求 ID。
    /// @param message 準備失敗原因。
    /// @return 無回傳值；有效回應會將 Bot 標成不可用。
    pub(crate) async fn handle_match_retry_preparation_failure(
        &self,
        bot_id: &str,
        request_id: &str,
        message: &str,
    ) {
        let accepted = {
            let mut state = self.state.lock().await;
            let accepted = state
                .retry_requests
                .get(bot_id)
                .is_some_and(|pending| pending.request_id == request_id);
            if accepted {
                state.retry_requests.remove(bot_id);
            }
            accepted
        };
        if accepted {
            self.mark_unavailable(bot_id, message).await;
        }
    }

    /// 確認世代及 Bot 階段後產生新嘗試 ID 並重新排隊。
    /// @param bot_id 本機 Bot ID。
    /// @param generation 排程時的目標世代。
    /// @return 無回傳值；已結束或取代的重試忽略。
    pub(crate) async fn retry_bot(&self, bot_id: &str, generation: u64) {
        let (command, snapshot) = {
            let mut state = self.state.lock().await;
            if state.target_generation != generation
                || state.snapshot.phase != MatchmakingPhase::Matching
                || state.retry_requests.contains_key(bot_id)
            {
                return;
            }
            let Some(plan) = state.plan.as_ref() else {
                return;
            };
            let mode = plan.mode();
            let requires_pitch_verification = plan.requires_pitch_verification();
            let Some(session_id) = state.snapshot.session_id.clone() else {
                return;
            };
            let Some(round_id) = state.snapshot.round_id.clone() else {
                return;
            };
            let Some(bot) = state.snapshot.bots.iter_mut().find(|bot| {
                bot.bot_id == bot_id
                    && matches!(bot.phase, BotMatchPhase::Queued | BotMatchPhase::Returning)
            }) else {
                return;
            };
            bot.phase = BotMatchPhase::Queued;
            bot.server = None;
            bot.message = None;
            state.clear_bot_attempt_tracking(bot_id);
            let attempt_id = Uuid::new_v4().to_string();
            state
                .active_attempts
                .insert(bot_id.to_owned(), attempt_id.clone());
            let command = BotCommand::BeginMatchAttempt(MatchAttempt {
                session_id,
                round_id,
                target_generation: generation,
                attempt_id,
                mode,
                requires_pitch_verification,
            });
            (command, state.snapshot.clone())
        };
        self.publish(snapshot);
        let _ = self.bots.command(bot_id, command).await;
    }
}
