use super::super::detector::real_name_joined_log;
use super::super::modes::{bedwars, duels};
use super::super::{BotMatchPhase, MatchmakingPhase};
use super::flow::{matched_count, random_command_spam_retry_delay, should_withdraw_after_commit};
use super::MatchmakingSession;
use crate::{
    bot_runtime::{BotCommand, GameKind, MatchAttempt},
    hypixel,
};
use std::sync::Arc;
use std::time::{Duration, Instant};
use uuid::Uuid;

impl MatchmakingSession {
    pub(crate) async fn handle_player_server(self: &Arc<Self>, server: String) {
        let current = {
            let state = self.state.lock().await;
            (state.snapshot.phase, state.snapshot.player_server.clone())
        };

        match current {
            (MatchmakingPhase::AwaitingPlayer | MatchmakingPhase::Failed, _)
                if hypixel::is_game_server(&server) =>
            {
                self.start_round(server).await;
            }
            (
                MatchmakingPhase::Matching | MatchmakingPhase::Committed | MatchmakingPhase::InGame,
                Some(target),
            ) if !hypixel::server_matches(&target, &server) && hypixel::is_game_server(&server) => {
                self.restart_for_new_player_server(server).await;
            }
            _ => {}
        }
    }

    pub(crate) async fn restart_for_new_player_server(self: &Arc<Self>, server: String) {
        self.reset_for_lobby("Player server changed; restarting matchmaking")
            .await;
        self.start_round(server).await;
    }

    pub(crate) async fn start_round(self: &Arc<Self>, player_server: String) {
        let (commands, snapshot) = {
            let mut state = self.state.lock().await;
            let Some(plan) = state.plan.as_ref() else {
                return;
            };
            let mode = plan.mode();
            let requires_pitch_verification = plan.requires_pitch_verification();
            let Some(session_id) = state.snapshot.session_id.clone() else {
                return;
            };

            state.reset_round();
            let generation = state.target_generation;
            let round_id = Uuid::new_v4().to_string();
            state.round_started_at = Some(Instant::now());
            state.snapshot.round_id = Some(round_id.clone());
            state.snapshot.phase = MatchmakingPhase::Matching;
            state.snapshot.player_server = Some(player_server);
            state.snapshot.matched_bots = 0;
            state.snapshot.message = Some("Matching bots to the player server".to_owned());

            let mut commands = Vec::with_capacity(state.snapshot.bots.len());
            let mut attempt_ids = Vec::with_capacity(state.snapshot.bots.len());
            for bot in &mut state.snapshot.bots {
                bot.phase = BotMatchPhase::Queued;
                bot.server = None;
                bot.attempts = 0;
                bot.message = None;
                let attempt_id = Uuid::new_v4().to_string();
                attempt_ids.push((bot.bot_id.clone(), attempt_id.clone()));
                commands.push((
                    bot.bot_id.clone(),
                    BotCommand::BeginMatchAttempt(MatchAttempt {
                        session_id: session_id.clone(),
                        round_id: round_id.clone(),
                        target_generation: generation,
                        attempt_id,
                        mode,
                        requires_pitch_verification,
                    }),
                ));
            }
            state.active_attempts.extend(attempt_ids);
            (commands, state.snapshot.clone())
        };

        self.publish(snapshot);
        for (bot_id, command) in commands {
            let _ = self.bots.command(&bot_id, command).await;
        }
    }

