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

//! 公開配對計畫、快照及協調器，隱藏記錄解析和遊戲模式的實作細節。

mod coordinator;
mod detector;
mod model;
mod modes;
mod parser;
mod plan;
mod player_log;

pub(crate) use coordinator::{MatchmakingDiagnostic, MatchmakingSession};
pub(crate) use model::{
    BotMatchPhase, MatchmakingBotState, MatchmakingPhase, MatchmakingSnapshot,
    StartMatchmakingInput,
};
pub(crate) use plan::MatchmakingPlan;
