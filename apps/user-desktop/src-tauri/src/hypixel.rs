use crate::bot_runtime::GameKind;

/// 玩家可見聊天文字中的共用 Hypixel 訊號，不包含來源格式或敏感資料處理。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum VisibleChatSignal {
    ServerTransfer(String),
    QueueProgress {
        username: String,
        current: u32,
        total: u32,
    },
    GameStarted(GameKind),
    DuelEnded,
    BedwarsOrSkywarsEnded,
    LimboSpawned,
    CommandRejected(String),
}

pub(crate) fn parse_visible_chat(message: &str) -> Option<VisibleChatSignal> {
    let plain = collapse_whitespace(message);
    if plain.is_empty() {
        return None;
    }
    if normalize_message(&plain) == "you were spawned in limbo" {
        return Some(VisibleChatSignal::LimboSpawned);
    }
    if let Some(server) = parse_transfer(&plain) {
        return Some(VisibleChatSignal::ServerTransfer(server));
    }
    if let Some((username, current, total)) = parse_queue_progress(&plain) {
        return Some(VisibleChatSignal::QueueProgress {
            username,
            current,
            total,
        });
    }
    if let Some(kind) = game_started(&plain) {
        return Some(VisibleChatSignal::GameStarted(kind));
    }
    if game_ended(&plain) {
        return Some(VisibleChatSignal::BedwarsOrSkywarsEnded);
    }
    if normalize_message(&plain).contains("reward summary") {
        return Some(VisibleChatSignal::DuelEnded);
    }
    command_rejection(&plain).map(VisibleChatSignal::CommandRejected)
}

pub(crate) fn is_game_server(server: &str) -> bool {
    server.get(..4).is_some_and(|prefix| {
        prefix.eq_ignore_ascii_case("mini")
            && server.get(4..).is_some_and(|suffix| {
                suffix.starts_with(|character: char| character.is_ascii_digit())
                    && suffix
                        .chars()
                        .all(|character| character.is_ascii_alphanumeric())
            })
    })
}

pub(crate) fn server_matches(target: &str, candidate: &str) -> bool {
    target.eq_ignore_ascii_case(candidate)
}

fn parse_transfer(message: &str) -> Option<String> {
    const PREFIX: &str = "Sending you to ";
    let prefix = message.get(..PREFIX.len())?;
    if !prefix.eq_ignore_ascii_case(PREFIX) {
        return None;
    }
    let server = message[PREFIX.len()..]
        .trim_end_matches(|character: char| character.is_ascii_punctuation())
        .trim();
    let suffix = server.get(4..)?;
    is_game_server(server).then(|| format!("mini{suffix}"))
}

fn parse_queue_progress(message: &str) -> Option<(String, u32, u32)> {
    const MARKER: &str = " has joined (";
    let lowercase = message.to_ascii_lowercase();
    let marker_start = lowercase.find(MARKER)?;
    let values_start = marker_start + MARKER.len();
    let values_end = lowercase[values_start..].find(')')? + values_start;
    let (current, total) = lowercase[values_start..values_end].split_once('/')?;
    let current = current.trim().parse().ok()?;
    let total = total.trim().parse().ok()?;
    let username = message[..marker_start]
        .split_whitespace()
        .last()?
        .to_owned();
    (total > 0
        && current <= total
        && username
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || character == '_'))
    .then_some((username, current, total))
}

fn game_started(message: &str) -> Option<GameKind> {
    let normalized = normalize_message(message);
    if normalized.starts_with("opponent") {
        Some(GameKind::Duels)
    } else if normalized.starts_with("cages opened fight") {
        Some(GameKind::Skywars)
    } else if normalized.starts_with("protect your bed and destroy the enemy beds") {
        Some(GameKind::Bedwars)
    } else {
        None
    }
}

fn game_ended(message: &str) -> bool {
    let normalized = normalize_message(message);
    normalized.starts_with("victory")
        || normalized.starts_with("game over")
        || ["1st killer", "2nd killer", "3rd killer"]
            .iter()
            .any(|marker| normalized.starts_with(marker))
}

fn command_rejection(message: &str) -> Option<String> {
    let normalized = message.to_ascii_lowercase();
    (normalized.contains("please don't spam the command")
        || normalized.contains("please do not spam the command")
        || normalized.contains("sending commands too fast"))
    .then(|| message.to_owned())
}

fn normalize_message(message: &str) -> String {
    let without_punctuation = message
        .chars()
        .map(|character| {
            if character.is_ascii_punctuation() {
                ' '
            } else {
                character
            }
        })
        .collect::<String>();
    collapse_whitespace(&without_punctuation).to_ascii_lowercase()
}

fn collapse_whitespace(message: &str) -> String {
    message.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_the_shared_transfer_and_queue_rules() {
        assert_eq!(
            parse_visible_chat("Sending you to MINI198Q!"),
            Some(VisibleChatSignal::ServerTransfer("mini198Q".to_owned()))
        );
        assert_eq!(
            parse_visible_chat("[MVP+] Player_1 has joined (2/8)!"),
            Some(VisibleChatSignal::QueueProgress {
                username: "Player_1".to_owned(),
                current: 2,
                total: 8,
            })
        );
        assert_eq!(parse_visible_chat("Sending you to miniABC!"), None);
    }

    #[test]
    fn recognizes_the_three_explicit_game_start_markers() {
        assert_eq!(
            parse_visible_chat("Opponent: Player"),
            Some(VisibleChatSignal::GameStarted(GameKind::Duels))
        );
        assert_eq!(
            parse_visible_chat("Cages opened! FIGHT!"),
            Some(VisibleChatSignal::GameStarted(GameKind::Skywars))
        );
        assert_eq!(
            parse_visible_chat("Protect your bed and destroy the enemy beds."),
            Some(VisibleChatSignal::GameStarted(GameKind::Bedwars))
        );
    }

    #[test]
    fn recognizes_retry_and_game_end_signals() {
        assert_eq!(
            parse_visible_chat("You were spawned in Limbo."),
            Some(VisibleChatSignal::LimboSpawned)
        );
        assert_eq!(
            parse_visible_chat("Please don't spam the command!"),
            Some(VisibleChatSignal::CommandRejected(
                "Please don't spam the command!".to_owned()
            ))
        );
        assert_eq!(
            parse_visible_chat("Reward Summary"),
            Some(VisibleChatSignal::DuelEnded)
        );
        assert_eq!(
            parse_visible_chat("VICTORY!"),
            Some(VisibleChatSignal::BedwarsOrSkywarsEnded)
        );
    }
}
