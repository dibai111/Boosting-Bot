use super::super::detector::{chat_verification, duel_pitch};
use super::super::modes::{bedwars, duels};
use super::super::MatchmakingPhase;
use super::state::PresenceCheck;
use super::MatchmakingSession;
use crate::bot_runtime::{BotCommand, DuelPitchDirection};
use std::sync::Arc;
use std::time::{Duration, Instant};

impl MatchmakingSession {
    pub(crate) async fn handle_duel_pitch_observed(
        self: &Arc<Self>,
        bot_id: &str,
        round_id: &str,
        generation: u64,
        attempt_id: &str,
        direction: &DuelPitchDirection,
        pitch: f32,
    ) {
        let should_match = {
            let mut state = self.state.lock().await;
            let valid = state.snapshot.phase == MatchmakingPhase::Matching
                && state.snapshot.round_id.as_deref() == Some(round_id)
                && state.target_generation == generation
                && state
                    .active_attempts
                    .get(bot_id)
                    .is_some_and(|current| current == attempt_id)
                && state
                    .plan
                    .as_ref()
                    .is_some_and(|plan| plan.requires_pitch_verification());
            if !valid {
                false
            } else {
                state.duel_pitch_observations.insert(
                    bot_id.to_owned(),
                    duel_pitch::Observation {
                        generation,
                        attempt_id: attempt_id.to_owned(),
                        observed_at: Instant::now(),
                    },
                );
                let should_match = state.duel_pitch_checks.remove(bot_id).is_some();
                if should_match {
                    if let Some(bot) = state
                        .snapshot
                        .bots
                        .iter_mut()
                        .find(|bot| bot.bot_id == bot_id)
                    {
                        bot.message = Some("Pitch verification confirmed".to_owned());
                    }
                }
                should_match
            }
        };
        self.diagnose(
            Some(bot_id),
            format!(
                "[duels pitch] Rust received gesture: direction={}, pitch={:.1}deg, accepted={}",
                direction.as_str(),
                pitch.to_degrees(),
                should_match
            ),
        );
        if should_match {
            let server = self
                .state
                .lock()
                .await
                .snapshot
                .bots
                .iter()
                .find(|bot| bot.bot_id == bot_id)
                .and_then(|bot| bot.server.clone());
            if let Some(server) = server {
                self.mark_matched(bot_id, Some(&server)).await;
            }
        }
    }

    pub(crate) async fn has_duel_pitch_observation(&self, bot_id: &str) -> bool {
        let state = self.state.lock().await;
        let Some(observation) = state.duel_pitch_observations.get(bot_id) else {
            return false;
        };
        let Some(attempt_id) = state.active_attempts.get(bot_id) else {
            return false;
        };
        duel_pitch::belongs_to_attempt(
            observation,
            state.target_generation,
            attempt_id,
            state.round_started_at,
        )
    }

    pub(crate) async fn start_duel_pitch_check(self: &Arc<Self>, bot_id: &str, server: &str) {
        let (generation, attempt_id) = {
            let mut state = self.state.lock().await;
            if state.duel_pitch_checks.contains_key(bot_id) {
                return;
            }
            let Some(attempt_id) = state.active_attempts.get(bot_id).cloned() else {
                return;
            };
            let generation = state.target_generation;
            state.duel_pitch_checks.insert(
                bot_id.to_owned(),
                duel_pitch::VerificationCheck {
                    generation,
                    attempt_id: attempt_id.clone(),
                    server: server.to_owned(),
                },
            );
            if let Some(bot) = state
                .snapshot
                .bots
                .iter_mut()
                .find(|bot| bot.bot_id == bot_id)
            {
                bot.message = Some("Waiting for pitch verification".to_owned());
            }
            (generation, attempt_id)
        };
        self.publish(self.state.lock().await.snapshot.clone());
        self.diagnose(
            Some(bot_id),
            format!("[duels pitch] verification gate opened for server={server}"),
        );
        self.spawn_duel_pitch_confirmation_timeout(bot_id.to_owned(), generation, attempt_id);
    }

    pub(crate) fn spawn_duel_pitch_confirmation_timeout(
        self: &Arc<Self>,
        bot_id: String,
        generation: u64,
        attempt_id: String,
    ) {
        let coordinator = Arc::downgrade(self);
        tokio::spawn(async move {
            tokio::time::sleep(duels::PITCH_CONFIRM_TIMEOUT).await;
            let Some(coordinator) = coordinator.upgrade() else {
                return;
            };
            let retry_server = {
                let state = coordinator.state.lock().await;
                state.duel_pitch_checks.get(&bot_id).and_then(|check| {
                    (check.generation == generation
                        && check.attempt_id == attempt_id
                        && state.snapshot.phase == MatchmakingPhase::Matching)
                        .then(|| check.server.clone())
                })
            };
            if let Some(server) = retry_server {
                coordinator.diagnose(
                    Some(&bot_id),
                    "[duels pitch] timed out without a valid gesture",
                );
                coordinator
                    .schedule_retry(
                        &bot_id,
                        Some(server),
                        "Pitch verification not confirmed",
                        Duration::ZERO,
                        true,
                    )
                    .await;
            }
        });
    }

