use std::time::{SystemTime, UNIX_EPOCH};

pub(crate) fn parse(line: &str) -> Option<(String, String)> {
    let (speaker, message) = line.split_once(':')?;
    let username = speaker
        .split_whitespace()
        .last()?
        .trim_matches(|character: char| !character.is_ascii_alphanumeric() && character != '_');
    if username.is_empty()
        || !username
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || character == '_')
        || message.trim().is_empty()
    {
        return None;
    }
    Some((username.to_owned(), message.trim().to_owned()))
}

pub(crate) fn matches(expected_username: &str, username: &str) -> bool {
    expected_username.eq_ignore_ascii_case(username)
}

pub(crate) fn random_phrase() -> String {
    const PHRASES: [&str; 100] = [
        "hi",
        "hello",
        "hey",
        "yo",
        "hiya",
        "sup",
        "heya",
        "greetings",
        "howdy",
        "hola",
        "hi there",
        "hello there",
        "hey there",
        "yo there",
        "good day",
        "morning",
        "evening",
        "nice game",
        "good luck",
        "have fun",
        "lets go",
        "ready",
        "all good",
        "im here",
        "here",
        "checking in",
        "quick hello",
        "just saying hi",
        "hello team",
        "hey team",
        "hi team",
        "hello friend",
        "hey friend",
        "hi friend",
        "good to see you",
        "nice to meet you",
        "welcome",
        "welcome here",
        "hello again",
        "hey again",
        "hi again",
        "test hello",
        "test hi",
        "chat check",
        "presence check",
        "queue check",
        "server check",
        "same room",
        "same game",
        "right here",
        "over here",
        "on my way",
        "we are ready",
        "we got this",
        "lets play",
        "lets win",
        "good start",
        "nice start",
        "all set",
        "set to go",
        "ready now",
        "hello everyone",
        "hey everyone",
        "hi everyone",
        "yo everyone",
        "hello all",
        "hey all",
        "hi all",
        "good evening",
        "good morning",
        "good afternoon",
        "what is up",
        "whats up",
        "how is it going",
        "hope youre well",
        "checking chat",
        "checking in now",
        "quick check",
        "one two",
        "two two",
        "copy that",
        "message sent",
        "chat ready",
        "hello lobby",
        "hey lobby",
        "hi lobby",
        "hello server",
        "hey server",
        "hi server",
        "friendly hello",
        "friendly hi",
        "small hello",
        "short hello",
        "quick hi",
        "quick hey",
        "say hi",
        "say hello",
        "ping",
        "pong",
        "lets begin",
    ];
    let tick = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    PHRASES[(tick % PHRASES.len() as u128) as usize].to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn confirms_the_real_username_without_requiring_the_sent_phrase() {
        assert!(matches("BotPlayer", "botplayer"));
        assert!(!matches("BotPlayer", "AnotherPlayer"));
    }
}
