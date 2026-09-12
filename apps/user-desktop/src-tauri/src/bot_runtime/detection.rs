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

//! 轉換可見聊天訊號並追蹤穩定俯仰手勢，輸出前另行遮蔽敏感聊天內容。

use super::{BotGamePhase, DuelPitchDirection, GameKind};
use crate::hypixel::{parse_visible_chat, VisibleChatSignal};
use std::{collections::HashMap, time::Duration, time::Instant};

const PITCH_THRESHOLD_DEGREES: f32 = 80.0;
const PITCH_STABLE_DURATION: Duration = Duration::from_millis(250);
const PITCH_EMIT_COOLDOWN: Duration = Duration::from_secs(1);

/// 將共用聊天訊號限制為指定遊戲的開始／結束事件。
/// @param kind 目前配對的遊戲種類。
/// @param message 伺服器聊天文字。
/// @return 遊戲階段；無關訊息為 None。
pub(super) fn game_state(kind: GameKind, message: &str) -> Option<BotGamePhase> {
    match parse_visible_chat(message) {
        Some(VisibleChatSignal::GameStarted(observed_kind)) if observed_kind == kind => {
            Some(BotGamePhase::Started)
        }
        Some(VisibleChatSignal::DuelEnded) if kind == GameKind::Duels => Some(BotGamePhase::Ended),
        Some(VisibleChatSignal::BedwarsOrSkywarsEnded) if kind != GameKind::Duels => {
            Some(BotGamePhase::Ended)
        }
        _ => None,
    }
}

/// 辨識服務端的 Limbo 已生成確認。
/// @param message 可見聊天文字。
/// @return 確認訊號符合時為 true。
pub(super) fn limbo_spawn(message: &str) -> bool {
    matches!(
        parse_visible_chat(message),
        Some(VisibleChatSignal::LimboSpawned)
    )
}

/// 擷取已辨識的 mini 遊戲伺服器轉服訊號。
/// @param message 可見聊天文字。
/// @return 伺服器 ID；非轉服訊息為 None。
pub(super) fn server_transfer(message: &str) -> Option<String> {
    match parse_visible_chat(message) {
        Some(VisibleChatSignal::ServerTransfer(server)) => Some(server),
        _ => None,
    }
}

/// 取得共用解析器辨識的佇列人數。
/// @param message 可見聊天文字。
/// @return 目前人數及總容量；無效訊息為 None。
pub(super) fn queue_progress(message: &str) -> Option<(u32, u32)> {
    match parse_visible_chat(message) {
        Some(VisibleChatSignal::QueueProgress { current, total, .. }) => Some((current, total)),
        _ => None,
    }
}

/// 辨識服務端的指令頻率限制。
/// @param message 可見聊天文字。
/// @return 拒絕原因；無關訊息為 None。
pub(super) fn command_rejection(message: &str) -> Option<String> {
    match parse_visible_chat(message) {
        Some(VisibleChatSignal::CommandRejected(message)) => Some(message),
        _ => None,
    }
}

/// 過濾內部忙碌訊息，並遮蔽常見權杖及 Cookie 片段。
/// @param message 待顯示的原始聊天文字。
/// @return 可見文字或 REDACTED；空白及內部訊息為 None。
pub(super) fn safe_chat(message: &str) -> Option<String> {
    let plain = collapse_whitespace(message);
    if plain.eq_ignore_ascii_case("You are currently BUSY") {
        return None;
    }
    let normalized = plain.to_ascii_lowercase();
    if normalized.contains("bearer ")
        || normalized.contains("accesstoken")
        || normalized.contains("refreshtoken")
        || normalized.contains("cookie=")
        || plain.contains("eyJ")
        || normalized.contains("bott-")
    {
        return Some("[REDACTED]".to_owned());
    }
    (!plain.is_empty()).then_some(plain)
}

/// 移除斷線訊息中的空行、網址及 Ban ID 說明。
/// @param message 服務端提供的可選斷線原因。
/// @return 供 UI 顯示的單行原因。
pub(super) fn disconnect_reason(message: Option<&str>) -> String {
    let Some(message) = message else {
        return "Disconnected by server".to_owned();
    };
    let visible = message
        .lines()
        .map(str::trim)
        .filter(|line| {
            !line.is_empty()
                && !line.starts_with("http://")
                && !line.starts_with("https://")
                && !line.starts_with("Find out more:")
                && !line.starts_with("Ban ID:")
                && !line.starts_with("Sharing your Ban ID")
        })
        .collect::<Vec<_>>()
        .join(" ");
    if visible.is_empty() {
        "Disconnected by server".to_owned()
    } else {
        visible
    }
}

/// 將封禁與一般踢出分類為固定錯誤代碼。
/// @param message 已整理的斷線原因。
/// @return banned 或 kicked。
pub(super) fn disconnect_code(message: &str) -> &'static str {
    let normalized = message.to_ascii_lowercase();
    if normalized.contains("banned") || normalized.contains("ban id") {
        "banned"
    } else {
        "kicked"
    }
}

