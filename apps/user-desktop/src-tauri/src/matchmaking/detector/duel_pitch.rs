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

//! 記錄俯仰驗證的來源與時間，排除前一輪或前一次嘗試的觀察。

use std::time::Instant;

/// 待確認俯仰手勢的伺服器與嘗試識別資訊。
pub(crate) struct VerificationCheck {
    pub generation: u64,
    pub attempt_id: String,
    pub server: String,
}

/// 已觀察手勢的世代、嘗試 ID 及時間。
pub(crate) struct Observation {
    pub generation: u64,
    pub attempt_id: String,
    pub observed_at: Instant,
}

/// 排除舊世代、舊嘗試及本輪之前的手勢。
/// @param observation 已保存的手勢觀察。
/// @param generation 目前目標世代。
/// @param attempt_id 目前嘗試 ID。
/// @param round_started_at 本輪開始時間。
/// @return 觀察仍有效時為 true。
pub(crate) fn belongs_to_attempt(
    observation: &Observation,
    generation: u64,
    attempt_id: &str,
    round_started_at: Option<Instant>,
) -> bool {
    observation.generation == generation
        && observation.attempt_id == attempt_id
        && round_started_at.is_some_and(|started| observation.observed_at >= started)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_an_observation_from_an_old_attempt() {
        let started = Instant::now();
        let observation = Observation {
            generation: 2,
            attempt_id: "current".to_owned(),
            observed_at: started,
        };
        assert!(belongs_to_attempt(
            &observation,
            2,
            "current",
            Some(started)
        ));
        assert!(!belongs_to_attempt(&observation, 2, "old", Some(started)));
    }
}
