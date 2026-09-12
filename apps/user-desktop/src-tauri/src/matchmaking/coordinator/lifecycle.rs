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

//! 處理遊戲開始、結束、Bot 離線及回到大廳時的工作階段轉換。

use super::super::{BotMatchPhase, MatchmakingPhase};
use super::flow::{matched_count, possible_matches, selected_bot_ids, waiting_bot};
use super::MatchmakingSession;
use crate::bot_runtime::{BotCommand, BotGamePhase};

impl MatchmakingSession {
    /// 玩家回大廳且已有目標時，清除本輪配對。
    /// @return 無回傳值。
    pub(crate) async fn handle_player_lobby_joined(&self) {
        let has_active_target = self.state.lock().await.snapshot.player_server.is_some();
        if has_active_target {
            self.reset_for_lobby("Player returned to the lobby").await;
        }
    }

    /// 只接受目前輪次中已配對 Bot 的遊戲階段訊號。
    /// @param bot_id 本機 Bot ID。
    /// @param round_id 事件輪次 ID。
    /// @param generation 事件目標世代。
    /// @param game_state 遊戲開始或結束。
    /// @return 無回傳值；無效來源忽略。
    pub(crate) async fn handle_bot_game_state(
        &self,
        bot_id: &str,
        round_id: &str,
        generation: u64,
        game_state: BotGamePhase,
    ) {
        let accepted = {
            let state = self.state.lock().await;
            state.snapshot.round_id.as_deref() == Some(round_id)
                && state.target_generation == generation
                && state.snapshot.bots.iter().any(|bot| {
                    bot.bot_id == bot_id
                        && matches!(bot.phase, BotMatchPhase::Matched | BotMatchPhase::Afk)
                })
        };
        if !accepted {
            return;
        }
        match game_state {
            BotGamePhase::Started => self.handle_game_started().await,
            BotGamePhase::Ended => self.reset_for_lobby("Game ended").await,
        }
    }

    /// 遊戲開始時檢查最低配對數，成功的 Bot 進入 AFK。
    /// @return 無回傳值；數量不足則使本輪失敗。
    pub(crate) async fn handle_game_started(&self) {
        let (should_fail, snapshot) = {
            let mut state = self.state.lock().await;
            if !matches!(
                state.snapshot.phase,
                MatchmakingPhase::Matching | MatchmakingPhase::Committed
            ) {
                return;
            }
            let matched = state
                .snapshot
                .bots
                .iter()
                .filter(|bot| bot.phase == BotMatchPhase::Matched)
                .count();
            let should_fail = matched < state.snapshot.required_matches;
            if !should_fail {
                state.snapshot.phase = MatchmakingPhase::InGame;
                state.snapshot.message = Some("Game started".to_owned());
                for bot in &mut state.snapshot.bots {
                    if bot.phase == BotMatchPhase::Matched {
                        bot.phase = BotMatchPhase::Afk;
                    }
                }
            }
            (should_fail, state.snapshot.clone())
        };
        if should_fail {
            self.fail_round("Game started before the minimum bots matched")
                .await;
            return;
        }
        self.publish(snapshot);
    }

    /// 清除指定 Bot 的嘗試追蹤，更新摘要及可達成數量。
    /// @param bot_id 本機 Bot ID。
    /// @param message 不可用的原因。
    /// @return 無回傳值；不足以達標時結束本輪。
    pub(crate) async fn mark_unavailable(&self, bot_id: &str, message: &str) {
        let (should_fail, snapshot) = {
            let mut state = self.state.lock().await;
            if !matches!(
                state.snapshot.phase,
                MatchmakingPhase::Matching | MatchmakingPhase::Committed | MatchmakingPhase::InGame
            ) {
                return;
            }
            state.active_attempts.remove(bot_id);
            state.retry_requests.remove(bot_id);
            state.clear_bot_attempt_tracking(bot_id);
            let Some(bot) = state
                .snapshot
                .bots
                .iter_mut()
                .find(|bot| bot.bot_id == bot_id)
            else {
                return;
            };
            bot.phase = BotMatchPhase::Unavailable;
            bot.message = Some(message.to_owned());
            state.snapshot.matched_bots = matched_count(&state.snapshot);
            let should_fail = state.snapshot.phase == MatchmakingPhase::Matching
                && possible_matches(&state.snapshot) < state.snapshot.required_matches;
            (should_fail, state.snapshot.clone())
        };
        self.publish(snapshot);
        if should_fail {
            self.fail_round("Not enough available bots to reach the minimum")
                .await;
        }
    }

    /// 使目前輪次失敗、清零已配對數量並撤回所有可用 Bot。
    /// @param message 供快照及日誌顯示的失敗原因。
    /// @return 無回傳值；未啟動計畫時忽略。
    pub(crate) async fn fail_round(&self, message: impl Into<String>) {
        let bot_ids = {
            let mut state = self.state.lock().await;
            if state.plan.is_none() {
                return;
            }
            state.reset_round();
            state.snapshot.phase = MatchmakingPhase::Failed;
            // 所有已匹配的 Bot 都會撤回，摘要數量也必須同步清零。
            state.snapshot.matched_bots = 0;
            state.snapshot.message = Some(message.into());
            for bot in &mut state.snapshot.bots {
                if bot.phase != BotMatchPhase::Unavailable {
                    bot.phase = BotMatchPhase::Returning;
                }
            }
            let ids = selected_bot_ids(&state);
            self.publish(state.snapshot.clone());
            ids
        };
        self.withdraw_bots(&bot_ids).await;
    }

    /// 使舊世代失效，恢復等待玩家轉服的狀態。
    /// @param message 此次重設的可見原因。
    /// @return 無回傳值；有進行中的輪次時撤回 Bot。
    pub(crate) async fn reset_for_lobby(&self, message: &str) {
        let bot_ids = {
            let mut state = self.state.lock().await;
            if state.plan.is_none() {
                return;
            }
            let active_round = state.snapshot.round_id.is_some()
                || !matches!(state.snapshot.phase, MatchmakingPhase::AwaitingPlayer);
            state.reset_round();
            state.snapshot.round_id = None;
            state.snapshot.phase = MatchmakingPhase::AwaitingPlayer;
            state.snapshot.player_server = None;
            state.snapshot.matched_bots = 0;
            state.snapshot.message = Some(message.to_owned());
            for bot in &mut state.snapshot.bots {
                *bot = waiting_bot(&bot.bot_id);
            }
            let ids = if active_round {
                selected_bot_ids(&state)
            } else {
                Vec::new()
            };
            self.publish(state.snapshot.clone());
            ids
        };
        self.withdraw_bots(&bot_ids).await;
    }

    /// 依序取消指定 Bot 的嘗試並要求回到大廳。
    /// @param bot_ids 需要撤回的 Bot ID。
    /// @return 無回傳值；已離線的通道錯誤忽略。
    pub(crate) async fn withdraw_bots(&self, bot_ids: &[String]) {
        for bot_id in bot_ids {
            let _ = self
                .bots
                .command(bot_id, BotCommand::CancelMatchAttempt)
                .await;
            let _ = self.bots.command(bot_id, BotCommand::ReturnToLobby).await;
        }
    }
}
