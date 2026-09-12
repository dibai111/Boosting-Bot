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

//! 處理單一 Bot 的配對指令、轉服逾時及俯仰角觀察；整體配對決策留在 coordinator。

use super::{
    detection::{self, PitchTracker},
    BotEvent, BotGamePhase, GameKind, MatchAttempt, SessionEmitter,
};
use azalea::{
    core::entity_id::MinecraftEntityId,
    ecs::query::{With, Without},
    entity::{metadata::Player, LocalEntity, LookDirection},
    Client,
};
use std::time::{Duration, Instant};

const TRANSFER_TIMEOUT: Duration = Duration::from_secs(6);
const RETRY_TRANSFER_TIMEOUT: Duration = Duration::from_secs(12);
const PITCH_SCAN_INTERVAL: Duration = Duration::from_millis(100);

/// 管理單一 Bot 的配對嘗試、Limbo 重試與俯仰觀察。
pub(super) struct MatchController {
    active: Option<ActiveMatch>,
    retry: Option<PendingRetry>,
    pitch: PitchTracker,
}

struct ActiveMatch {
    attempt: MatchAttempt,
    waiting_for_transfer: bool,
    transfer_deadline: Instant,
    game_state: Option<BotGamePhase>,
    pitch_armed: bool,
    next_pitch_scan: Instant,
}

struct PendingRetry {
    request_id: String,
    deadline: Instant,
}

impl MatchController {
    /// 建立尚未配對的控制器。
    /// @return 沒有有效嘗試或觀察資料的控制器。
    pub(super) fn new() -> Self {
        Self {
            active: None,
            retry: None,
            pitch: PitchTracker::new(),
        }
    }

    /// 清除上一個嘗試後發出遊戲佇列指令。
    /// @param client 已生成角色的連線。
    /// @param attempt 含 session、round、generation 及模式的嘗試。
    /// @param now 本次嘗試的單調時鐘時間。
    /// @return 無回傳值；開始等待轉服訊息。
    pub(super) fn begin(&mut self, client: &Client, attempt: MatchAttempt, now: Instant) {
        self.cancel();
        client.chat(attempt.mode.play_command());
        self.active = Some(ActiveMatch {
            attempt,
            waiting_for_transfer: true,
            transfer_deadline: now + TRANSFER_TIMEOUT,
            game_state: None,
            pitch_armed: false,
            next_pitch_scan: now,
        });
    }

    /// 清除配對、重試與俯仰追蹤狀態。
    /// @return 無回傳值；不直接發送網路指令。
    pub(super) fn cancel(&mut self) {
        self.active = None;
        self.retry = None;
        self.pitch.reset();
    }

    /// 取消嘗試，角色已生成時才發送返回大廳指令。
    /// @param client 目前連線。
    /// @param spawned 角色是否已可接收聊天指令。
    /// @return 無回傳值。
    pub(super) fn return_to_lobby(&mut self, client: &Client, spawned: bool) {
        self.cancel();
        if spawned {
            client.chat("/l");
        }
    }

    /// 先清除舊嘗試，再要求進入 Limbo。
    /// @param client 目前連線。
    /// @param request_id 協調器指定的重試識別碼。
    /// @param now 計算 Limbo 逾時的基準時間。
    /// @return 無回傳值；準備結果透過事件回報。
    pub(super) fn prepare_retry(&mut self, client: &Client, request_id: String, now: Instant) {
        self.cancel();
        self.retry = Some(PendingRetry {
            request_id,
            deadline: now + RETRY_TRANSFER_TIMEOUT,
        });
        client.chat("/limbo");
    }

