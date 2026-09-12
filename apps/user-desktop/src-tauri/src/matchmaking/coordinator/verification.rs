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

//! 管理聊天與俯仰驗證；確認訊號和逾時任務必須對應目前嘗試。

use super::super::detector::{chat_verification, duel_pitch};
use super::super::modes::{bedwars, duels};
use super::super::MatchmakingPhase;
use super::state::PresenceCheck;
use super::MatchmakingSession;
use crate::bot_runtime::{BotCommand, DuelPitchDirection};
use std::sync::Arc;
use std::time::{Duration, Instant};

impl MatchmakingSession {
    /// 核對目前嘗試並保存俯仰觀察，已有確認閘門時提交候選。
    /// @param bot_id 本機 Bot ID。
    /// @param round_id 目前配對輪次 ID。
    /// @param generation 目標世代編號。
    /// @param attempt_id 單次嘗試 ID。
    /// @param direction 上下俯仰方向。
    /// @param pitch 事件提供的角度，單位為弧度。
    /// @return 無回傳值；不符合來源或設定時不接受。
    pub(crate) async fn handle_duel_pitch_observed(
        self: &Arc<Self>,
        bot_id: &str,
        round_id: &str,
        generation: u64,
        attempt_id: &str,
        direction: &DuelPitchDirection,
        pitch: f32,
    ) {
        let should_match = {
            let mut state = self.state.lock().await;
            let valid = state.snapshot.phase == MatchmakingPhase::Matching
                && state.snapshot.round_id.as_deref() == Some(round_id)
                && state.target_generation == generation
                && state
                    .active_attempts
                    .get(bot_id)
                    .is_some_and(|current| current == attempt_id)
                && state
                    .plan
                    .as_ref()
                    .is_some_and(|plan| plan.requires_pitch_verification());
            if !valid {
                false
            } else {
                state.duel_pitch_observations.insert(
                    bot_id.to_owned(),
                    duel_pitch::Observation {
                        generation,
                        attempt_id: attempt_id.to_owned(),
                        observed_at: Instant::now(),
                    },
                );
                let should_match = state.duel_pitch_checks.remove(bot_id).is_some();
                if should_match {
                    if let Some(bot) = state
                        .snapshot
                        .bots
                        .iter_mut()
                        .find(|bot| bot.bot_id == bot_id)
                    {
                        bot.message = Some("Pitch verification confirmed".to_owned());
                    }
                }
                should_match
            }
        };
        self.diagnose(
            Some(bot_id),
            format!(
                "[duels pitch] Rust received gesture: direction={}, pitch={:.1}deg, accepted={}",
                direction.as_str(),
                pitch.to_degrees(),
                should_match
            ),
        );
        if should_match {
            let server = self
                .state
                .lock()
                .await
                .snapshot
                .bots
                .iter()
                .find(|bot| bot.bot_id == bot_id)
                .and_then(|bot| bot.server.clone());
            if let Some(server) = server {
                self.mark_matched(bot_id, Some(&server)).await;
            }
        }
    }

    /// 檢查觀察是否屬於目前嘗試與輪次。
    /// @param bot_id 本機 Bot ID。
    /// @return 存在有效俯仰觀察時為 true。
    pub(crate) async fn has_duel_pitch_observation(&self, bot_id: &str) -> bool {
        let state = self.state.lock().await;
        let Some(observation) = state.duel_pitch_observations.get(bot_id) else {
            return false;
        };
        let Some(attempt_id) = state.active_attempts.get(bot_id) else {
            return false;
        };
        duel_pitch::belongs_to_attempt(
            observation,
            state.target_generation,
            attempt_id,
            state.round_started_at,
        )
    }

    /// 為有效嘗試建立俯仰驗證期限。
    /// @param bot_id 本機 Bot ID。
    /// @param server 已由佇列確認的伺服器。
    /// @return 無回傳值；已有驗證時不重複建立。
    pub(crate) async fn start_duel_pitch_check(self: &Arc<Self>, bot_id: &str, server: &str) {
        let (generation, attempt_id) = {
            let mut state = self.state.lock().await;
            if state.duel_pitch_checks.contains_key(bot_id) {
                return;
            }
            let Some(attempt_id) = state.active_attempts.get(bot_id).cloned() else {
                return;
            };
            let generation = state.target_generation;
            state.duel_pitch_checks.insert(
                bot_id.to_owned(),
                duel_pitch::VerificationCheck {
                    generation,
                    attempt_id: attempt_id.clone(),
                    server: server.to_owned(),
                },
            );
            if let Some(bot) = state
                .snapshot
                .bots
                .iter_mut()
                .find(|bot| bot.bot_id == bot_id)
            {
                bot.message = Some("Waiting for pitch verification".to_owned());
            }
            (generation, attempt_id)
        };
        self.publish(self.state.lock().await.snapshot.clone());
        self.diagnose(
            Some(bot_id),
            format!("[duels pitch] verification gate opened for server={server}"),
        );
        self.spawn_duel_pitch_confirmation_timeout(bot_id.to_owned(), generation, attempt_id);
    }

