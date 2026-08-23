use super::super::{BotMatchPhase, MatchmakingBotState, MatchmakingSnapshot};
use super::state::SessionState;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

pub(super) fn random_command_spam_retry_delay() -> Duration {
    let tick = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    const MIN_MS: u64 = 800;
    const RANGE_MS: u128 = 201;
    Duration::from_millis(MIN_MS + (tick % RANGE_MS) as u64)
}

pub(super) fn retry_message(message: &str, delay: Duration) -> String {
    if delay.is_zero() {
        format!("{message}; retrying now")
    } else {
        format!("{message}; retrying in {:.2} seconds", delay.as_secs_f32())
    }
}

pub(super) fn waiting_bot(bot_id: &str) -> MatchmakingBotState {
    MatchmakingBotState {
        bot_id: bot_id.to_owned(),
        phase: BotMatchPhase::Waiting,
        server: None,
        attempts: 0,
        message: None,
    }
}

pub(super) fn matched_count(snapshot: &MatchmakingSnapshot) -> usize {
    snapshot
        .bots
        .iter()
        .filter(|bot| matches!(bot.phase, BotMatchPhase::Matched | BotMatchPhase::Afk))
        .count()
}

pub(super) fn possible_matches(snapshot: &MatchmakingSnapshot) -> usize {
    snapshot
        .bots
        .iter()
        .filter(|bot| bot.phase != BotMatchPhase::Unavailable)
        .count()
}

pub(super) fn should_withdraw_after_commit(phase: BotMatchPhase) -> bool {
    matches!(
        phase,
        BotMatchPhase::Waiting | BotMatchPhase::Queued | BotMatchPhase::Returning
    )
}

pub(super) fn selected_bot_ids(state: &SessionState) -> Vec<String> {
    state
        .plan
        .as_ref()
        .map(|plan| plan.bot_ids().map(str::to_owned).collect())
        .unwrap_or_default()
}
