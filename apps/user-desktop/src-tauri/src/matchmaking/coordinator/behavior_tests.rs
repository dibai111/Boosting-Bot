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

//! 以配對協調器的既有入口驗證事件順序、停止及重啟，測試自行推進時鐘。
use super::super::player_log::PlayerLogMessage;
use super::{
    effects::{CommandResult, MatchmakingEffects},
    input::ScheduledTimer,
    MatchmakingSession,
};
use crate::{
    bot_runtime::{BotCommand, BotEvent, GameMode, MatchAttempt},
    matchmaking::{BotMatchPhase, MatchmakingPhase, MatchmakingPlan, StartMatchmakingInput},
};
use std::{
    path::PathBuf,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
use uuid::Uuid;

struct CommandGate {
    entered: tokio::sync::oneshot::Sender<()>,
    release: tokio::sync::oneshot::Receiver<()>,
}
struct ManualEffects {
    now: Mutex<Instant>,
    commands: Mutex<Vec<(String, BotCommand)>>,
    scheduled: Mutex<Vec<(Instant, ScheduledTimer)>>,
    command_gate: Mutex<Option<CommandGate>>,
}
impl ManualEffects {
    fn new() -> Self {
        Self {
            now: Mutex::new(Instant::now()),
            commands: Mutex::new(Vec::new()),
            scheduled: Mutex::new(Vec::new()),
            command_gate: Mutex::new(None),
        }
    }
    fn block_next_attempt(
        &self,
    ) -> (
        tokio::sync::oneshot::Receiver<()>,
        tokio::sync::oneshot::Sender<()>,
    ) {
        let (entered, entry) = tokio::sync::oneshot::channel();
        let (release, released) = tokio::sync::oneshot::channel();
        *self.command_gate.lock().unwrap() = Some(CommandGate {
            entered,
            release: released,
        });
        (entry, release)
    }
    fn drain(&self) -> Vec<(String, BotCommand)> {
        std::mem::take(&mut *self.commands.lock().unwrap())
    }
    async fn advance(&self, duration: Duration) {
        let end = self.now() + duration;
        loop {
            let next = {
                let mut pending = self.scheduled.lock().unwrap();
                let index = pending
                    .iter()
                    .enumerate()
                    .filter(|(_, (at, _))| *at <= end)
                    .min_by_key(|(_, (at, _))| *at)
                    .map(|(i, _)| i);
                index.map(|i| pending.remove(i))
            };
            let Some((at, task)) = next else {
                break;
            };
            *self.now.lock().unwrap() = at;
            task.deliver().await;
        }
        *self.now.lock().unwrap() = end;
    }
}
impl MatchmakingEffects for ManualEffects {
    fn command<'a>(&'a self, bot_id: &'a str, command: BotCommand) -> CommandResult<'a> {
        let gate = if matches!(&command, BotCommand::BeginMatchAttempt(_)) {
            self.command_gate.lock().unwrap().take()
        } else {
            None
        };
        Box::pin(async move {
            if let Some(gate) = gate {
                let _ = gate.entered.send(());
                let _ = gate.release.await;
            }
            self.commands
                .lock()
                .unwrap()
                .push((bot_id.to_owned(), command));
            Ok(())
        })
    }

    fn now(&self) -> Instant {
        *self.now.lock().unwrap()
    }
    fn schedule(&self, delay: Duration, task: ScheduledTimer) {
        self.scheduled
            .lock()
            .unwrap()
            .push((self.now() + delay, task));
    }
}
struct Fixture {
    session: Arc<MatchmakingSession>,
    effects: Arc<ManualEffects>,
    path: PathBuf,
}
impl Fixture {
    async fn start(mode: GameMode, count: usize, required: usize) -> Self {
        let path = std::env::temp_dir().join(format!("botting-match-{}.log", Uuid::new_v4()));
        std::fs::write(&path, "").unwrap();
        let effects = Arc::new(ManualEffects::new());
        let session = Arc::new(MatchmakingSession::with_effects(effects.clone()));
        let ids = (0..count).map(|i| format!("bot-{i}")).collect::<Vec<_>>();
        let plan = MatchmakingPlan::compile(
            StartMatchmakingInput {
                mode,
                bot_ids: ids.clone(),
                verify_presence: false,
                verify_duel_pitch: false,
                required_matches: required,
                log_path: path.to_string_lossy().into_owned(),
            },
            ids.iter().map(|id| (id.clone(), format!("Player_{id}"))),
        )
        .unwrap();
        session.start(plan).await.unwrap();
        let session_id = session.snapshot().await.session_id.unwrap();
        session
            .handle_player_log(
                &session_id,
                PlayerLogMessage::Line("[CHAT] Sending you to mini1!".to_owned()),
            )
            .await;
        Self {
            session,
            effects,
            path,
        }
    }
    async fn line(&self, line: &str) {
        let Some(session_id) = self.session.snapshot().await.session_id else {
            return;
        };
        self.session
            .handle_player_log(&session_id, PlayerLogMessage::Line(line.to_owned()))
            .await;
    }
    async fn restart(&self) -> String {
        let plan = MatchmakingPlan::compile(
            StartMatchmakingInput {
                mode: GameMode::DuelsUhc,
                bot_ids: vec!["bot-0".to_owned()],
                verify_presence: false,
                verify_duel_pitch: false,
                required_matches: 1,
                log_path: self.path.to_string_lossy().into_owned(),
            },
            [("bot-0".to_owned(), "Player_bot-0".to_owned())],
        )
        .unwrap();
        self.session.start(plan).await.unwrap().session_id.unwrap()
    }
    async fn close(self) {
        self.session.stop().await;
        std::fs::remove_file(&self.path).unwrap();
    }
    fn attempts(&self) -> Vec<(String, MatchAttempt)> {
        self.effects
            .drain()
            .into_iter()
            .filter_map(|(id, command)| match command {
                BotCommand::BeginMatchAttempt(attempt) => Some((id, attempt)),
                _ => None,
            })
            .collect()
    }
    async fn result(&self, id: &str, attempt: &MatchAttempt, server: &str) {
        self.session
            .handle_bot_event(&BotEvent::MatchAttemptResult {
                bot_id: id.to_owned(),
                session_id: attempt.session_id.clone(),
                round_id: attempt.round_id.clone(),
                target_generation: attempt.target_generation,
                attempt_id: attempt.attempt_id.clone(),
                server: server.to_owned(),
            })
            .await;
    }
    async fn match_bedwars(&self, id: &str, attempt: &MatchAttempt) {
        self.line("[CHAT] Player has joined (1/8)!").await;
        self.session
            .handle_bot_event(&BotEvent::QueueProgress {
                bot_id: id.to_owned(),
                current: 2,
                total: 8,
            })
            .await;
        self.result(id, attempt, "mini1").await;
    }
}

#[tokio::test]
async fn commit_withdraws_other_bots_and_ignores_late_results_and_timeouts() {
    let fixture = Fixture::start(GameMode::BedwarsDoubles, 2, 1).await;
    let attempts = fixture.attempts();
    fixture
        .result(&attempts[1].0, &attempts[1].1, "mini1")
        .await;
    fixture.match_bedwars(&attempts[0].0, &attempts[0].1).await;
    let snapshot = fixture.session.snapshot().await;
    assert_eq!(snapshot.phase, MatchmakingPhase::Committed);
    assert_eq!(snapshot.matched_bots, 1);
    assert_eq!(snapshot.bots[1].phase, BotMatchPhase::Returning);
    assert!(fixture
        .effects
        .drain()
        .iter()
        .any(|(id, cmd)| id == &attempts[1].0 && matches!(cmd, BotCommand::ReturnToLobby)));
    fixture.effects.advance(Duration::from_secs(3)).await;
    fixture
        .result(&attempts[1].0, &attempts[1].1, "mini1")
        .await;
    assert_eq!(fixture.session.snapshot().await.matched_bots, 1);
    assert!(fixture.effects.drain().is_empty());
    fixture.close().await;
}

#[tokio::test]
async fn early_game_start_clears_partial_matches_and_finish_prevents_log_restart() {
    let fixture = Fixture::start(GameMode::BedwarsDoubles, 2, 2).await;
    let attempts = fixture.attempts();
    fixture.match_bedwars(&attempts[0].0, &attempts[0].1).await;
    assert_eq!(fixture.session.snapshot().await.matched_bots, 1);
    fixture
        .line("[CHAT] Protect your bed and destroy the enemy beds!")
        .await;
    let snapshot = fixture.session.snapshot().await;
    assert_eq!(snapshot.phase, MatchmakingPhase::Failed);
    assert_eq!(snapshot.matched_bots, 0);
    assert!(fixture.session.finish_if_terminal().await);
    fixture.effects.drain();
    fixture.line("[CHAT] Sending you to mini2!").await;
    assert_eq!(
        fixture.session.snapshot().await.phase,
        MatchmakingPhase::Failed
    );
    assert!(fixture.effects.drain().is_empty());
    fixture.close().await;
}

#[tokio::test]
async fn timeout_prepares_limbo_and_acknowledgement_starts_a_new_attempt() {
    let fixture = Fixture::start(GameMode::DuelsUhc, 1, 1).await;
    let (id, attempt) = fixture.attempts().pop().unwrap();
    fixture.result(&id, &attempt, "mini1").await;
    fixture.effects.advance(Duration::from_millis(1999)).await;
    assert!(fixture.effects.drain().is_empty());
    fixture.effects.advance(Duration::from_millis(1)).await;
    let commands = fixture.effects.drain();
    let request_id = commands
        .into_iter()
        .find_map(|(_, command)| match command {
            BotCommand::PrepareMatchRetry { request_id } => Some(request_id),
            _ => None,
        })
        .expect("timeout requests limbo");
    fixture
        .session
        .handle_bot_event(&BotEvent::MatchRetryReady {
            bot_id: id.clone(),
            request_id,
        })
        .await;
    fixture.effects.advance(Duration::ZERO).await;
    let (_, next) = fixture.attempts().pop().expect("retry queues again");
    assert_ne!(attempt.attempt_id, next.attempt_id);
    fixture.result(&id, &attempt, "mini1").await;
    assert_eq!(fixture.session.snapshot().await.matched_bots, 0);
    fixture.close().await;
}

#[tokio::test]
async fn a_scheduled_retry_cannot_cross_into_a_new_round() {
    let fixture = Fixture::start(GameMode::DuelsUhc, 1, 1).await;
    let (id, attempt) = fixture.attempts().pop().unwrap();
    fixture
        .session
        .handle_bot_event(&BotEvent::MatchAttemptFailed {
            bot_id: id,
            session_id: attempt.session_id,
            round_id: attempt.round_id,
            target_generation: attempt.target_generation,
            attempt_id: attempt.attempt_id,
            code: "command_spam".to_owned(),
            message: "slow down".to_owned(),
        })
        .await;
    fixture.line("[CHAT] Sending you to mini2!").await;
    assert_eq!(fixture.attempts().len(), 1);
    fixture.effects.advance(Duration::from_secs(2)).await;
    assert!(fixture.attempts().is_empty());
    assert_eq!(
        fixture.session.snapshot().await.player_server.as_deref(),
        Some("mini2")
    );
    fixture.close().await;
}

#[tokio::test]
async fn stale_log_lines_and_errors_cannot_modify_a_restarted_session() {
    let fixture = Fixture::start(GameMode::DuelsUhc, 1, 1).await;
    let old = fixture.session.snapshot().await;
    let current_id = fixture.restart().await;
    fixture.effects.drain();
    fixture
        .session
        .handle_player_log(
            old.session_id.as_deref().unwrap(),
            PlayerLogMessage::Line("[CHAT] Sending you to mini9!".to_owned()),
        )
        .await;
    fixture
        .session
        .handle_player_log(
            old.session_id.as_deref().unwrap(),
            PlayerLogMessage::Unavailable("old file vanished".to_owned()),
        )
        .await;
    let current = fixture.session.snapshot().await;
    assert_eq!(current.session_id.as_deref(), Some(current_id.as_str()));
    assert_eq!(current.phase, MatchmakingPhase::AwaitingPlayer);
    assert!(current.player_server.is_none());
    assert!(fixture.effects.drain().is_empty());
    fixture.line("[CHAT] Sending you to mini2!").await;
    assert_eq!(fixture.attempts().len(), 1);
    fixture.close().await;
}

#[tokio::test]
async fn queued_old_consumer_cannot_deliver_into_a_new_session() {
    let fixture = Fixture::start(GameMode::DuelsUhc, 1, 1).await;
    let old_id = fixture.session.snapshot().await.session_id.unwrap();
    let (sender, receiver) = tokio::sync::mpsc::channel(2);
    sender
        .send(PlayerLogMessage::Line(
            "[CHAT] Sending you to mini9!".to_owned(),
        ))
        .await
        .unwrap();
    sender
        .send(PlayerLogMessage::Unavailable("old log".to_owned()))
        .await
        .unwrap();
    drop(sender);
    fixture.restart().await;
    fixture.effects.drain();
    fixture
        .session
        .spawn_log_consumer(old_id, receiver)
        .await
        .unwrap();
    assert_eq!(
        fixture.session.snapshot().await.phase,
        MatchmakingPhase::AwaitingPlayer
    );
    assert!(fixture.effects.drain().is_empty());
    fixture.close().await;
}

#[tokio::test]
async fn timer_due_after_stop_cannot_publish_or_send_new_commands() {
    let fixture = Fixture::start(GameMode::DuelsUhc, 1, 1).await;
    let (id, attempt) = fixture.attempts().pop().unwrap();
    fixture.result(&id, &attempt, "mini1").await;
    fixture.session.stop().await;
    fixture.effects.drain();
    let stopped = fixture.session.snapshot().await;
    fixture.effects.advance(Duration::from_secs(5)).await;
    assert!(fixture.effects.drain().is_empty());
    let after = fixture.session.snapshot().await;
    assert_eq!(after.revision, stopped.revision);
    assert_eq!(after.phase, MatchmakingPhase::Idle);
    fixture.close().await;
}

#[tokio::test]
async fn revisions_increase_across_events_stops_and_new_sessions() {
    let fixture = Fixture::start(GameMode::DuelsUhc, 1, 1).await;
    let first = fixture.session.snapshot().await;
    assert!(first.revision > 0);
    fixture.session.stop().await;
    let stopped = fixture.session.snapshot().await;
    assert!(stopped.revision > first.revision);
    let new_id = fixture.restart().await;
    let restarted = fixture.session.snapshot().await;
    assert!(restarted.revision > stopped.revision);
    assert_ne!(first.session_id.as_deref(), Some(new_id.as_str()));
    assert_eq!(
        fixture.session.subscribe().borrow().revision,
        restarted.revision
    );
    fixture.close().await;
}

#[tokio::test]
async fn stopping_waits_for_an_in_flight_timer_command_before_withdrawing() {
    use std::{future::Future, task::Poll};
    let fixture = Fixture::start(GameMode::DuelsUhc, 1, 1).await;
    let (id, attempt) = fixture.attempts().pop().unwrap();
    fixture
        .session
        .handle_bot_event(&BotEvent::MatchAttemptFailed {
            bot_id: id,
            session_id: attempt.session_id,
            round_id: attempt.round_id,
            target_generation: attempt.target_generation,
            attempt_id: attempt.attempt_id,
            code: "command_spam".to_owned(),
            message: "slow down".to_owned(),
        })
        .await;
    let (entered, release) = fixture.effects.block_next_attempt();
    let effects = fixture.effects.clone();
    let advancing = tokio::spawn(async move {
        effects.advance(Duration::from_secs(2)).await;
    });
    entered.await.unwrap();
    let mut stopping = Box::pin(fixture.session.stop());
    assert!(std::future::poll_fn(|cx| Poll::Ready(stopping.as_mut().poll(cx).is_pending())).await);
    release.send(()).unwrap();
    advancing.await.unwrap();
    stopping.await;
    let commands = fixture.effects.drain();
    assert!(matches!(
        commands.first().map(|(_, command)| command),
        Some(BotCommand::BeginMatchAttempt(_))
    ));
    let last_attempt = commands
        .iter()
        .rposition(|(_, command)| matches!(command, BotCommand::BeginMatchAttempt(_)))
        .unwrap();
    let withdrawal = commands
        .iter()
        .position(|(_, command)| matches!(command, BotCommand::ReturnToLobby))
        .unwrap();
    assert!(last_attempt < withdrawal);
    assert_eq!(
        fixture.session.snapshot().await.phase,
        MatchmakingPhase::Idle
    );
    fixture.close().await;
}
