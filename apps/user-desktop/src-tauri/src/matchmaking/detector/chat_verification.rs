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

//! 解析玩家聊天名稱並提供驗證短句；在場確認採不區分大小寫的名稱比對。

use std::time::{SystemTime, UNIX_EPOCH};

/// 解析玩家聊天的發言者及內容，名稱僅接受 Minecraft 字元。
/// @param line 已移除格式碼的可見聊天行。
/// @return 玩家名稱與內容；無效格式為 None。
pub(crate) fn parse(line: &str) -> Option<(String, String)> {
    let (speaker, message) = line.split_once(':')?;
    let username = speaker
        .split_whitespace()
        .last()?
        .trim_matches(|character: char| !character.is_ascii_alphanumeric() && character != '_');
    if username.is_empty()
        || !username
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || character == '_')
        || message.trim().is_empty()
    {
        return None;
    }
    Some((username.to_owned(), message.trim().to_owned()))
}

/// 以不區分大小寫的真實名稱確認發言者。
/// @param expected_username 計畫內保存的玩家名稱。
/// @param username 玩家日誌解析出的發言者。
/// @return 名稱相符時為 true。
pub(crate) fn matches(expected_username: &str, username: &str) -> bool {
    expected_username.eq_ignore_ascii_case(username)
}

/// 從既有短句清單選取在場驗證用聊天文字。
/// @return 短句字串；不是密碼學挑戰值。
pub(crate) fn random_phrase() -> String {
    const PHRASES: [&str; 100] = [
        "hi",
        "hello",
        "hey",
        "yo",
        "hiya",
        "sup",
        "heya",
        "greetings",
        "howdy",
        "hola",
        "hi there",
        "hello there",
        "hey there",
        "yo there",
        "good day",
        "morning",
        "evening",
        "nice game",
        "good luck",
        "have fun",
        "lets go",
        "ready",
        "all good",
        "im here",
        "here",
        "checking in",
        "quick hello",
        "just saying hi",
        "hello team",
        "hey team",
        "hi team",
        "hello friend",
        "hey friend",
        "hi friend",
        "good to see you",
        "nice to meet you",
        "welcome",
        "welcome here",
        "hello again",
        "hey again",
        "hi again",
        "test hello",
        "test hi",
        "chat check",
        "presence check",
        "queue check",
        "server check",
        "same room",
        "same game",
        "right here",
        "over here",
        "on my way",
        "we are ready",
        "we got this",
        "lets play",
        "lets win",
        "good start",
        "nice start",
        "all set",
        "set to go",
        "ready now",
        "hello everyone",
        "hey everyone",
        "hi everyone",
        "yo everyone",
        "hello all",
        "hey all",
        "hi all",
        "good evening",
        "good morning",
        "good afternoon",
        "what is up",
        "whats up",
        "how is it going",
        "hope youre well",
        "checking chat",
        "checking in now",
        "quick check",
        "one two",
        "two two",
        "copy that",
        "message sent",
        "chat ready",
        "hello lobby",
        "hey lobby",
        "hi lobby",
        "hello server",
        "hey server",
        "hi server",
        "friendly hello",
        "friendly hi",
        "small hello",
        "short hello",
        "quick hi",
        "quick hey",
        "say hi",
        "say hello",
        "ping",
        "pong",
        "lets begin",
    ];
    let tick = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    PHRASES[(tick % PHRASES.len() as u128) as usize].to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn confirms_the_real_username_without_requiring_the_sent_phrase() {
        assert!(matches("BotPlayer", "botplayer"));
        assert!(!matches("BotPlayer", "AnotherPlayer"));
    }
}
