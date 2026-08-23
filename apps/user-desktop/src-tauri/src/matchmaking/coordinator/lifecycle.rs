use super::super::{BotMatchPhase, MatchmakingPhase};
use super::flow::{possible_matches, selected_bot_ids, waiting_bot};
use super::MatchmakingSession;
use crate::bot_runtime::{BotCommand, BotGamePhase};

impl MatchmakingSession {
    pub(crate) async fn handle_player_lobby_joined(&self) {
        let has_active_target = self.state.lock().await.snapshot.player_server.is_some();
        if has_active_target {
            self.reset_for_lobby("Player returned to the lobby").await;
        }
    }

    pub(crate) async fn handle_bot_game_state(
        &self,
        bot_id: &str,
        round_id: &str,
        generation: u64,
        game_state: BotGamePhase,
    ) {
        let accepted = {
            let state = self.state.lock().await;
            state.snapshot.round_id.as_deref() == Some(round_id)
                && state.target_generation == generation
                && state.snapshot.bots.iter().any(|bot| {
                    bot.bot_id == bot_id
                        && matches!(bot.phase, BotMatchPhase::Matched | BotMatchPhase::Afk)
                })
        };
        if !accepted {
            return;
        }
        match game_state {
            BotGamePhase::Started => self.handle_game_started().await,
            BotGamePhase::Ended => self.reset_for_lobby("Game ended").await,
        }
    }

    pub(crate) async fn handle_game_started(&self) {
        let (should_fail, snapshot) = {
            let mut state = self.state.lock().await;
            if !matches!(
                state.snapshot.phase,
                MatchmakingPhase::Matching | MatchmakingPhase::Committed
            ) {
                return;
            }
            let matched = state
                .snapshot
                .bots
                .iter()
                .filter(|bot| bot.phase == BotMatchPhase::Matched)
                .count();
            let should_fail = matched < state.snapshot.required_matches;
            if !should_fail {
                state.snapshot.phase = MatchmakingPhase::InGame;
                state.snapshot.message = Some("Game started".to_owned());
                for bot in &mut state.snapshot.bots {
                    if bot.phase == BotMatchPhase::Matched {
                        bot.phase = BotMatchPhase::Afk;
                    }
                }
            }
            (should_fail, state.snapshot.clone())
        };
        if should_fail {
            self.fail_round("Game started before the minimum bots matched")
                .await;
            return;
        }
        self.publish(snapshot);
    }

    pub(crate) async fn mark_unavailable(&self, bot_id: &str, message: &str) {
        let (should_fail, snapshot) = {
            let mut state = self.state.lock().await;
            if !matches!(
                state.snapshot.phase,
                MatchmakingPhase::Matching | MatchmakingPhase::Committed | MatchmakingPhase::InGame
            ) {
                return;
            }
            state.active_attempts.remove(bot_id);
            state.retry_requests.remove(bot_id);
            state.clear_bot_attempt_tracking(bot_id);
            let Some(bot) = state
                .snapshot
                .bots
                .iter_mut()
                .find(|bot| bot.bot_id == bot_id)
            else {
                return;
            };
            bot.phase = BotMatchPhase::Unavailable;
            bot.message = Some(message.to_owned());
            let should_fail = state.snapshot.phase == MatchmakingPhase::Matching
                && possible_matches(&state.snapshot) < state.snapshot.required_matches;
            (should_fail, state.snapshot.clone())
        };
        self.publish(snapshot);
        if should_fail {
            self.fail_round("Not enough available bots to reach the minimum")
                .await;
        }
    }

    pub(crate) async fn fail_round(&self, message: impl Into<String>) {
        let bot_ids = {
            let mut state = self.state.lock().await;
            if state.plan.is_none() {
                return;
            }
            state.reset_round();
            state.snapshot.phase = MatchmakingPhase::Failed;
            state.snapshot.message = Some(message.into());
            for bot in &mut state.snapshot.bots {
                if bot.phase != BotMatchPhase::Unavailable {
                    bot.phase = BotMatchPhase::Returning;
                }
            }
            let ids = selected_bot_ids(&state);
            self.publish(state.snapshot.clone());
            ids
        };
        self.withdraw_bots(&bot_ids).await;
    }

    pub(crate) async fn reset_for_lobby(&self, message: &str) {
        let bot_ids = {
            let mut state = self.state.lock().await;
            if state.plan.is_none() {
                return;
            }
            let active_round = state.snapshot.round_id.is_some()
                || !matches!(state.snapshot.phase, MatchmakingPhase::AwaitingPlayer);
            state.reset_round();
            state.snapshot.round_id = None;
            state.snapshot.phase = MatchmakingPhase::AwaitingPlayer;
            state.snapshot.player_server = None;
            state.snapshot.matched_bots = 0;
            state.snapshot.message = Some(message.to_owned());
            for bot in &mut state.snapshot.bots {
                *bot = waiting_bot(&bot.bot_id);
            }
            let ids = if active_round {
                selected_bot_ids(&state)
            } else {
                Vec::new()
            };
            self.publish(state.snapshot.clone());
            ids
        };
        self.withdraw_bots(&bot_ids).await;
    }

    pub(crate) async fn withdraw_bots(&self, bot_ids: &[String]) {
        for bot_id in bot_ids {
            let _ = self
                .bots
                .command(bot_id, BotCommand::CancelMatchAttempt)
                .await;
            let _ = self.bots.command(bot_id, BotCommand::ReturnToLobby).await;
        }
    }
}