    pub(crate) async fn handle_attempt_result(
        self: &Arc<Self>,
        bot_id: &str,
        session_id: &str,
        round_id: &str,
        generation: u64,
        attempt_id: &str,
        server: &str,
    ) {
        let (
            target_matches,
            duel_candidate,
            bedwars_candidate,
            duel_mode,
            bedwars_mode,
            skywars_mode,
            skywars_name_candidate,
        ) = {
            let mut state = self.state.lock().await;
            let is_current = state.snapshot.session_id.as_deref() == Some(session_id)
                && state.snapshot.round_id.as_deref() == Some(round_id)
                && state.target_generation == generation
                && state
                    .active_attempts
                    .get(bot_id)
                    .is_some_and(|current| current == attempt_id);
            if !is_current {
                return;
            }
            let mode = state.plan.as_ref().map(|plan| plan.mode());
            let duel_mode = mode.is_some_and(|mode| mode.kind() == GameKind::Duels);
            let bedwars_mode = mode.is_some_and(|mode| mode.kind() == GameKind::Bedwars);
            let skywars_mode = mode.is_some_and(|mode| mode.kind() == GameKind::Skywars);
            let target_matches = state
                .snapshot
                .player_server
                .as_deref()
                .is_some_and(|target| {
                    mode.is_some_and(|_| hypixel::server_matches(target, server))
                });
            let skywars_name_candidate = skywars_mode
                && state
                    .plan
                    .as_ref()
                    .and_then(|plan| plan.username(bot_id))
                    .is_some_and(|name| {
                        state
                            .player_queue_usernames
                            .iter()
                            .any(|username| real_name_joined_log::matches(Some(name), username))
                    });
            if skywars_mode {
                state.bot_transfers.insert(bot_id.to_owned());
            }
            if let Some(bot) = state
                .snapshot
                .bots
                .iter_mut()
                .find(|bot| bot.bot_id == bot_id)
            {
                bot.attempts = bot.attempts.saturating_add(1);
                if skywars_mode {
                    bot.server = None;
                    bot.message = Some("Waiting for player name".to_owned());
                } else if target_matches && (duel_mode || bedwars_mode) {
                    bot.server = Some(server.to_owned());
                    bot.message = Some("Waiting for queue confirmation".to_owned());
                }
            }
            let duel_candidate = target_matches
                && duel_mode
                && duels::is_candidate(
                    state.snapshot.player_server.as_deref(),
                    Some(server),
                    state.round_started_at,
                    state.player_queue,
                    state.bot_queues.get(bot_id).copied(),
                );
            let bedwars_candidate = target_matches
                && bedwars_mode
                && bedwars::is_candidate(
                    state.snapshot.player_server.as_deref(),
                    Some(server),
                    state.round_started_at,
                    state.player_bedwars_queue,
                    state.bot_bedwars_queues.get(bot_id).copied(),
                );
            (
                target_matches,
                duel_candidate,
                bedwars_candidate,
                duel_mode,
                bedwars_mode,
                skywars_mode,
                skywars_name_candidate,
            )
        };

        if skywars_mode && skywars_name_candidate {
            self.mark_matched(bot_id, None).await;
        } else if skywars_mode {
            self.spawn_skywars_name_confirmation_timeout(
                bot_id.to_owned(),
                generation,
                attempt_id.to_owned(),
            );
        } else if target_matches && duel_mode && duel_candidate {
            let pitch_enabled = {
                let state = self.state.lock().await;
                state
                    .plan
                    .as_ref()
                    .is_some_and(|plan| plan.requires_pitch_verification())
            };
            if pitch_enabled {
                if self.has_duel_pitch_observation(bot_id).await {
                    self.mark_matched(bot_id, Some(server)).await;
                } else {
                    self.start_duel_pitch_check(bot_id, server).await;
                }
            } else {
                self.mark_matched(bot_id, Some(server)).await;
            }
        } else if target_matches && duel_mode {
            self.spawn_duels_queue_confirmation_timeout(
                bot_id.to_owned(),
                generation,
                attempt_id.to_owned(),
                server.to_owned(),
            );
        } else if target_matches && bedwars_mode && bedwars_candidate {
            self.confirm_queue_candidates().await;
        } else if target_matches && bedwars_mode {
            self.spawn_bedwars_queue_confirmation_timeout(
                bot_id.to_owned(),
                generation,
                attempt_id.to_owned(),
                server.to_owned(),
            );
        } else {
            self.schedule_retry(
                bot_id,
                Some(server.to_owned()),
                "Different server",
                Duration::ZERO,
                true,
            )
            .await;
        }
    }

    pub(crate) async fn handle_attempt_failure(
        self: &Arc<Self>,
        bot_id: &str,
        round_id: &str,
        generation: u64,
        attempt_id: &str,
        code: &str,
        message: &str,
    ) {
        if self
            .is_current_attempt(bot_id, round_id, generation, attempt_id)
            .await
        {
            let retry_delay = if code == "command_spam" {
                random_command_spam_retry_delay()
            } else {
                Duration::ZERO
            };
            self.schedule_retry(bot_id, None, message, retry_delay, code != "command_spam")
                .await;
        }
    }

    pub(crate) async fn is_current_attempt(
        &self,
        bot_id: &str,
        round_id: &str,
        generation: u64,
        attempt_id: &str,
    ) -> bool {
        let state = self.state.lock().await;
        state.snapshot.phase == MatchmakingPhase::Matching
            && state.snapshot.round_id.as_deref() == Some(round_id)
            && state.target_generation == generation
            && state
                .active_attempts
                .get(bot_id)
                .is_some_and(|current| current == attempt_id)
    }

    pub(crate) async fn mark_matched(&self, bot_id: &str, server: Option<&str>) {
        let (snapshot, withdraw) = {
            let mut state = self.state.lock().await;
            state.active_attempts.remove(bot_id);
            state.presence_checks.remove(bot_id);
            state.duel_pitch_checks.remove(bot_id);
            state.duel_pitch_observations.remove(bot_id);
            let Some(bot) = state
                .snapshot
                .bots
                .iter_mut()
                .find(|bot| bot.bot_id == bot_id)
            else {
                return;
            };
            bot.phase = BotMatchPhase::Matched;
            bot.server = server.map(str::to_owned);
            bot.message = Some(if server.is_some() {
                "Same server".to_owned()
            } else {
                "Player name matched".to_owned()
            });
            state.snapshot.matched_bots = matched_count(&state.snapshot);

            let mut withdraw = Vec::new();
            if state.snapshot.matched_bots >= state.snapshot.required_matches {
                state.snapshot.phase = MatchmakingPhase::Committed;
                state.snapshot.message =
                    Some("Required bots matched; waiting for game start".to_owned());
                for bot in &mut state.snapshot.bots {
                    if should_withdraw_after_commit(bot.phase) {
                        bot.phase = BotMatchPhase::Returning;
                        bot.message = Some("Minimum reached; returning to lobby".to_owned());
                        withdraw.push(bot.bot_id.clone());
                    }
                }
                state.active_attempts.clear();
                state.retry_requests.clear();
                state.duel_pitch_checks.clear();
                state.duel_pitch_observations.clear();
            }
            (state.snapshot.clone(), withdraw)
        };
        self.publish(snapshot);
        self.withdraw_bots(&withdraw).await;
    }
}
