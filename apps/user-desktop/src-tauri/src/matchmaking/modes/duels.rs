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

//! 要求玩家先見到等待狀態，再與 Bot 同時觀察到二人佇列已滿。

use crate::hypixel;
use std::time::{Duration, Instant};

pub(crate) const CONFIRM_WINDOW: Duration = Duration::from_secs(2);
pub(crate) const CONFIRM_TIMEOUT: Duration = Duration::from_secs(2);
pub(crate) const PITCH_CONFIRM_TIMEOUT: Duration = Duration::from_secs(3);

#[derive(Clone, Copy)]
/// Duels 佇列快照，另記錄是否曾觀察到一人等待狀態。
pub(crate) struct QueueObservation {
    pub current: u32,
    pub total: u32,
    pub observed_at: Instant,
    pub saw_waiting: bool,
}

/// 更新佇列並保留是否曾經看過一人等待。
/// @param previous 同一輪的前次觀察。
/// @param current 目前人數。
/// @param total 佇列容量。
/// @return 帶累積等待標記的新觀察。
pub(crate) fn next_observation(
    previous: Option<QueueObservation>,
    current: u32,
    total: u32,
) -> QueueObservation {
    QueueObservation {
        current,
        total,
        observed_at: Instant::now(),
        saw_waiting: previous.is_some_and(|value| value.saw_waiting)
            || (current == 1 && total == 2),
    }
}

/// 要求玩家曾等待，之後雙方在短時間內觀察到相同伺服器二人滿隊。
/// @param player_server 玩家目標伺服器。
/// @param bot_server Bot 觀察到的伺服器。
/// @param round_started_at 本輪開始時間。
/// @param player 玩家端佇列觀察。
/// @param bot Bot 端佇列觀察。
/// @return 符合本輪 Duels 佇列條件時為 true。
pub(crate) fn is_candidate(
    player_server: Option<&str>,
    bot_server: Option<&str>,
    round_started_at: Option<Instant>,
    player: Option<QueueObservation>,
    bot: Option<QueueObservation>,
) -> bool {
    let Some(round_started_at) = round_started_at else {
        return false;
    };
    let Some(player_server) = player_server else {
        return false;
    };
    let Some(bot_server) = bot_server else {
        return false;
    };
    if !hypixel::server_matches(player_server, bot_server) {
        return false;
    }

    let Some(player) = player else {
        return false;
    };
    let Some(bot) = bot else {
        return false;
    };
    if player.current != 2
        || player.total != 2
        || !player.saw_waiting
        || bot.current != 2
        || bot.total != 2
        || player.observed_at < round_started_at
        || bot.observed_at < round_started_at
    {
        return false;
    }

    let difference = if player.observed_at >= bot.observed_at {
        player.observed_at.duration_since(bot.observed_at)
    } else {
        bot.observed_at.duration_since(player.observed_at)
    };
    difference <= CONFIRM_WINDOW
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_bot_that_only_observes_two_of_two() {
        let round_started_at = Instant::now();
        let player = QueueObservation {
            current: 2,
            total: 2,
            observed_at: round_started_at + Duration::from_millis(300),
            saw_waiting: true,
        };
        let bot = QueueObservation {
            current: 2,
            total: 2,
            observed_at: round_started_at + Duration::from_millis(550),
            saw_waiting: false,
        };

        assert!(is_candidate(
            Some("mini98CN"),
            Some("mini98CN"),
            Some(round_started_at),
            Some(player),
            Some(bot),
        ));
    }
}
