use super::{BotGamePhase, DuelPitchDirection, GameKind};
use crate::hypixel::{parse_visible_chat, VisibleChatSignal};
use std::{collections::HashMap, time::Duration, time::Instant};

const PITCH_THRESHOLD_DEGREES: f32 = 80.0;
const PITCH_STABLE_DURATION: Duration = Duration::from_millis(250);
const PITCH_EMIT_COOLDOWN: Duration = Duration::from_secs(1);

pub(super) fn game_state(kind: GameKind, message: &str) -> Option<BotGamePhase> {
    match parse_visible_chat(message) {
        Some(VisibleChatSignal::GameStarted(observed_kind)) if observed_kind == kind => {
            Some(BotGamePhase::Started)
        }
        Some(VisibleChatSignal::DuelEnded) if kind == GameKind::Duels => Some(BotGamePhase::Ended),
        Some(VisibleChatSignal::BedwarsOrSkywarsEnded) if kind != GameKind::Duels => {
            Some(BotGamePhase::Ended)
        }
        _ => None,
    }
}

pub(super) fn limbo_spawn(message: &str) -> bool {
    matches!(
        parse_visible_chat(message),
        Some(VisibleChatSignal::LimboSpawned)
    )
}

pub(super) fn server_transfer(message: &str) -> Option<String> {
    match parse_visible_chat(message) {
        Some(VisibleChatSignal::ServerTransfer(server)) => Some(server),
        _ => None,
    }
}

pub(super) fn queue_progress(message: &str) -> Option<(u32, u32)> {
    match parse_visible_chat(message) {
        Some(VisibleChatSignal::QueueProgress { current, total, .. }) => Some((current, total)),
        _ => None,
    }
}

pub(super) fn command_rejection(message: &str) -> Option<String> {
    match parse_visible_chat(message) {
        Some(VisibleChatSignal::CommandRejected(message)) => Some(message),
        _ => None,
    }
}

pub(super) fn safe_chat(message: &str) -> Option<String> {
    let plain = collapse_whitespace(message);
    if plain.eq_ignore_ascii_case("You are currently BUSY") {
        return None;
    }
    let normalized = plain.to_ascii_lowercase();
    if normalized.contains("bearer ")
        || normalized.contains("accesstoken")
        || normalized.contains("refreshtoken")
        || normalized.contains("cookie=")
        || plain.contains("eyJ")
        || normalized.contains("bott-")
    {
        return Some("[REDACTED]".to_owned());
    }
    (!plain.is_empty()).then_some(plain)
}

pub(super) fn disconnect_reason(message: Option<&str>) -> String {
    let Some(message) = message else {
        return "Disconnected by server".to_owned();
    };
    let visible = message
        .lines()
        .map(str::trim)
        .filter(|line| {
            !line.is_empty()
                && !line.starts_with("http://")
                && !line.starts_with("https://")
                && !line.starts_with("Find out more:")
                && !line.starts_with("Ban ID:")
                && !line.starts_with("Sharing your Ban ID")
        })
        .collect::<Vec<_>>()
        .join(" ");
    if visible.is_empty() {
        "Disconnected by server".to_owned()
    } else {
        visible
    }
}

pub(super) fn disconnect_code(message: &str) -> &'static str {
    let normalized = message.to_ascii_lowercase();
    if normalized.contains("banned") || normalized.contains("ban id") {
        "banned"
    } else {
        "kicked"
    }
}

pub(super) struct PitchTracker {
    states: HashMap<i32, PitchState>,
}

struct PitchState {
    saw_neutral: bool,
    direction: Option<DuelPitchDirection>,
    active_since: Option<Instant>,
    last_emitted_at: Option<Instant>,
}

impl PitchTracker {
    pub(super) fn new() -> Self {
        Self {
            states: HashMap::new(),
        }
    }

    pub(super) fn reset(&mut self) {
        self.states.clear();
    }

    pub(super) fn observe(
        &mut self,
        entity_id: i32,
        pitch: f32,
        now: Instant,
    ) -> Option<DuelPitchDirection> {
        let direction = pitch_direction(pitch);
        let state = self.states.entry(entity_id).or_insert(PitchState {
            saw_neutral: false,
            direction: None,
            active_since: None,
            last_emitted_at: None,
        });

        let Some(direction) = direction else {
            state.saw_neutral = true;
            state.direction = None;
            state.active_since = None;
            return None;
        };
        if !state.saw_neutral {
            return None;
        }
        if state.direction != Some(direction) {
            state.direction = Some(direction);
            state.active_since = Some(now);
            return None;
        }
        if state
            .active_since
            .is_none_or(|started| now.duration_since(started) < PITCH_STABLE_DURATION)
            || state
                .last_emitted_at
                .is_some_and(|last| now.duration_since(last) < PITCH_EMIT_COOLDOWN)
        {
            return None;
        }

        state.last_emitted_at = Some(now);
        Some(direction)
    }
}

fn pitch_direction(pitch: f32) -> Option<DuelPitchDirection> {
    if !pitch.is_finite() || pitch.abs() <= PITCH_THRESHOLD_DEGREES {
        None
    } else if pitch < 0.0 {
        Some(DuelPitchDirection::Up)
    } else {
        Some(DuelPitchDirection::Down)
    }
}

fn collapse_whitespace(message: &str) -> String {
    message.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_only_the_selected_game_start_markers() {
        assert_eq!(
            game_state(GameKind::Duels, "Opponent: Player"),
            Some(BotGamePhase::Started)
        );
        assert_eq!(
            game_state(GameKind::Skywars, "Cages opened! FIGHT!"),
            Some(BotGamePhase::Started)
        );
        assert_eq!(
            game_state(
                GameKind::Bedwars,
                "Protect your bed and destroy the enemy beds."
            ),
            Some(BotGamePhase::Started)
        );
        assert_eq!(game_state(GameKind::Skywars, "The game has started"), None);
    }

    #[test]
    fn detects_limbo_spawn_confirmation_from_chat() {
        assert!(limbo_spawn("You were spawned in Limbo."));
        assert!(limbo_spawn("YOU WERE SPAWNED IN LIMBO"));
        assert!(!limbo_spawn("/limbo for more information."));
    }

    #[test]
    fn parses_transfer_and_queue_messages() {
        assert_eq!(
            server_transfer("Sending you to mini198Q!"),
            Some("mini198Q".to_owned())
        );
        assert_eq!(queue_progress("Player has joined (2/8)!"), Some((2, 8)));
    }

    #[test]
    fn pitch_requires_neutral_then_a_stable_gesture() {
        let start = Instant::now();
        let mut tracker = PitchTracker::new();
        assert_eq!(tracker.observe(1, 0.0, start), None);
        assert_eq!(tracker.observe(1, -85.0, start), None);
        assert_eq!(
            tracker.observe(1, -85.0, start + PITCH_STABLE_DURATION),
            Some(DuelPitchDirection::Up)
        );
    }

    #[test]
    fn redacts_sensitive_chat() {
        assert_eq!(safe_chat("Bearer secret"), Some("[REDACTED]".to_owned()));
        assert_eq!(safe_chat("You are currently BUSY"), None);
    }

    #[test]
    fn classifies_ban_disconnects_separately_from_other_kicks() {
        assert_eq!(
            disconnect_code("You are permanently banned from this server!"),
            "banned"
        );
        assert_eq!(disconnect_code("Kicked by an administrator"), "kicked");
    }
}
