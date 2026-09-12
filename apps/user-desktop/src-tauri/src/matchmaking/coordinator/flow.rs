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

//! 提供配對計數、撤回條件及重試訊息等無副作用的共用判斷。

use super::super::{BotMatchPhase, MatchmakingBotState, MatchmakingSnapshot};
use super::state::SessionState;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// 為服務端指令節流選取短暫抖動，降低連續重試同步發生的機會。
/// @return 800 至 1000 毫秒的等待時間，非密碼學亂數。
pub(super) fn random_command_spam_retry_delay() -> Duration {
    let tick = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    const MIN_MS: u64 = 800;
    const RANGE_MS: u128 = 201;
    Duration::from_millis(MIN_MS + (tick % RANGE_MS) as u64)
}

/// 將重試原因與等待時間組成可見文字。
/// @param message 原始重試原因。
/// @param delay 再次排隊前的等待時間。
/// @return 立即或延遲重試的說明。
pub(super) fn retry_message(message: &str, delay: Duration) -> String {
    if delay.is_zero() {
        format!("{message}; retrying now")
    } else {
        format!("{message}; retrying in {:.2} seconds", delay.as_secs_f32())
    }
}

/// 建立尚未嘗試配對的 Bot 快照。
/// @param bot_id 本機 Bot ID。
/// @return Waiting 階段且嘗試數為零的狀態。
pub(super) fn waiting_bot(bot_id: &str) -> MatchmakingBotState {
    MatchmakingBotState {
        bot_id: bot_id.to_owned(),
        phase: BotMatchPhase::Waiting,
        server: None,
        attempts: 0,
        message: None,
    }
}

/// 計算已匹配或已進入 AFK 的 Bot。
/// @param snapshot 目前配對快照。
/// @return 已達成配對數量。
pub(super) fn matched_count(snapshot: &MatchmakingSnapshot) -> usize {
    snapshot
        .bots
        .iter()
        .filter(|bot| matches!(bot.phase, BotMatchPhase::Matched | BotMatchPhase::Afk))
        .count()
}

/// 計算仍可能配對的 Bot，排除 Unavailable。
/// @param snapshot 目前配對快照。
/// @return 可用候選數量。
pub(super) fn possible_matches(snapshot: &MatchmakingSnapshot) -> usize {
    snapshot
        .bots
        .iter()
        .filter(|bot| bot.phase != BotMatchPhase::Unavailable)
        .count()
}

/// 判斷達到最低數量後是否應撤回此 Bot。
/// @param phase Bot 目前配對階段。
/// @return 仍等待、排隊或返回中的 Bot 為 true。
pub(super) fn should_withdraw_after_commit(phase: BotMatchPhase) -> bool {
    matches!(
        phase,
        BotMatchPhase::Waiting | BotMatchPhase::Queued | BotMatchPhase::Returning
    )
}

/// 從不可變計畫取得原始參與清單。
/// @param state 目前工作階段狀態。
/// @return 選定 ID 副本；沒有計畫時為空。
pub(super) fn selected_bot_ids(state: &SessionState) -> Vec<String> {
    state
        .plan
        .as_ref()
        .map(|plan| plan.bot_ids().map(str::to_owned).collect())
        .unwrap_or_default()
}
