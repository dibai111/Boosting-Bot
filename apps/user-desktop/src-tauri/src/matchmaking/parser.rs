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

//! 只解析玩家記錄中的聊天內容，移除 Minecraft 格式碼後轉成配對事件。

use super::detector::chat_verification;
use crate::hypixel::{self, VisibleChatSignal};

#[derive(Debug, Clone, PartialEq, Eq)]
/// 從玩家 Minecraft 日誌解析出的配對相關事件。
pub enum PlayerLogEvent {
    ServerTransfer(String),
    QueueProgress {
        username: String,
        current: u32,
        total: u32,
    },
    ChatMessage {
        username: String,
        message: String,
    },
    LobbyJoined,
    GameStarted,
    GameEnded,
}

/// 只處理 CHAT 區段，移除格式碼後辨識配對訊號。
/// @param line 單行 Minecraft 玩家日誌。
/// @return 已辨識事件；非聊天或無關內容為 None。
pub fn parse_player_log_line(line: &str) -> Option<PlayerLogEvent> {
    let payload = chat_payload(line)?;
    let plain = strip_formatting(payload);
    let normalized = plain.trim();

    if let Some(signal) = hypixel::parse_visible_chat(normalized) {
        match signal {
            VisibleChatSignal::ServerTransfer(server) => {
                return Some(PlayerLogEvent::ServerTransfer(server));
            }
            VisibleChatSignal::QueueProgress {
                username,
                current,
                total,
            } => {
                return Some(PlayerLogEvent::QueueProgress {
                    username,
                    current,
                    total,
                });
            }
            VisibleChatSignal::GameStarted(_) => return Some(PlayerLogEvent::GameStarted),
            VisibleChatSignal::DuelEnded | VisibleChatSignal::BedwarsOrSkywarsEnded => {
                return Some(PlayerLogEvent::GameEnded);
            }
            VisibleChatSignal::LimboSpawned | VisibleChatSignal::CommandRejected(_) => {}
        }
    }

    if normalized.starts_with('{') {
        return None;
    }
    if let Some((username, message)) = chat_verification::parse(normalized) {
        return Some(PlayerLogEvent::ChatMessage { username, message });
    }

    let lowercase = normalized.to_ascii_lowercase();
    if lowercase.contains("joined the lobby!") {
        return Some(PlayerLogEvent::LobbyJoined);
    }
    None
}

fn chat_payload(line: &str) -> Option<&str> {
    line.rsplit_once("[CHAT]")
        .map(|(_, payload)| payload.trim())
}

fn strip_formatting(line: &str) -> String {
    let mut result = String::with_capacity(line.len());
    let mut characters = line.chars();
    while let Some(character) = characters.next() {
        if character == '\u{00a7}' {
            let _ = characters.next();
        } else {
            result.push(character);
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_fresh_server_transfer_from_minecraft_log() {
        assert_eq!(
            parse_player_log_line(
                "[21:38:08] [Client thread/INFO]: [CHAT] Sending you to mini198Q!"
            ),
            Some(PlayerLogEvent::ServerTransfer("mini198Q".to_owned()))
        );
    }

    #[test]
    fn recognizes_game_boundaries() {
        assert_eq!(
            parse_player_log_line("[CHAT] Opponent: Player"),
            Some(PlayerLogEvent::GameStarted)
        );
        assert_eq!(
            parse_player_log_line("[CHAT] Cages opened! FIGHT!"),
            Some(PlayerLogEvent::GameStarted)
        );
        assert_eq!(
            parse_player_log_line("[CHAT] Protect your bed and destroy the enemy beds!"),
            Some(PlayerLogEvent::GameStarted)
        );
        assert_eq!(
            parse_player_log_line("[CHAT] VICTORY!"),
            Some(PlayerLogEvent::GameEnded)
        );
        assert_eq!(
            parse_player_log_line("[CHAT] Reward Summary"),
            Some(PlayerLogEvent::GameEnded)
        );
        for rank in ["1st", "2nd", "3rd"] {
            assert_eq!(
                parse_player_log_line(&format!(
                    "[CHAT] \u{00a7}e{rank} \u{00a7}fKiller - Player - 10"
                )),
                Some(PlayerLogEvent::GameEnded)
            );
        }
    }

    #[test]
    fn recognizes_any_lobby_join_message() {
        assert_eq!(
            parse_player_log_line(
                "[02:45:03] [Client thread/INFO]: [CHAT] \u{00a7}b[MVP\u{00a7}c+\u{00a7}b] baibai_002\u{00a7}f \u{00a7}6joined the lobby!"
            ),
            Some(PlayerLogEvent::LobbyJoined)
        );
        assert_eq!(
            parse_player_log_line("[CHAT] AnotherPlayer joined the lobby!"),
            Some(PlayerLogEvent::LobbyJoined)
        );
        assert_eq!(
            parse_player_log_line("[CHAT] baibai_002 has joined (1/12)!"),
            Some(PlayerLogEvent::QueueProgress {
                username: "baibai_002".to_owned(),
                current: 1,
                total: 12,
            })
        );
    }

    #[test]
    fn parses_duel_queue_progress() {
        assert_eq!(
            parse_player_log_line("[CHAT] Opponent has joined (2/2)!"),
            Some(PlayerLogEvent::QueueProgress {
                username: "Opponent".to_owned(),
                current: 2,
                total: 2,
            })
        );
    }

    #[test]
    fn parses_player_chat_for_presence_confirmation() {
        assert_eq!(
            parse_player_log_line("[CHAT] [MVP+] BotPlayer: hello there"),
            Some(PlayerLogEvent::ChatMessage {
                username: "BotPlayer".to_owned(),
                message: "hello there".to_owned(),
            })
        );
        assert_eq!(
            parse_player_log_line(
                "[03:59:31] [Client thread/INFO]: [CHAT] §b[MVP§2+§b] Prank_Monkey6969§f: hi"
            ),
            Some(PlayerLogEvent::ChatMessage {
                username: "Prank_Monkey6969".to_owned(),
                message: "hi".to_owned(),
            })
        );
        assert_eq!(
            parse_player_log_line(
                "[03:59:31] [Client thread/INFO]: [CHAT] [MVP+] Prank_Monkey6969: hi"
            ),
            Some(PlayerLogEvent::ChatMessage {
                username: "Prank_Monkey6969".to_owned(),
                message: "hi".to_owned(),
            })
        );
    }

    #[test]
    fn ignores_player_chat_and_old_location_payloads() {
        assert_eq!(
            parse_player_log_line("[CHAT] <Player> Sending you to mini198Q!"),
            None
        );
        assert_eq!(
            parse_player_log_line(r#"[CHAT] {"server":"mini198Q","gametype":"BEDWARS"}"#),
            None
        );
    }
}
