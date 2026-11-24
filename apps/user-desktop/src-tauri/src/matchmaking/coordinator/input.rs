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

//! 所有配對輸入在同一生命週期鎖內處理，執行指令完成前不接受啟停。
use super::super::player_log::PlayerLogMessage;
use super::{MatchmakingPhase, MatchmakingSession};
use crate::bot_runtime::BotEvent;
use std::{
    sync::{Arc, Weak},
    time::Duration,
};

pub(super) enum SessionInput {
    Bot(BotEvent),
    PlayerLog {
        session_id: String,
        message: PlayerLogMessage,
    },
    Timer(TimerEvent),
}

/// 每種計時器只描述業務事件，adapter 不能傳入任意可變狀態回呼。
pub(super) enum TimerKind {
    Retry,
    DuelsQueue { attempt_id: String, server: String },
    BedwarsQueue { attempt_id: String, server: String },
    SkywarsName { attempt_id: String },
    DuelPitch { attempt_id: String },
    Presence { attempt_id: String },
}

pub(super) struct TimerEvent {
    session_id: String,
    round_id: String,
    generation: u64,
    bot_id: String,
    kind: TimerKind,
}

/// 弱參照收件者與具型別的到期事件；不延長工作階段壽命。
pub(super) struct ScheduledTimer {
    target: Weak<MatchmakingSession>,
    event: TimerEvent,
}
impl ScheduledTimer {
    pub(super) async fn deliver(self) {
        if let Some(target) = self.target.upgrade() {
            target.handle_input(SessionInput::Timer(self.event)).await;
        }
    }
}

impl MatchmakingSession {
    /// 只從這個入口取得生命週期鎖；涵蓋狀態轉移及指令發送。
    pub(super) async fn handle_input(self: &Arc<Self>, input: SessionInput) {
        let _lifecycle = self.lifecycle.lock().await;
        match input {
            SessionInput::Bot(event) => self.apply_bot_event(&event).await,
            SessionInput::PlayerLog {
                session_id,
                message,
            } => {
                let current = {
                    let state = self.state.lock().await;
                    state.plan.is_some()
                        && state.snapshot.session_id.as_deref() == Some(&session_id)
                };
                if !current {
                    return;
                }
                match message {
                    PlayerLogMessage::Line(line) => self.apply_player_line(&line).await,
                    PlayerLogMessage::Unavailable(error) => {
                        self.fail_round(format!("Player log unavailable: {error}"))
                            .await
                    }
                }
            }
            SessionInput::Timer(event) => {
                let current = {
                    let state = self.state.lock().await;
                    state.snapshot.phase == MatchmakingPhase::Matching
                        && state.snapshot.session_id.as_deref() == Some(&event.session_id)
                        && state.snapshot.round_id.as_deref() == Some(&event.round_id)
                        && state.target_generation == event.generation
                };
                if !current {
                    return;
                }
                let TimerEvent {
                    bot_id,
                    generation,
                    kind,
                    ..
                } = event;
                match kind {
                    TimerKind::Retry => self.retry_bot(&bot_id, generation).await,
                    TimerKind::DuelsQueue { attempt_id, server } => {
                        self.handle_duels_queue_timeout(bot_id, generation, attempt_id, server)
                            .await
                    }
                    TimerKind::BedwarsQueue { attempt_id, server } => {
                        self.handle_bedwars_queue_timeout(bot_id, generation, attempt_id, server)
                            .await
                    }
                    TimerKind::SkywarsName { attempt_id } => {
                        self.handle_skywars_name_timeout(bot_id, generation, attempt_id)
                            .await
                    }
                    TimerKind::DuelPitch { attempt_id } => {
                        self.handle_duel_pitch_timeout(bot_id, generation, attempt_id)
                            .await
                    }
                    TimerKind::Presence { attempt_id } => {
                        self.handle_presence_timeout(bot_id, generation, attempt_id)
                            .await
                    }
                }
            }
        }
    }

    /// 日誌 consumer 保留啟動時的 session_id；排隊中的舊輸入不會跨到新執行。
    pub(super) async fn handle_player_log(
        self: &Arc<Self>,
        session_id: &str,
        message: PlayerLogMessage,
    ) {
        self.handle_input(SessionInput::PlayerLog {
            session_id: session_id.to_owned(),
            message,
        })
        .await;
    }

    /// 在排程時取得完整執行識別；計時器到期後仍須經過同一輸入入口。
    pub(super) async fn schedule_timer(
        self: &Arc<Self>,
        delay: Duration,
        bot_id: String,
        generation: u64,
        kind: TimerKind,
    ) {
        let state = self.state.lock().await;
        if state.snapshot.phase != MatchmakingPhase::Matching
            || state.target_generation != generation
        {
            return;
        }
        let (Some(session_id), Some(round_id)) = (
            state.snapshot.session_id.clone(),
            state.snapshot.round_id.clone(),
        ) else {
            return;
        };
        let timer = ScheduledTimer {
            target: Arc::downgrade(self),
            event: TimerEvent {
                session_id,
                round_id,
                generation,
                bot_id,
                kind,
            },
        };
        drop(state);
        self.effects.schedule(delay, timer);
    }
}
