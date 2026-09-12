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

//! 以 tick 驅動 AFK 的走動、視角和揮手，停止時一併解除持續中的動作。

use azalea::{
    protocol::packets::game::{s_interact::InteractionHand, ServerboundSwing},
    Client, WalkDirection,
};
use rand::Rng;
use std::time::{Duration, Instant};

const ACTION_DELAY_MS: std::ops::RangeInclusive<u64> = 1_000..=2_500;
const LOOK_DELAY_MS: std::ops::RangeInclusive<u64> = 100..=400;
const LOOK_YAW_DELTA_DEGREES: std::ops::RangeInclusive<f32> = -35.0..=35.0;
const LOOK_PITCH_DELTA_DEGREES: std::ops::RangeInclusive<f32> = -10.0..=10.0;
const WALK_INTERVAL_MS: std::ops::RangeInclusive<u64> = 6_000..=10_000;
const WALK_DURATION_MS: std::ops::RangeInclusive<u64> = 250..=400;

/// 以 Tick 排程短時間走動、轉向、蹲下及揮手的 AFK 控制器。
pub(super) struct AdvancedAfk {
    active: bool,
    next_action: Instant,
    next_look: Instant,
    next_walk: Instant,
    crouch_until: Option<Instant>,
    walking_until: Option<Instant>,
}

impl AdvancedAfk {
    /// 初始化尚未啟用的 AFK 排程。
    /// @param now 初始單調時間。
    /// @return 未啟用的控制器。
    pub(super) fn new(now: Instant) -> Self {
        Self {
            active: false,
            next_action: now,
            next_look: now,
            next_walk: now,
            crouch_until: None,
            walking_until: None,
        }
    }

    /// 啟用 AFK 並建立下一次動作時間。
    /// @param bot 可執行動作的 client。
    /// @param now 排程基準時間。
    /// @return 由停用變成啟用時為 true。
    pub(super) fn start(&mut self, bot: &Client, now: Instant) -> bool {
        if self.active {
            return false;
        }

        self.active = true;
        self.next_action = now + random_duration(ACTION_DELAY_MS);
        self.next_look = now + random_duration(LOOK_DELAY_MS);
        self.crouch_until = None;
        self.walking_until = None;
        self.start_walk(bot, now);
        true
    }

    /// 執行到期動作，並解除已到期限的走動與蹲下。
    /// @param bot 目前連線。
    /// @param now 本次 Tick 時間。
    /// @return 無回傳值；未啟用時直接返回。
    pub(super) fn tick(&mut self, bot: &Client, now: Instant) {
        if !self.active {
            return;
        }

        // 所有 AFK 動作由同一個 Minecraft tick state machine 控制，停止時不會留下背景 task。
        if self.crouch_until.is_some_and(|deadline| now >= deadline) {
            bot.set_crouching(false);
            self.crouch_until = None;
        }
        if self.walking_until.is_some_and(|deadline| now >= deadline) {
            bot.walk(WalkDirection::None);
            self.walking_until = None;
        }
        if now >= self.next_walk {
            self.start_walk(bot, now);
        }

        if now >= self.next_look {
            let direction = bot.direction();
            bot.set_direction(
                direction.y_rot() + random_f32(LOOK_YAW_DELTA_DEGREES),
                direction.x_rot() + random_f32(LOOK_PITCH_DELTA_DEGREES),
            );
            self.next_look = now + random_duration(LOOK_DELAY_MS);
        }

        if now < self.next_action {
            return;
        }

        if random_chance(0.4) {
            bot.write_packet(ServerboundSwing {
                hand: InteractionHand::MainHand,
            });
        }
        if random_chance(0.4) {
            bot.set_crouching(true);
            self.crouch_until = Some(now + random_duration(300..=500));
        }
        if random_chance(0.3) {
            bot.jump();
        }
        self.next_action = now + random_duration(ACTION_DELAY_MS);
    }

    fn start_walk(&mut self, bot: &Client, now: Instant) {
        bot.walk(WalkDirection::Forward);
        self.walking_until = Some(now + random_duration(WALK_DURATION_MS));
        self.next_walk = now + random_duration(WALK_INTERVAL_MS);
    }

    /// 清除持續動作及排程，角色仍連線時發送解除動作。
    /// @param bot 可用 client；已斷線時可傳 None。
    /// @return 停止前是否啟用。
    pub(super) fn stop(&mut self, bot: Option<&Client>) -> bool {
        let was_active = self.active;
        self.active = false;
        self.crouch_until = None;
        self.walking_until = None;
        if let Some(bot) = bot {
            bot.walk(WalkDirection::None);
            bot.set_crouching(false);
        }
        was_active
    }
}

fn random_chance(probability: f64) -> bool {
    rand::thread_rng().gen_bool(probability)
}

fn random_duration(range: std::ops::RangeInclusive<u64>) -> Duration {
    Duration::from_millis(rand::thread_rng().gen_range(range))
}

fn random_f32(range: std::ops::RangeInclusive<f32>) -> f32 {
    rand::thread_rng().gen_range(range)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn afk_look_deltas_stay_within_the_configured_ranges() {
        for _ in 0..1_000 {
            assert!(LOOK_YAW_DELTA_DEGREES.contains(&random_f32(LOOK_YAW_DELTA_DEGREES)));
            assert!(LOOK_PITCH_DELTA_DEGREES.contains(&random_f32(LOOK_PITCH_DELTA_DEGREES)));
        }
    }

    #[test]
    fn afk_walks_briefly_after_a_six_to_ten_second_interval() {
        for _ in 0..1_000 {
            let interval = random_duration(WALK_INTERVAL_MS);
            let duration = random_duration(WALK_DURATION_MS);
            assert!(interval >= Duration::from_secs(6));
            assert!(interval <= Duration::from_secs(10));
            assert!(duration >= Duration::from_millis(250));
            assert!(duration <= Duration::from_millis(400));
        }
    }
}
