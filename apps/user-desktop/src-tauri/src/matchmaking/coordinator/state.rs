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

//! 集中每輪可變資料，區分整輪重置與單一 Bot 嘗試的清理範圍。

use super::super::{
    detector::duel_pitch,
    modes::{bedwars, duels::QueueObservation},
};
use super::super::{MatchmakingPlan, MatchmakingSnapshot};
use std::collections::{HashMap, HashSet};
use std::time::Instant;

/// 配對期間的所有可變資料，集中管理避免 Coordinator 持有零散狀態。
pub(crate) struct SessionState {
    pub(crate) snapshot: MatchmakingSnapshot,
    pub(crate) plan: Option<MatchmakingPlan>,
    pub(crate) target_generation: u64,
    pub(crate) active_attempts: HashMap<String, String>,
    pub(crate) retry_requests: HashMap<String, PendingRetry>,
    pub(crate) round_started_at: Option<Instant>,
    pub(crate) player_queue: Option<QueueObservation>,
    pub(crate) player_bedwars_queue: Option<bedwars::QueueObservation>,
    pub(crate) player_queue_usernames: HashSet<String>,
    pub(crate) bot_queues: HashMap<String, QueueObservation>,
    pub(crate) bot_bedwars_queues: HashMap<String, bedwars::QueueObservation>,
    pub(crate) bot_transfers: HashSet<String>,
    pub(crate) presence_checks: HashMap<String, PresenceCheck>,
    pub(crate) duel_pitch_checks: HashMap<String, duel_pitch::VerificationCheck>,
    pub(crate) duel_pitch_observations: HashMap<String, duel_pitch::Observation>,
}

/// 等待 Bot 完成 Limbo 準備的要求及其來源世代。
pub(crate) struct PendingRetry {
    pub(crate) request_id: String,
    pub(crate) generation: u64,
    pub(crate) retry_delay: std::time::Duration,
}

/// 聊天在場確認所需的名稱、伺服器與嘗試識別資訊。
pub(crate) struct PresenceCheck {
    pub(crate) username: String,
    pub(crate) generation: u64,
    pub(crate) attempt_id: String,
    pub(crate) server: String,
}

impl SessionState {
    /// 以空追蹤集合包裝初始快照。
    /// @param snapshot 初始配對快照。
    /// @return 尚未設定計畫的 SessionState。
    pub(crate) fn new(snapshot: MatchmakingSnapshot) -> Self {
        Self {
            snapshot,
            plan: None,
            target_generation: 0,
            active_attempts: HashMap::new(),
            retry_requests: HashMap::new(),
            round_started_at: None,
            player_queue: None,
            player_bedwars_queue: None,
            player_queue_usernames: HashSet::new(),
            bot_queues: HashMap::new(),
            bot_bedwars_queues: HashMap::new(),
            bot_transfers: HashSet::new(),
            presence_checks: HashMap::new(),
            duel_pitch_checks: HashMap::new(),
            duel_pitch_observations: HashMap::new(),
        }
    }

    /// 清除本輪追蹤並遞增世代，使舊 worker 結果失效。
    /// @return 無回傳值。
    pub(crate) fn reset_round(&mut self) {
        self.clear_round_tracking();
        // 舊 worker event 不可以在新 round 重新寫入目前 session。
        self.target_generation = self.target_generation.wrapping_add(1);
    }
    /// 清空所有 Bot 與玩家的本輪觀察、嘗試及驗證資料。
    /// @return 無回傳值；不自行改動快照或計畫。
    pub(crate) fn clear_round_tracking(&mut self) {
        // 每輪觀察資料必須完全隔離，否則上一輪結果可能令新一輪誤判為成功。
        self.active_attempts.clear();
        self.retry_requests.clear();
        self.round_started_at = None;
        self.player_queue = None;
        self.player_bedwars_queue = None;
        self.player_queue_usernames.clear();
        self.bot_queues.clear();
        self.bot_bedwars_queues.clear();
        self.bot_transfers.clear();
        self.presence_checks.clear();
        self.duel_pitch_checks.clear();
        self.duel_pitch_observations.clear();
    }

    /// 移除單一 Bot 的佇列、轉服與驗證觀察。
    /// @param bot_id 需重新嘗試的 Bot ID。
    /// @return 無回傳值；其他 Bot 與玩家觀察保留。
    pub(crate) fn clear_bot_attempt_tracking(&mut self, bot_id: &str) {
        self.bot_queues.remove(bot_id);
        self.bot_bedwars_queues.remove(bot_id);
        self.bot_transfers.remove(bot_id);
        self.presence_checks.remove(bot_id);
        self.duel_pitch_checks.remove(bot_id);
        self.duel_pitch_observations.remove(bot_id);
    }
}