/// 按實體 ID 記住中立角度、手勢穩定時間及發送冷卻。
pub(super) struct PitchTracker {
    states: HashMap<i32, PitchState>,
}

struct PitchState {
    saw_neutral: bool,
    direction: Option<DuelPitchDirection>,
    active_since: Option<Instant>,
    last_emitted_at: Option<Instant>,
}

impl PitchTracker {
    /// 建立沒有玩家觀察資料的俯仰追蹤器。
    /// @return 空的 PitchTracker。
    pub(super) fn new() -> Self {
        Self {
            states: HashMap::new(),
        }
    }

    /// 清除所有實體的觀察與冷卻狀態。
    /// @return 無回傳值；下一輪需重新觀察中立角度。
    pub(super) fn reset(&mut self) {
        self.states.clear();
    }

    /// 要求先觀察中立角度，再確認持續的俯仰手勢。
    /// @param entity_id Minecraft 實體 ID。
    /// @param pitch 俯仰角，單位為度。
    /// @param now 本次觀察的單調時間。
    /// @return 已確認的上下方向；未穩定或冷卻中為 None。
    pub(super) fn observe(
        &mut self,
        entity_id: i32,
        pitch: f32,
        now: Instant,
    ) -> Option<DuelPitchDirection> {
        let direction = pitch_direction(pitch);
        let state = self.states.entry(entity_id).or_insert(PitchState {
            saw_neutral: false,
            direction: None,
            active_since: None,
            last_emitted_at: None,
        });

        let Some(direction) = direction else {
            state.saw_neutral = true;
            state.direction = None;
            state.active_since = None;
            return None;
        };
        if !state.saw_neutral {
            return None;
        }
        if state.direction != Some(direction) {
            state.direction = Some(direction);
            state.active_since = Some(now);
            return None;
        }
        if state
            .active_since
            .is_none_or(|started| now.duration_since(started) < PITCH_STABLE_DURATION)
            || state
                .last_emitted_at
                .is_some_and(|last| now.duration_since(last) < PITCH_EMIT_COOLDOWN)
        {
            return None;
        }

        state.last_emitted_at = Some(now);
        Some(direction)
    }
}

fn pitch_direction(pitch: f32) -> Option<DuelPitchDirection> {
    if !pitch.is_finite() || pitch.abs() <= PITCH_THRESHOLD_DEGREES {
        None
    } else if pitch < 0.0 {
        Some(DuelPitchDirection::Up)
    } else {
        Some(DuelPitchDirection::Down)
    }
}

fn collapse_whitespace(message: &str) -> String {
    message.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_only_the_selected_game_start_markers() {
        assert_eq!(
            game_state(GameKind::Duels, "Opponent: Player"),
            Some(BotGamePhase::Started)
        );
        assert_eq!(
            game_state(GameKind::Skywars, "Cages opened! FIGHT!"),
            Some(BotGamePhase::Started)
        );
        assert_eq!(
            game_state(
                GameKind::Bedwars,
                "Protect your bed and destroy the enemy beds."
            ),
            Some(BotGamePhase::Started)
        );
        assert_eq!(game_state(GameKind::Skywars, "The game has started"), None);
    }

    #[test]
    fn detects_limbo_spawn_confirmation_from_chat() {
        assert!(limbo_spawn("You were spawned in Limbo."));
        assert!(limbo_spawn("YOU WERE SPAWNED IN LIMBO"));
        assert!(!limbo_spawn("/limbo for more information."));
    }

    #[test]
    fn parses_transfer_and_queue_messages() {
        assert_eq!(
            server_transfer("Sending you to mini198Q!"),
            Some("mini198Q".to_owned())
        );
        assert_eq!(queue_progress("Player has joined (2/8)!"), Some((2, 8)));
    }

    #[test]
    fn pitch_requires_neutral_then_a_stable_gesture() {
        let start = Instant::now();
        let mut tracker = PitchTracker::new();
        assert_eq!(tracker.observe(1, 0.0, start), None);
        assert_eq!(tracker.observe(1, -85.0, start), None);
        assert_eq!(
            tracker.observe(1, -85.0, start + PITCH_STABLE_DURATION),
            Some(DuelPitchDirection::Up)
        );
    }

    #[test]
    fn redacts_sensitive_chat() {
        assert_eq!(safe_chat("Bearer secret"), Some("[REDACTED]".to_owned()));
        assert_eq!(safe_chat("You are currently BUSY"), None);
    }

    #[test]
    fn classifies_ban_disconnects_separately_from_other_kicks() {
        assert_eq!(
            disconnect_code("You are permanently banned from this server!"),
            "banned"
        );
        assert_eq!(disconnect_code("Kicked by an administrator"), "kicked");
    }
}
