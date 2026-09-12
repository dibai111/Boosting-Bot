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

//! 定義 Bot 指令、事件及遊戲模式；序列化名稱是與前端溝通的固定契約。

use super::nick_roller::{NickRollerConfig, NickRollerPhase};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
/// 支援的 Hypixel 遊戲模式；serde 名稱為前後端固定契約。
pub(crate) enum GameMode {
    #[serde(rename = "solo")]
    BedwarsSolo,
    #[serde(rename = "doubles")]
    BedwarsDoubles,
    #[serde(rename = "threes")]
    BedwarsThrees,
    #[serde(rename = "fours")]
    BedwarsFours,
    #[serde(rename = "four_v_four")]
    BedwarsFourVFour,
    #[serde(rename = "duels_uhc")]
    DuelsUhc,
    #[serde(rename = "duels_classic")]
    DuelsClassic,
    #[serde(rename = "duels_skywars")]
    DuelsSkywars,
    #[serde(rename = "duels_combo")]
    DuelsCombo,
    #[serde(rename = "duels_bow")]
    DuelsBow,
    #[serde(rename = "duels_nodebuff")]
    DuelsNoDebuff,
    #[serde(rename = "duels_sumo")]
    DuelsSumo,
    #[serde(rename = "duels_blitz")]
    DuelsBlitz,
    #[serde(rename = "duels_op")]
    DuelsOp,
    #[serde(rename = "duels_mega_walls")]
    DuelsMegaWalls,
    #[serde(rename = "duels_bow_spleef")]
    DuelsBowSpleef,
    #[serde(rename = "duels_bridge")]
    DuelsBridge,
    #[serde(rename = "duels_bedwars")]
    DuelsBedwars,
    #[serde(rename = "duels_bed_rush")]
    DuelsBedRush,
    #[serde(rename = "duels_boxing")]
    DuelsBoxing,
    #[serde(rename = "duels_quakecraft")]
    DuelsQuakecraft,
    #[serde(rename = "skywars_solo_normal")]
    SkywarsSoloNormal,
    #[serde(rename = "skywars_solo_insane")]
    SkywarsSoloInsane,
    #[serde(rename = "skywars_doubles_normal")]
    SkywarsDoublesNormal,
    #[serde(rename = "skywars_doubles_insane")]
    SkywarsDoublesInsane,
    #[serde(rename = "skywars_solo_lucky")]
    SkywarsSoloLucky,
    #[serde(rename = "skywars_doubles_lucky")]
    SkywarsDoublesLucky,
    #[serde(rename = "skywars_mini_normal")]
    SkywarsMiniNormal,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
/// 模式所屬遊戲，用於選取配對與驗證策略。
pub(crate) enum GameKind {
    Bedwars,
    Duels,
    Skywars,
}

impl GameMode {
    /// 將具體遊戲模式對應至策略種類。
    /// @return Bedwars、Duels 或 Skywars。
    pub(crate) const fn kind(self) -> GameKind {
        match self {
            Self::BedwarsSolo
            | Self::BedwarsDoubles
            | Self::BedwarsThrees
            | Self::BedwarsFours
            | Self::BedwarsFourVFour => GameKind::Bedwars,
            Self::DuelsUhc
            | Self::DuelsClassic
            | Self::DuelsSkywars
            | Self::DuelsCombo
            | Self::DuelsBow
            | Self::DuelsNoDebuff
            | Self::DuelsSumo
            | Self::DuelsBlitz
            | Self::DuelsOp
            | Self::DuelsMegaWalls
            | Self::DuelsBowSpleef
            | Self::DuelsBridge
            | Self::DuelsBedwars
            | Self::DuelsBedRush
            | Self::DuelsBoxing
            | Self::DuelsQuakecraft => GameKind::Duels,
            Self::SkywarsSoloNormal
            | Self::SkywarsSoloInsane
            | Self::SkywarsDoublesNormal
            | Self::SkywarsDoublesInsane
            | Self::SkywarsSoloLucky
            | Self::SkywarsDoublesLucky
            | Self::SkywarsMiniNormal => GameKind::Skywars,
        }
    }

