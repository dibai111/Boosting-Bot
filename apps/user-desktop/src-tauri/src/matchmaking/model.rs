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

//! 定義配對請求、Bot 狀態與快照，作為前後端共用的傳輸契約。

use crate::bot_runtime::GameMode;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
/// 整個配對工作階段的可見階段。
pub(crate) enum MatchmakingPhase {
    Idle,
    AwaitingPlayer,
    Matching,
    Committed,
    InGame,
    Failed,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
/// 單一 Bot 在配對輪次中的階段。
pub(crate) enum BotMatchPhase {
    Waiting,
    Queued,
    Matched,
    Returning,
    Afk,
    Unavailable,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
/// 前端配對請求；玩家名稱由後端儲存快照補足。
pub(crate) struct StartMatchmakingInput {
    pub(crate) mode: GameMode,
    pub(crate) bot_ids: Vec<String>,
    #[serde(default = "default_verify_presence")]
    pub(crate) verify_presence: bool,
    #[serde(default = "default_verify_duel_pitch")]
    pub(crate) verify_duel_pitch: bool,
    pub(crate) required_matches: usize,
    pub(crate) log_path: String,
}

fn default_verify_presence() -> bool {
    true
}

fn default_verify_duel_pitch() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize)]
/// 單一 Bot 的伺服器、嘗試數與進度快照。
pub(crate) struct MatchmakingBotState {
    pub(crate) bot_id: String,
    pub(crate) phase: BotMatchPhase,
    pub(crate) server: Option<String>,
    pub(crate) attempts: u32,
    pub(crate) message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
/// 供 UI 及浮動視窗訂閱的完整配對進度。
pub(crate) struct MatchmakingSnapshot {
    pub(crate) session_id: Option<String>,
    pub(crate) round_id: Option<String>,
    pub(crate) phase: MatchmakingPhase,
    pub(crate) mode: Option<GameMode>,
    pub(crate) player_server: Option<String>,
    pub(crate) required_matches: usize,
    pub(crate) matched_bots: usize,
    pub(crate) bots: Vec<MatchmakingBotState>,
    pub(crate) message: Option<String>,
}

impl MatchmakingSnapshot {
    /// 建立沒有計畫及 Bot 的空閒快照。
    /// @return 初始 MatchmakingSnapshot。
    pub(crate) fn idle() -> Self {
        Self {
            session_id: None,
            round_id: None,
            phase: MatchmakingPhase::Idle,
            mode: None,
            player_server: None,
            required_matches: 0,
            matched_bots: 0,
            bots: Vec::new(),
            message: None,
        }
    }
}
