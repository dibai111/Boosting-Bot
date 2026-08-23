use crate::hypixel;
use std::time::{Duration, Instant};

pub(crate) const CONFIRM_WINDOW: Duration = Duration::from_secs(2);
pub(crate) const CONFIRM_TIMEOUT: Duration = Duration::from_secs(2);
pub(crate) const PITCH_CONFIRM_TIMEOUT: Duration = Duration::from_secs(3);

#[derive(Clone, Copy)]
pub(crate) struct QueueObservation {
    pub current: u32,
    pub total: u32,
    pub observed_at: Instant,
    pub saw_waiting: bool,
}

pub(crate) fn next_observation(
    previous: Option<QueueObservation>,
    current: u32,
    total: u32,
) -> QueueObservation {
    QueueObservation {
        current,
        total,
        observed_at: Instant::now(),
        saw_waiting: previous.is_some_and(|value| value.saw_waiting)
            || (current == 1 && total == 2),
    }
}

pub(crate) fn is_candidate(
    player_server: Option<&str>,
    bot_server: Option<&str>,
    round_started_at: Option<Instant>,
    player: Option<QueueObservation>,
    bot: Option<QueueObservation>,
) -> bool {
    let Some(round_started_at) = round_started_at else {
        return false;
    };
    let Some(player_server) = player_server else {
        return false;
    };
    let Some(bot_server) = bot_server else {
        return false;
    };
    if !hypixel::server_matches(player_server, bot_server) {
        return false;
    }

    let Some(player) = player else {
        return false;
    };
    let Some(bot) = bot else {
        return false;
    };
    if player.current != 2
        || player.total != 2
        || !player.saw_waiting
        || bot.current != 2
        || bot.total != 2
        || player.observed_at < round_started_at
        || bot.observed_at < round_started_at
    {
        return false;
    }

    let difference = if player.observed_at >= bot.observed_at {
        player.observed_at.duration_since(bot.observed_at)
    } else {
        bot.observed_at.duration_since(player.observed_at)
    };
    difference <= CONFIRM_WINDOW
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_bot_that_only_observes_two_of_two() {
        let round_started_at = Instant::now();
        let player = QueueObservation {
            current: 2,
            total: 2,
            observed_at: round_started_at + Duration::from_millis(300),
            saw_waiting: true,
        };
        let bot = QueueObservation {
            current: 2,
            total: 2,
            observed_at: round_started_at + Duration::from_millis(550),
            saw_waiting: false,
        };

        assert!(is_candidate(
            Some("mini98CN"),
            Some("mini98CN"),
            Some(round_started_at),
            Some(player),
            Some(bot),
        ));
    }
}
