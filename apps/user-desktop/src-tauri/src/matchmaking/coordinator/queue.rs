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

//! 比對玩家與 Bot 的佇列觀察，依遊戲模式啟動後續確認或逾時重試。

use super::super::detector::real_name_joined_log;
use super::super::modes::{bedwars, duels, skywars};
use super::super::{BotMatchPhase, MatchmakingPhase};
use super::MatchmakingSession;
use crate::bot_runtime::GameKind;
use std::sync::Arc;
use std::time::Duration;

impl MatchmakingSession {
    /// 保存玩家端佇列觀察，並尋找可確認的 Bot。
    /// @param username 加入訊息中的真實玩家名稱。
    /// @param current 目前佇列人數。
    /// @param total 佇列容量。
    /// @return 無回傳值；非配對階段忽略。
    pub(crate) async fn handle_player_queue_progress(
        self: &Arc<Self>,
        username: &str,
        current: u32,
        total: u32,
    ) {
        let skywars_candidates = {
            let mut state = self.state.lock().await;
            if state.snapshot.phase != MatchmakingPhase::Matching {
                return;
            }
            let mode = state.plan.as_ref().map(|plan| plan.mode());
            let duel_mode = mode.is_some_and(|mode| mode.kind() == GameKind::Duels);
            if duel_mode {
                state.player_queue =
                    Some(duels::next_observation(state.player_queue, current, total));
            } else if mode.is_some_and(|mode| mode.kind() == GameKind::Bedwars) {
                state.player_bedwars_queue = Some(bedwars::observe_queue(current, total));
            }
            state
                .player_queue_usernames
                .insert(username.to_ascii_lowercase());
            if !mode.is_some_and(|mode| mode.kind() == GameKind::Skywars) {
                Vec::new()
            } else {
                state
                    .snapshot
                    .bots
                    .iter()
                    .filter(|bot| {
                        bot.phase == BotMatchPhase::Queued
                            && state.bot_transfers.contains(&bot.bot_id)
                            && state.active_attempts.contains_key(&bot.bot_id)
                            && state
                                .plan
                                .as_ref()
                                .and_then(|plan| plan.username(&bot.bot_id))
                                .is_some_and(|name| {
                                    real_name_joined_log::matches(Some(name), username)
                                })
                    })
                    .map(|bot| bot.bot_id.clone())
                    .collect::<Vec<_>>()
            }
        };
        for bot_id in skywars_candidates {
            self.mark_matched(&bot_id, None).await;
        }
        self.confirm_queue_candidates().await;
    }

    /// 依模式保存 Bot 佇列觀察並檢查候選。
    /// @param bot_id 本機 Bot ID。
    /// @param current Bot 看到的目前人數。
    /// @param total Bot 看到的佇列容量。
    /// @return 無回傳值。
    pub(crate) async fn handle_bot_queue_progress(
        self: &Arc<Self>,
        bot_id: &str,
        current: u32,
        total: u32,
    ) {
        {
            let mut state = self.state.lock().await;
            if state.snapshot.phase != MatchmakingPhase::Matching {
                return;
            }
            let mode = state.plan.as_ref().map(|plan| plan.mode());
            let duel_mode = mode.is_some_and(|mode| mode.kind() == GameKind::Duels);
            if duel_mode {
                let previous = state.bot_queues.get(bot_id).copied();
                state.bot_queues.insert(
                    bot_id.to_owned(),
                    duels::next_observation(previous, current, total),
                );
            } else if mode.is_some_and(|mode| mode.kind() == GameKind::Bedwars) {
                state
                    .bot_bedwars_queues
                    .insert(bot_id.to_owned(), bedwars::observe_queue(current, total));
            }
        }
        self.confirm_queue_candidates().await;
    }

    /// 依佇列條件收集候選，再啟動聊天／俯仰驗證或直接確認。
    /// @return 無回傳值；已在驗證的候選不重複發送。
    pub(crate) async fn confirm_queue_candidates(self: &Arc<Self>) {
        let candidates = {
            let state = self.state.lock().await;
            let Some(plan) = state.plan.as_ref() else {
                return;
            };
            let mode = plan.mode();
            let verify_presence = plan.requires_presence_verification();
            let verify_duel_pitch = plan.requires_pitch_verification();
            state
                .snapshot
                .bots
                .iter()
                .filter_map(|bot| {
                    if bot.phase != BotMatchPhase::Queued
                        || state.presence_checks.contains_key(&bot.bot_id)
                    {
                        return None;
                    }
                    let server = bot.server.as_deref()?;
                    let candidate = if mode.kind() == GameKind::Duels {
                        duels::is_candidate(
                            state.snapshot.player_server.as_deref(),
                            Some(server),
                            state.round_started_at,
                            state.player_queue,
                            state.bot_queues.get(&bot.bot_id).copied(),
                        )
                    } else if mode.kind() == GameKind::Bedwars {
                        bedwars::is_candidate(
                            state.snapshot.player_server.as_deref(),
                            Some(server),
                            state.round_started_at,
                            state.player_bedwars_queue,
                            state.bot_bedwars_queues.get(&bot.bot_id).copied(),
                        )
                    } else {
                        false
                    };
                    candidate.then_some((
                        bot.bot_id.clone(),
                        server.to_owned(),
                        verify_presence,
                        verify_duel_pitch,
                    ))
                })
                .collect::<Vec<_>>()
        };

        for (bot_id, server, verify_presence, verify_duel_pitch) in candidates {
            if verify_presence {
                self.start_presence_check(&bot_id, &server).await;
            } else if verify_duel_pitch {
                if self.has_duel_pitch_observation(&bot_id).await {
                    self.mark_matched(&bot_id, Some(&server)).await;
                } else {
                    self.start_duel_pitch_check(&bot_id, &server).await;
                }
            } else {
                self.mark_matched(&bot_id, Some(&server)).await;
            }
        }
    }

