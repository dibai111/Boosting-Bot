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

//! 解析共用的 Hypixel 聊天訊號，讓 Bot 事件與玩家記錄使用相同的判斷規則。

use crate::bot_runtime::GameKind;

/// 玩家可見聊天文字中的共用 Hypixel 訊號，不包含來源格式或敏感資料處理。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum VisibleChatSignal {
    ServerTransfer(String),
    QueueProgress {
        username: String,
        current: u32,
        total: u32,
    },
    GameStarted(GameKind),
    DuelEnded,
    BedwarsOrSkywarsEnded,
    LimboSpawned,
    CommandRejected(String),
}

/// 一次正規化聊天文字，辨識共用遊戲與佇列訊號。
/// @param message Minecraft 可見聊天內容，不包含日誌前綴。
/// @return 第一個符合的訊號；無關或空白訊息為 None。
pub(crate) fn parse_visible_chat(message: &str) -> Option<VisibleChatSignal> {
    let plain = collapse_whitespace(message);
    if plain.is_empty() {
        return None;
    }
    let normalized = normalize_message(&plain);
    if normalized == "you were spawned in limbo" {
        return Some(VisibleChatSignal::LimboSpawned);
    }
    if let Some(server) = parse_transfer(&plain) {
        return Some(VisibleChatSignal::ServerTransfer(server));
    }
    if let Some((username, current, total)) = parse_queue_progress(&plain) {
        return Some(VisibleChatSignal::QueueProgress {
            username,
            current,
            total,
        });
    }
    if let Some(kind) = game_started(&normalized) {
        return Some(VisibleChatSignal::GameStarted(kind));
    }
    if game_ended(&normalized) {
        return Some(VisibleChatSignal::BedwarsOrSkywarsEnded);
    }
    if normalized.contains("reward summary") {
        return Some(VisibleChatSignal::DuelEnded);
    }
    command_rejection(&plain).map(VisibleChatSignal::CommandRejected)
}

/// 識別 mini 後接數字及英數字尾碼的遊戲伺服器。
/// @param server 待判斷的伺服器識別字串。
/// @return 符合遊戲伺服器格式時為 true。
pub(crate) fn is_game_server(server: &str) -> bool {
    server.get(..4).is_some_and(|prefix| {
        prefix.eq_ignore_ascii_case("mini")
            && server.get(4..).is_some_and(|suffix| {
                suffix.starts_with(|character: char| character.is_ascii_digit())
                    && suffix
                        .chars()
                        .all(|character| character.is_ascii_alphanumeric())
            })
    })
}

/// 以不區分 ASCII 大小寫方式比對伺服器 ID。
/// @param target 玩家目標伺服器。
/// @param candidate Bot 觀察到的伺服器。
/// @return 相同時為 true。
pub(crate) fn server_matches(target: &str, candidate: &str) -> bool {
    target.eq_ignore_ascii_case(candidate)
}

fn parse_transfer(message: &str) -> Option<String> {
    const PREFIX: &str = "Sending you to ";
    let prefix = message.get(..PREFIX.len())?;
    if !prefix.eq_ignore_ascii_case(PREFIX) {
        return None;
    }
    let server = message[PREFIX.len()..]
        .trim_end_matches(|character: char| character.is_ascii_punctuation())
        .trim();
    let suffix = server.get(4..)?;
    is_game_server(server).then(|| format!("mini{suffix}"))
}

fn parse_queue_progress(message: &str) -> Option<(String, u32, u32)> {
    const MARKER: &str = " has joined (";
    let lowercase = message.to_ascii_lowercase();
    let marker_start = lowercase.find(MARKER)?;
    let values_start = marker_start + MARKER.len();
    let values_end = lowercase[values_start..].find(')')? + values_start;
    let (current, total) = lowercase[values_start..values_end].split_once('/')?;
    let current = current.trim().parse().ok()?;
    let total = total.trim().parse().ok()?;
    let username = message[..marker_start]
        .split_whitespace()
        .last()?
        .to_owned();
    (total > 0
        && current <= total
        && username
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || character == '_'))
    .then_some((username, current, total))
}

fn game_started(normalized: &str) -> Option<GameKind> {
    if normalized.starts_with("opponent") {
        Some(GameKind::Duels)
    } else if normalized.starts_with("cages opened fight") {
        Some(GameKind::Skywars)
    } else if normalized.starts_with("protect your bed and destroy the enemy beds") {
        Some(GameKind::Bedwars)
    } else {
        None
    }
}

fn game_ended(normalized: &str) -> bool {
    normalized.starts_with("victory")
        || normalized.starts_with("game over")
        || ["1st killer", "2nd killer", "3rd killer"]
            .iter()
            .any(|marker| normalized.starts_with(marker))
}

fn command_rejection(message: &str) -> Option<String> {
    let normalized = message.to_ascii_lowercase();
    (normalized.contains("please don't spam the command")
        || normalized.contains("please do not spam the command")
        || normalized.contains("sending commands too fast"))
    .then(|| message.to_owned())
}

fn normalize_message(message: &str) -> String {
    let without_punctuation = message
        .chars()
        .map(|character| {
            if character.is_ascii_punctuation() {
                ' '
            } else {
                character
            }
        })
        .collect::<String>();
    collapse_whitespace(&without_punctuation).to_ascii_lowercase()
}

fn collapse_whitespace(message: &str) -> String {
    message.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_the_shared_transfer_and_queue_rules() {
        assert_eq!(
            parse_visible_chat("Sending you to MINI198Q!"),
            Some(VisibleChatSignal::ServerTransfer("mini198Q".to_owned()))
        );
        assert_eq!(
            parse_visible_chat("[MVP+] Player_1 has joined (2/8)!"),
            Some(VisibleChatSignal::QueueProgress {
                username: "Player_1".to_owned(),
                current: 2,
                total: 8,
            })
        );
        assert_eq!(parse_visible_chat("Sending you to miniABC!"), None);
    }

    #[test]
    fn recognizes_the_three_explicit_game_start_markers() {
        assert_eq!(
            parse_visible_chat("Opponent: Player"),
            Some(VisibleChatSignal::GameStarted(GameKind::Duels))
        );
        assert_eq!(
            parse_visible_chat("Cages opened! FIGHT!"),
            Some(VisibleChatSignal::GameStarted(GameKind::Skywars))
        );
        assert_eq!(
            parse_visible_chat("Protect your bed and destroy the enemy beds."),
            Some(VisibleChatSignal::GameStarted(GameKind::Bedwars))
        );
    }

    #[test]
    fn recognizes_retry_and_game_end_signals() {
        assert_eq!(
            parse_visible_chat("You were spawned in Limbo."),
            Some(VisibleChatSignal::LimboSpawned)
        );
        assert_eq!(
            parse_visible_chat("Please don't spam the command!"),
            Some(VisibleChatSignal::CommandRejected(
                "Please don't spam the command!".to_owned()
            ))
        );
        assert_eq!(
            parse_visible_chat("Reward Summary"),
            Some(VisibleChatSignal::DuelEnded)
        );
        assert_eq!(
            parse_visible_chat("VICTORY!"),
            Some(VisibleChatSignal::BedwarsOrSkywarsEnded)
        );
    }
}
