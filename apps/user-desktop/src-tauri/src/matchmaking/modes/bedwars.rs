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

//! 以相同伺服器、容量及短時間內增加的玩家數判斷 BedWars 候選。

use crate::hypixel;
use std::time::{Duration, Instant};

pub(crate) const CONFIRM_WINDOW: Duration = Duration::from_secs(2);
pub(crate) const CONFIRM_TIMEOUT: Duration = Duration::from_secs(2);

#[derive(Clone, Copy)]
/// 包含人數、容量及單調觀察時間的 BedWars 佇列快照。
pub(crate) struct QueueObservation {
    pub current: u32,
    pub total: u32,
    pub observed_at: Instant,
}

/// 記錄一次 BedWars 佇列人數。
/// @param current 目前人數。
/// @param total 佇列容量。
/// @return 以目前 Instant 標記的觀察。
pub(crate) fn observe_queue(current: u32, total: u32) -> QueueObservation {
    QueueObservation {
        current,
        total,
        observed_at: Instant::now(),
    }
}

/// 要求相同伺服器及容量，且 Bot 人數在短時間內晚於玩家增加。
/// @param player_server 玩家目標伺服器。
/// @param bot_server Bot 觀察到的伺服器。
/// @param round_started_at 本輪開始時間。
/// @param player 玩家端佇列觀察。
/// @param bot Bot 端佇列觀察。
/// @return 符合本輪 BedWars 佇列條件時為 true。
pub(crate) fn is_candidate(
    player_server: Option<&str>,
    bot_server: Option<&str>,
    round_started_at: Option<Instant>,
    player: Option<QueueObservation>,
    bot: Option<QueueObservation>,
) -> bool {
    let (Some(player_server), Some(bot_server), Some(round_started_at), Some(player), Some(bot)) =
        (player_server, bot_server, round_started_at, player, bot)
    else {
        return false;
    };
    if !hypixel::server_matches(player_server, bot_server)
        || bot.current <= player.current
        || player.total != bot.total
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
    fn requires_bot_queue_count_to_follow_player_count() {
        let started = Instant::now();
        let player = QueueObservation {
            current: 5,
            total: 8,
            observed_at: started,
        };
        let bot = QueueObservation {
            current: 6,
            total: 8,
            observed_at: started,
        };
        assert!(is_candidate(
            Some("mini1"),
            Some("MINI1"),
            Some(started),
            Some(player),
            Some(bot)
        ));
        assert!(!is_candidate(
            Some("mini1"),
            Some("mini1"),
            Some(started),
            Some(player),
            Some(QueueObservation { current: 5, ..bot }),
        ));
        assert!(!is_candidate(
            Some("mini1"),
            Some("mini1"),
            Some(started),
            Some(player),
            Some(QueueObservation { total: 4, ..bot }),
        ));
    }

    #[test]
    fn accepts_a_matching_late_queue_observation() {
        let started = Instant::now();
        let player = observe_queue(6, 8);
        let bot = observe_queue(7, 8);

        assert!(is_candidate(
            Some("mini220Q"),
            Some("MINI220Q"),
            Some(started),
            Some(player),
            Some(bot),
        ));
    }
}