    /// 等待俯仰確認，期限到時只處理相同世代與嘗試。
    /// @param bot_id 本機 Bot ID。
    /// @param generation 排程時的目標世代。
    /// @param attempt_id 排程時的嘗試 ID。
    /// @return 無回傳值；建立背景期限任務。
    pub(crate) fn spawn_duel_pitch_confirmation_timeout(
        self: &Arc<Self>,
        bot_id: String,
        generation: u64,
        attempt_id: String,
    ) {
        let coordinator = Arc::downgrade(self);
        tokio::spawn(async move {
            tokio::time::sleep(duels::PITCH_CONFIRM_TIMEOUT).await;
            let Some(coordinator) = coordinator.upgrade() else {
                return;
            };
            let retry_server = {
                let state = coordinator.state.lock().await;
                state.duel_pitch_checks.get(&bot_id).and_then(|check| {
                    (check.generation == generation
                        && check.attempt_id == attempt_id
                        && state.snapshot.phase == MatchmakingPhase::Matching)
                        .then(|| check.server.clone())
                })
            };
            if let Some(server) = retry_server {
                coordinator.diagnose(
                    Some(&bot_id),
                    "[duels pitch] timed out without a valid gesture",
                );
                coordinator
                    .schedule_retry(
                        &bot_id,
                        Some(server),
                        "Pitch verification not confirmed",
                        Duration::ZERO,
                        true,
                    )
                    .await;
            }
        });
    }

    /// 依玩家日誌中的真實名稱確認在場，不要求訊息等於送出的短句。
    /// @param username 日誌中解析出的發言者名稱。
    /// @param _message 發言內容；目前規則不使用。
    /// @return 無回傳值；沒有對應驗證時忽略。
    pub(crate) async fn handle_player_chat(self: &Arc<Self>, username: &str, _message: &str) {
        let matched_bot = {
            let state = self.state.lock().await;
            state
                .presence_checks
                .iter()
                .find(|(_, check)| chat_verification::matches(&check.username, username))
                .map(|(bot_id, check)| (bot_id.clone(), check.server.clone()))
        };
        let Some((bot_id, server)) = matched_bot else {
            return;
        };
        {
            let mut state = self.state.lock().await;
            state.presence_checks.remove(&bot_id);
        }
        self.mark_matched(&bot_id, Some(&server)).await;
    }

    /// 記錄預期玩家名稱並向 Bot 發送聊天驗證短句。
    /// @param bot_id 本機 Bot ID。
    /// @param server 已由佇列確認的伺服器。
    /// @return 無回傳值；發送失敗時安排重試。
    pub(crate) async fn start_presence_check(self: &Arc<Self>, bot_id: &str, server: &str) {
        let (command, generation, attempt_id) = {
            let mut state = self.state.lock().await;
            if state.presence_checks.contains_key(bot_id) {
                return;
            }
            let Some(username) = state
                .plan
                .as_ref()
                .and_then(|plan| plan.username(bot_id).map(str::to_owned))
            else {
                return;
            };
            let Some(attempt_id) = state.active_attempts.get(bot_id).cloned() else {
                return;
            };
            let phrase = chat_verification::random_phrase();
            let generation = state.target_generation;
            state.presence_checks.insert(
                bot_id.to_owned(),
                PresenceCheck {
                    username,
                    generation,
                    attempt_id: attempt_id.clone(),
                    server: server.to_owned(),
                },
            );
            if let Some(bot) = state
                .snapshot
                .bots
                .iter_mut()
                .find(|bot| bot.bot_id == bot_id)
            {
                bot.message = Some("Waiting for chat confirmation".to_owned());
            }
            (BotCommand::SendChat(phrase), generation, attempt_id)
        };
        self.publish(self.state.lock().await.snapshot.clone());
        if let Err(error) = self.bots.command(bot_id, command).await {
            {
                let mut state = self.state.lock().await;
                state.presence_checks.remove(bot_id);
                if let Some(bot) = state
                    .snapshot
                    .bots
                    .iter_mut()
                    .find(|bot| bot.bot_id == bot_id)
                {
                    bot.message = Some("Chat verification could not be sent".to_owned());
                }
            }
            self.schedule_retry(
                bot_id,
                Some(server.to_owned()),
                &format!("Chat verification failed: {error}"),
                Duration::ZERO,
                true,
            )
            .await;
            return;
        }
        {
            let mut state = self.state.lock().await;
            if state.presence_checks.get(bot_id).is_some_and(|check| {
                check.generation == generation && check.attempt_id == attempt_id
            }) {
                if let Some(bot) = state
                    .snapshot
                    .bots
                    .iter_mut()
                    .find(|bot| bot.bot_id == bot_id)
                {
                    bot.message = Some("Chat verification sent".to_owned());
                }
                self.publish(state.snapshot.clone());
            }
        }
        self.diagnose(
            Some(bot_id),
            "[chat] scan: chat verification sent; waiting for player log",
        );
        self.spawn_presence_timeout(bot_id.to_owned(), generation, attempt_id);
    }

    /// 為聊天在場確認建立期限。
    /// @param bot_id 本機 Bot ID。
    /// @param generation 排程時的目標世代。
    /// @param attempt_id 排程時的嘗試 ID。
    /// @return 無回傳值；過期要求不會重試。
    pub(crate) fn spawn_presence_timeout(
        self: &Arc<Self>,
        bot_id: String,
        generation: u64,
        attempt_id: String,
    ) {
        let coordinator = Arc::downgrade(self);
        tokio::spawn(async move {
            tokio::time::sleep(bedwars::CONFIRM_TIMEOUT).await;
            let Some(coordinator) = coordinator.upgrade() else {
                return;
            };
            let retry_server = {
                let state = coordinator.state.lock().await;
                state.presence_checks.get(&bot_id).and_then(|check| {
                    (check.generation == generation
                        && check.attempt_id == attempt_id
                        && state.snapshot.phase == MatchmakingPhase::Matching)
                        .then(|| check.server.clone())
                })
            };
            if let Some(server) = retry_server {
                coordinator
                    .schedule_retry(
                        &bot_id,
                        Some(server),
                        "Chat presence not confirmed",
                        Duration::ZERO,
                        true,
                    )
                    .await;
            }
        });
    }
}