    pub(crate) async fn handle_player_chat(self: &Arc<Self>, username: &str, _message: &str) {
        let matched_bot = {
            let state = self.state.lock().await;
            state
                .presence_checks
                .iter()
                .find(|(_, check)| chat_verification::matches(&check.username, username))
                .map(|(bot_id, check)| (bot_id.clone(), check.server.clone()))
        };
        let Some((bot_id, server)) = matched_bot else {
            return;
        };
        {
            let mut state = self.state.lock().await;
            state.presence_checks.remove(&bot_id);
        }
        self.mark_matched(&bot_id, Some(&server)).await;
    }

    pub(crate) async fn start_presence_check(self: &Arc<Self>, bot_id: &str, server: &str) {
        let (command, generation, attempt_id) = {
            let mut state = self.state.lock().await;
            if state.presence_checks.contains_key(bot_id) {
                return;
            }
            let Some(username) = state
                .plan
                .as_ref()
                .and_then(|plan| plan.username(bot_id).map(str::to_owned))
            else {
                return;
            };
            let Some(attempt_id) = state.active_attempts.get(bot_id).cloned() else {
                return;
            };
            let phrase = chat_verification::random_phrase();
            let generation = state.target_generation;
            state.presence_checks.insert(
                bot_id.to_owned(),
                PresenceCheck {
                    username,
                    generation,
                    attempt_id: attempt_id.clone(),
                    server: server.to_owned(),
                },
            );
            if let Some(bot) = state
                .snapshot
                .bots
                .iter_mut()
                .find(|bot| bot.bot_id == bot_id)
            {
                bot.message = Some("Waiting for chat confirmation".to_owned());
            }
            (BotCommand::SendChat(phrase), generation, attempt_id)
        };
        self.publish(self.state.lock().await.snapshot.clone());
        if let Err(error) = self.bots.command(bot_id, command).await {
            {
                let mut state = self.state.lock().await;
                state.presence_checks.remove(bot_id);
                if let Some(bot) = state
                    .snapshot
                    .bots
                    .iter_mut()
                    .find(|bot| bot.bot_id == bot_id)
                {
                    bot.message = Some("Chat verification could not be sent".to_owned());
                }
            }
            self.schedule_retry(
                bot_id,
                Some(server.to_owned()),
                &format!("Chat verification failed: {error}"),
                Duration::ZERO,
                true,
            )
            .await;
            return;
        }
        {
            let mut state = self.state.lock().await;
            if state.presence_checks.get(bot_id).is_some_and(|check| {
                check.generation == generation && check.attempt_id == attempt_id
            }) {
                if let Some(bot) = state
                    .snapshot
                    .bots
                    .iter_mut()
                    .find(|bot| bot.bot_id == bot_id)
                {
                    bot.message = Some("Chat verification sent".to_owned());
                }
                self.publish(state.snapshot.clone());
            }
        }
        self.diagnose(
            Some(bot_id),
            "[chat] scan: chat verification sent; waiting for player log",
        );
        self.spawn_presence_timeout(bot_id.to_owned(), generation, attempt_id);
    }

    pub(crate) fn spawn_presence_timeout(
        self: &Arc<Self>,
        bot_id: String,
        generation: u64,
        attempt_id: String,
    ) {
        let coordinator = Arc::downgrade(self);
        tokio::spawn(async move {
            tokio::time::sleep(bedwars::CONFIRM_TIMEOUT).await;
            let Some(coordinator) = coordinator.upgrade() else {
                return;
            };
            let retry_server = {
                let state = coordinator.state.lock().await;
                state.presence_checks.get(&bot_id).and_then(|check| {
                    (check.generation == generation
                        && check.attempt_id == attempt_id
                        && state.snapshot.phase == MatchmakingPhase::Matching)
                        .then(|| check.server.clone())
                })
            };
            if let Some(server) = retry_server {
                coordinator
                    .schedule_retry(
                        &bot_id,
                        Some(server),
                        "Chat presence not confirmed",
                        Duration::ZERO,
                        true,
                    )
                    .await;
            }
        });
    }
}