    /// 安排 Duels 佇列確認期限，僅仍有效的嘗試可重試。
    /// @param bot_id 本機 Bot ID。
    /// @param generation 目標世代。
    /// @param attempt_id 排程時的嘗試 ID。
    /// @param server 待確認的伺服器。
    /// @return 無回傳值；建立背景期限任務。
    pub(crate) fn spawn_duels_queue_confirmation_timeout(
        self: &Arc<Self>,
        bot_id: String,
        generation: u64,
        attempt_id: String,
        server: String,
    ) {
        let coordinator = Arc::downgrade(self);
        tokio::spawn(async move {
            tokio::time::sleep(duels::CONFIRM_TIMEOUT).await;
            let Some(coordinator) = coordinator.upgrade() else {
                return;
            };
            let should_retry = {
                let state = coordinator.state.lock().await;
                state.snapshot.phase == MatchmakingPhase::Matching
                    && state.target_generation == generation
                    && state
                        .active_attempts
                        .get(&bot_id)
                        .is_some_and(|current| current == &attempt_id)
                    && !duels::is_candidate(
                        state.snapshot.player_server.as_deref(),
                        Some(&server),
                        state.round_started_at,
                        state.player_queue,
                        state.bot_queues.get(&bot_id).copied(),
                    )
            };
            if should_retry {
                coordinator
                    .schedule_retry(
                        &bot_id,
                        Some(server),
                        "Queue count did not confirm",
                        Duration::ZERO,
                        true,
                    )
                    .await;
            }
        });
    }

    /// 安排 BedWars 佇列確認期限，已進入聊天驗證時不重複重試。
    /// @param bot_id 本機 Bot ID。
    /// @param generation 目標世代。
    /// @param attempt_id 排程時的嘗試 ID。
    /// @param server 待確認的伺服器。
    /// @return 無回傳值；建立背景期限任務。
    pub(crate) fn spawn_bedwars_queue_confirmation_timeout(
        self: &Arc<Self>,
        bot_id: String,
        generation: u64,
        attempt_id: String,
        server: String,
    ) {
        let coordinator = Arc::downgrade(self);
        tokio::spawn(async move {
            tokio::time::sleep(bedwars::CONFIRM_TIMEOUT).await;
            let Some(coordinator) = coordinator.upgrade() else {
                return;
            };
            let should_retry = {
                let state = coordinator.state.lock().await;
                state.snapshot.phase == MatchmakingPhase::Matching
                    && state.target_generation == generation
                    && state
                        .active_attempts
                        .get(&bot_id)
                        .is_some_and(|current| current == &attempt_id)
                    && !state.presence_checks.contains_key(&bot_id)
                    && !bedwars::is_candidate(
                        state.snapshot.player_server.as_deref(),
                        Some(&server),
                        state.round_started_at,
                        state.player_bedwars_queue,
                        state.bot_bedwars_queues.get(&bot_id).copied(),
                    )
            };
            if should_retry {
                coordinator
                    .schedule_retry(
                        &bot_id,
                        Some(server),
                        "Queue count did not confirm",
                        Duration::ZERO,
                        true,
                    )
                    .await;
            }
        });
    }

    /// 等待玩家日誌出現 Bot 名稱，逾時後重試有效嘗試。
    /// @param bot_id 本機 Bot ID。
    /// @param generation 目標世代。
    /// @param attempt_id 排程時的嘗試 ID。
    /// @return 無回傳值；建立背景期限任務。
    pub(crate) fn spawn_skywars_name_confirmation_timeout(
        self: &Arc<Self>,
        bot_id: String,
        generation: u64,
        attempt_id: String,
    ) {
        let coordinator = Arc::downgrade(self);
        tokio::spawn(async move {
            tokio::time::sleep(skywars::CONFIRM_TIMEOUT).await;
            let Some(coordinator) = coordinator.upgrade() else {
                return;
            };
            let should_retry = {
                let state = coordinator.state.lock().await;
                state
                    .plan
                    .as_ref()
                    .is_some_and(|plan| plan.mode().kind() == GameKind::Skywars)
                    && state.snapshot.phase == MatchmakingPhase::Matching
                    && state.target_generation == generation
                    && state
                        .active_attempts
                        .get(&bot_id)
                        .is_some_and(|current| current == &attempt_id)
            };
            if should_retry {
                coordinator
                    .schedule_retry(
                        &bot_id,
                        None,
                        "Bot name not found in player log",
                        Duration::ZERO,
                        true,
                    )
                    .await;
            }
        });
    }
}