    /// 取得伺服器辨識的固定遊戲佇列指令。
    /// @return 以 /play 開頭的靜態字串。
    pub(crate) const fn play_command(self) -> &'static str {
        match self {
            Self::BedwarsSolo => "/play bedwars_eight_one",
            Self::BedwarsDoubles => "/play bedwars_eight_two",
            Self::BedwarsThrees => "/play bedwars_four_three",
            Self::BedwarsFours => "/play bedwars_four_four",
            Self::BedwarsFourVFour => "/play bedwars_two_four",
            Self::DuelsUhc => "/play duels_uhc_duel",
            Self::DuelsClassic => "/play duels_classic_duel",
            Self::DuelsSkywars => "/play duels_sw_duel",
            Self::DuelsCombo => "/play duels_combo_duel",
            Self::DuelsBow => "/play duels_bow_duel",
            Self::DuelsNoDebuff => "/play duels_potion_duel",
            Self::DuelsSumo => "/play duels_sumo_duel",
            Self::DuelsBlitz => "/play duels_blitz_duel",
            Self::DuelsOp => "/play duels_op_duel",
            Self::DuelsMegaWalls => "/play duels_mw_duel",
            Self::DuelsBowSpleef => "/play duels_bowspleef_duel",
            Self::DuelsBridge => "/play duels_bridge_duel",
            Self::DuelsBedwars => "/play bedwars_two_one_duels",
            Self::DuelsBedRush => "/play bedwars_two_one_duels_rush",
            Self::DuelsBoxing => "/play duels_boxing_duel",
            Self::DuelsQuakecraft => "/play duels_quake_duel",
            Self::SkywarsSoloNormal => "/play solo_normal",
            Self::SkywarsSoloInsane => "/play solo_insane",
            Self::SkywarsDoublesNormal => "/play teams_normal",
            Self::SkywarsDoublesInsane => "/play teams_insane",
            Self::SkywarsSoloLucky => "/play solo_insane_lucky",
            Self::SkywarsDoublesLucky => "/play teams_insane_lucky",
            Self::SkywarsMiniNormal => "/play mini_normal",
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
/// 單一 Minecraft 連線的生命週期階段。
pub(crate) enum BotPhase {
    Offline,
    Starting,
    Authenticating,
    Connecting,
    Online,
    Stopping,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
/// 應用層互斥的閒置、配對與 Nick 篩選模式。
pub(crate) enum RuntimeMode {
    Idle,
    Matching,
    NickRoller,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
/// Bot 所見的遊戲開始或結束訊號。
pub(crate) enum BotGamePhase {
    Started,
    Ended,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
/// 玩家確認手勢的上下俯仰方向。
pub(crate) enum DuelPitchDirection {
    Up,
    Down,
}

impl DuelPitchDirection {
    /// 取得俯仰方向的診斷名稱。
    /// @return up 或 down。
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::Up => "up",
            Self::Down => "down",
        }
    }
}

/// 完成登入驗證後的連線設定；access token 不對前端序列化。
pub(crate) struct BotConfig {
    pub(crate) bot_id: String,
    pub(crate) username: String,
    pub(crate) uuid: Uuid,
    pub(crate) access_token: String,
    pub(crate) server_address: String,
}

#[derive(Debug, Clone)]
/// 一次配對嘗試的來源識別碼及驗證策略。
pub(crate) struct MatchAttempt {
    pub(crate) session_id: String,
    pub(crate) round_id: String,
    pub(crate) target_generation: u64,
    pub(crate) attempt_id: String,
    pub(crate) mode: GameMode,
    pub(crate) requires_pitch_verification: bool,
}

#[derive(Debug, Clone)]
/// 交給 SessionActor 執行的聊天、配對及 Nick 指令。
pub(crate) enum BotCommand {
    SendChat(String),
    BeginMatchAttempt(MatchAttempt),
    ReturnToLobby,
    PrepareMatchRetry { request_id: String },
    CancelMatchAttempt,
    StartNickRoller { config: NickRollerConfig },
    StopNickRoller,
    NickDecision { candidate_id: u64, take: bool },
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
/// 對前端及配對協調器發布的 tagged union 事件契約。
pub(crate) enum BotEvent {
    RuntimeMode {
        mode: RuntimeMode,
    },
    Status {
        bot_id: String,
        phase: BotPhase,
        message: Option<String>,
    },
    Profile {
        bot_id: String,
        username: String,
        uuid: Option<String>,
    },
    DeviceCode {
        request_id: String,
        user_code: String,
        verification_uri: String,
        expires_in: Option<u64>,
    },
    MicrosoftAuthResult {
        request_id: String,
        username: Option<String>,
        uuid: Option<String>,
        error: Option<String>,
    },
    NickRollerState {
        bot_id: String,
        phase: NickRollerPhase,
        message: Option<String>,
    },
    NickCandidate {
        bot_id: String,
        candidate_id: u64,
        nick: String,
        accepted: bool,
        reasons: Vec<String>,
        processed_count: u32,
        accepted_count: u32,
        rejected_count: u32,
        decision_timeout_ms: Option<u64>,
    },
    NickVerification {
        bot_id: String,
        candidate_id: u64,
        expected_nick: String,
        actual_nick: Option<String>,
        success: bool,
        reason: String,
        processed_count: u32,
    },
    NickAttention {
        bot_id: String,
        code: String,
        message: String,
    },
    AfkState {
        bot_id: String,
        active: bool,
    },
    MatchAttemptResult {
        bot_id: String,
        session_id: String,
        round_id: String,
        target_generation: u64,
        attempt_id: String,
        server: String,
    },
    MatchAttemptFailed {
        bot_id: String,
        session_id: String,
        round_id: String,
        target_generation: u64,
        attempt_id: String,
        code: String,
        message: String,
    },
    MatchRetryReady {
        bot_id: String,
        request_id: String,
    },
    MatchRetryPreparationFailed {
        bot_id: String,
        request_id: String,
        message: String,
    },
    QueueProgress {
        bot_id: String,
        current: u32,
        total: u32,
    },
    DuelPitchObserved {
        bot_id: String,
        round_id: String,
        target_generation: u64,
        attempt_id: String,
        direction: DuelPitchDirection,
        pitch: f32,
    },
    MatchmakingDebug {
        bot_id: String,
        message: String,
    },
    BotGameState {
        bot_id: String,
        round_id: String,
        target_generation: u64,
        state: BotGamePhase,
    },
    ChatMessage {
        bot_id: String,
        message: String,
    },
    Error {
        bot_id: Option<String>,
        code: String,
        message: String,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_mode_family_and_play_command_together() {
        assert_eq!(GameMode::BedwarsDoubles.kind(), GameKind::Bedwars);
        assert_eq!(GameMode::DuelsSumo.kind(), GameKind::Duels);
        assert_eq!(GameMode::SkywarsMiniNormal.kind(), GameKind::Skywars);
        assert_eq!(GameMode::DuelsSumo.play_command(), "/play duels_sumo_duel");
    }
}