    /// 將聊天訊號轉成轉服、佇列、重試及遊戲階段事件。
    /// @param message 伺服器可見聊天文字。
    /// @param bot_id 此連線的 Bot ID。
    /// @param emitter 此連線的事件發送器。
    /// @param now 本次觀察時間。
    /// @return 無回傳值；只更新目前嘗試並發布對應事件。
    pub(super) fn handle_chat(
        &mut self,
        message: &str,
        bot_id: &str,
        emitter: &SessionEmitter,
        now: Instant,
    ) {
        // /limbo 不一定觸發 Azalea Spawn；Hypixel 的確認聊天才代表 retry 已可繼續。
        if detection::limbo_spawn(message) {
            if let Some(retry) = self.retry.take() {
                emitter.publish(BotEvent::MatchRetryReady {
                    bot_id: bot_id.to_owned(),
                    request_id: retry.request_id,
                });
                return;
            }
        }

        if self.active.is_some() {
            if let Some((current, total)) = detection::queue_progress(message) {
                emitter.publish(BotEvent::QueueProgress {
                    bot_id: bot_id.to_owned(),
                    current,
                    total,
                });
            }
        }

        if self
            .active
            .as_ref()
            .is_some_and(|active| active.waiting_for_transfer)
        {
            if let Some(rejection) = detection::command_rejection(message) {
                let Some(active) = self.active.take() else {
                    return;
                };
                emit_attempt_failure(emitter, bot_id, active.attempt, "command_spam", rejection);
                return;
            }

            if let Some(server) = detection::server_transfer(message) {
                let (attempt, start_pitch_observation) = {
                    let Some(active) = self.active.as_mut() else {
                        return;
                    };
                    active.waiting_for_transfer = false;
                    let start_pitch_observation = active.attempt.mode.kind() == GameKind::Duels
                        && active.attempt.requires_pitch_verification;
                    if start_pitch_observation {
                        active.pitch_armed = true;
                        active.next_pitch_scan = now;
                    }
                    (active.attempt.clone(), start_pitch_observation)
                };
                if start_pitch_observation {
                    emitter.publish(BotEvent::MatchmakingDebug {
                        bot_id: bot_id.to_owned(),
                        message: "[duels pitch] observation started".to_owned(),
                    });
                }
                emitter.publish(BotEvent::MatchAttemptResult {
                    bot_id: bot_id.to_owned(),
                    session_id: attempt.session_id,
                    round_id: attempt.round_id,
                    target_generation: attempt.target_generation,
                    attempt_id: attempt.attempt_id,
                    server,
                });
            }
        }

        let Some(active) = self.active.as_mut() else {
            return;
        };
        let Some(state) = detection::game_state(active.attempt.mode.kind(), message) else {
            return;
        };
        if active.game_state == Some(state) {
            return;
        }
        active.game_state = Some(state);
        emitter.publish(BotEvent::BotGameState {
            bot_id: bot_id.to_owned(),
            round_id: active.attempt.round_id.clone(),
            target_generation: active.attempt.target_generation,
            state,
        });
    }

    /// 需要俯仰驗證時才掃描附近玩家，確認後立即停止掃描。
    /// @param client 目前連線。
    /// @param now 掃描節流與穩定手勢判斷時間。
    /// @param bot_id 此連線的 Bot ID。
    /// @param emitter 此連線的事件發送器。
    /// @return 無回傳值；符合條件時發布俯仰觀察。
    pub(super) fn tick(
        &mut self,
        client: &Client,
        now: Instant,
        bot_id: &str,
        emitter: &SessionEmitter,
    ) {
        let Some(active) = self.active.as_mut() else {
            return;
        };
        if !active.pitch_armed || now < active.next_pitch_scan {
            return;
        }
        active.next_pitch_scan = now + PITCH_SCAN_INTERVAL;

        let players =
            client.nearest_entity_ids_by::<(), (With<Player>, Without<LocalEntity>)>(|()| true);
        for entity in players {
            let Ok((entity_id, pitch)) = client
                .try_query_entity::<(&MinecraftEntityId, &LookDirection), _>(
                    entity,
                    |(id, look)| (id.0, look.x_rot()),
                )
            else {
                continue;
            };
            let Some(direction) = self.pitch.observe(entity_id, pitch, now) else {
                continue;
            };

            active.pitch_armed = false;
            emitter.publish(BotEvent::MatchmakingDebug {
                bot_id: bot_id.to_owned(),
                message: format!(
                    "[duels pitch] gesture confirmed: direction={}, pitch={pitch:.1}deg",
                    direction.as_str()
                ),
            });
            emitter.publish(BotEvent::DuelPitchObserved {
                bot_id: bot_id.to_owned(),
                round_id: active.attempt.round_id.clone(),
                target_generation: active.attempt.target_generation,
                attempt_id: active.attempt.attempt_id.clone(),
                direction,
                pitch: pitch.to_radians(),
            });
            break;
        }
    }

    /// 結束已超過期限的轉服或 Limbo 準備。
    /// @param now 當前單調時鐘時間。
    /// @param bot_id 此連線的 Bot ID。
    /// @param emitter 此連線的事件發送器。
    /// @return 無回傳值；逾時會發布帶識別碼的失敗事件。
    pub(super) fn check_timeouts(&mut self, now: Instant, bot_id: &str, emitter: &SessionEmitter) {
        if self
            .active
            .as_ref()
            .is_some_and(|active| active.waiting_for_transfer && now >= active.transfer_deadline)
        {
            let Some(active) = self.active.take() else {
                return;
            };
            emit_attempt_failure(
                emitter,
                bot_id,
                active.attempt,
                "server_transfer_timeout",
                "server transfer chat timed out".to_owned(),
            );
        }

        if self
            .retry
            .as_ref()
            .is_some_and(|retry| now >= retry.deadline)
        {
            let Some(retry) = self.retry.take() else {
                return;
            };
            emitter.publish(BotEvent::MatchRetryPreparationFailed {
                bot_id: bot_id.to_owned(),
                request_id: retry.request_id,
                message: "server transfer timed out".to_owned(),
            });
        }
    }
}

fn emit_attempt_failure(
    emitter: &SessionEmitter,
    bot_id: &str,
    attempt: MatchAttempt,
    code: &str,
    message: String,
) {
    emitter.publish(BotEvent::MatchAttemptFailed {
        bot_id: bot_id.to_owned(),
        session_id: attempt.session_id,
        round_id: attempt.round_id,
        target_generation: attempt.target_generation,
        attempt_id: attempt.attempt_id,
        code: code.to_owned(),
        message,
    });
}
