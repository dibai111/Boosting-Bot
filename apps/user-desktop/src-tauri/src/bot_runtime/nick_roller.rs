use serde::{Deserialize, Serialize};
use std::time::{Duration, Instant};

const ROLL_BOOK_COMMAND: &str = "/nick help setrandom";
const ACCEPT_NICK_COMMAND: &str = "/nick actuallyset {nick} respawn";
const RANDOM_NICK_BOOK_MARKER: &str = "We've generated a random username for you:";
const NICK_CONFIRMATION_BOOK_MARKER: &str = "You have finished setting up your nickname!";
const NICK_CONFIRMATION_NAME_MARKER: &str = "When you go into a game, you will be nicked as";
const LOCRAW_DELAY: Duration = Duration::from_secs(2);
const MANUAL_DECISION_TIMEOUT_MS: u64 = 100_000;
const NICK_CONFIRMATION_TIMEOUT: Duration = Duration::from_secs(10);
const MAX_IGNORED_BOOK_RESULTS: u8 = 5;
const MIN_BOOK_TIMEOUT_MS: u64 = 500;
const MAX_BOOK_TIMEOUT_MS: u64 = 30_000;
const MAX_NEXT_ROLL_DELAY_MS: u64 = 60_000;
const PRIORITY_ACCEPT_SEQUENCES: [&str; 5] = ["aaa", "eee", "iii", "ooo", "uuu"];
const IGNORED_CANDIDATES: [&str; 35] = [
    "a",
    "an",
    "and",
    "are",
    "book",
    "cancel",
    "click",
    "close",
    "confirm",
    "for",
    "help",
    "here",
    "hypixel",
    "is",
    "nick",
    "nickname",
    "random",
    "respawn",
    "set",
    "setrandom",
    "skin",
    "the",
    "this",
    "to",
    "your",
    "we",
    "have",
    "finished",
    "setting",
    "up",
    "when",
    "you",
    "go",
    "mvp",
    "vip",
];

#[derive(Debug, Clone, Copy, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum NickContainsMatchMode {
    #[default]
    Any,
    All,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default)]
pub(crate) struct NickRules {
    pub(crate) case_sensitive: bool,
    pub(crate) exact_length: Option<u8>,
    pub(crate) min_length: Option<u8>,
    pub(crate) max_length: Option<u8>,
    pub(crate) allow_numbers: bool,
    pub(crate) allow_underscore: bool,
    pub(crate) allow_list: Vec<String>,
    pub(crate) starts_with: Vec<String>,
    pub(crate) ends_with: Vec<String>,
    pub(crate) contains: Vec<String>,
    pub(crate) contains_match_mode: NickContainsMatchMode,
    pub(crate) starts_with_priority: bool,
    pub(crate) ends_with_priority: bool,
    pub(crate) contains_priority: bool,
    pub(crate) legacy_priority: bool,
}

impl Default for NickRules {
    fn default() -> Self {
        Self {
            case_sensitive: false,
            exact_length: None,
            min_length: Some(1),
            max_length: Some(8),
            allow_numbers: true,
            allow_underscore: true,
            allow_list: Vec::new(),
            starts_with: Vec::new(),
            ends_with: Vec::new(),
            contains: Vec::new(),
            contains_match_mode: NickContainsMatchMode::Any,
            starts_with_priority: false,
            ends_with_priority: false,
            contains_priority: false,
            legacy_priority: true,
        }
    }
}

impl NickRules {
    fn normalized(mut self) -> Self {
        self.exact_length = normalize_length(self.exact_length);
        self.min_length = normalize_length(self.min_length);
        self.max_length = normalize_length(self.max_length);
        self.allow_list = normalize_list(self.allow_list);
        self.starts_with = normalize_list(self.starts_with);
        self.ends_with = normalize_list(self.ends_with);
        self.contains = normalize_list(self.contains);
        self
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default)]
pub(crate) struct NickRollerConfig {
    pub(crate) book_timeout_ms: u64,
    pub(crate) next_roll_delay_ms: u64,
    pub(crate) stop_after_found: bool,
    pub(crate) rules: NickRules,
}

impl Default for NickRollerConfig {
    fn default() -> Self {
        Self {
            book_timeout_ms: 2_000,
            next_roll_delay_ms: 1_000,
            stop_after_found: false,
            rules: NickRules::default(),
        }
    }
}

impl NickRollerConfig {
    pub(crate) fn normalized(mut self) -> Self {
        self.book_timeout_ms = self
            .book_timeout_ms
            .clamp(MIN_BOOK_TIMEOUT_MS, MAX_BOOK_TIMEOUT_MS);
        self.next_roll_delay_ms = self.next_roll_delay_ms.min(MAX_NEXT_ROLL_DELAY_MS);
        self.rules = self.rules.normalized();
        self
    }
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum NickRollerPhase {
    Idle,
    Preparing,
    Rolling,
    AwaitingDecision,
    Verifying,
    Finished,
    Failed,
    Stopped,
}

impl NickRollerPhase {
    pub(crate) const fn is_terminal(self) -> bool {
        matches!(self, Self::Finished | Self::Failed | Self::Stopped)
    }

    fn is_active(self) -> bool {
        matches!(
            self,
            Self::Preparing | Self::Rolling | Self::AwaitingDecision | Self::Verifying
        )
    }
}

#[derive(Debug, Clone)]
pub(super) struct NickBook {
    pub(super) pages: Vec<String>,
}

#[derive(Debug)]
pub(super) enum NickInput {
    Start {
        config: NickRollerConfig,
        now: Instant,
    },
    Spawn {
        now: Instant,
    },
    Chat {
        message: String,
        now: Instant,
    },
    Book {
        book: NickBook,
        now: Instant,
    },
    Tick {
        now: Instant,
    },
    Decision {
        candidate_id: u64,
        take: bool,
        now: Instant,
    },
    Stop,
}

pub(super) enum NickAction {
    SendChat(String),
    State {
        phase: NickRollerPhase,
        message: Option<String>,
    },
    Candidate {
        candidate_id: u64,
        nick: String,
        accepted: bool,
        reasons: Vec<String>,
        processed_count: u32,
        accepted_count: u32,
        rejected_count: u32,
        decision_timeout_ms: Option<u64>,
    },
    Verification {
        candidate_id: u64,
        expected_nick: String,
        actual_nick: Option<String>,
        success: bool,
        reason: String,
        processed_count: u32,
    },
    Attention {
        code: String,
        message: String,
    },
}

struct Candidate {
    id: u64,
    nick: String,
}

struct PendingConfirmation {
    candidate_id: u64,
    expected_nick: String,
    deadline: Instant,
}

struct BookResult {
    random_nick: Option<String>,
    confirmation: Option<Option<String>>,
}

pub(super) struct NickRoller {
    config: NickRollerConfig,
    phase: NickRollerPhase,
    next_locraw_at: Option<Instant>,
    next_roll_at: Option<Instant>,
    pending_book_deadline: Option<Instant>,
    pending_confirmation: Option<PendingConfirmation>,
    candidate: Option<Candidate>,
    next_candidate_id: u64,
    processed_count: u32,
    accepted_count: u32,
    rejected_count: u32,
    ignored_book_results: u8,
}

impl NickRoller {
    pub(super) fn new() -> Self {
        Self {
            config: NickRollerConfig::default(),
            phase: NickRollerPhase::Idle,
            next_locraw_at: None,
            next_roll_at: None,
            pending_book_deadline: None,
            pending_confirmation: None,
            candidate: None,
            next_candidate_id: 0,
            processed_count: 0,
            accepted_count: 0,
            rejected_count: 0,
            ignored_book_results: 0,
        }
    }

    #[cfg(test)]
    pub(super) fn phase(&self) -> NickRollerPhase {
        self.phase
    }

    pub(super) fn is_active(&self) -> bool {
        self.phase.is_active()
    }

    pub(super) fn handle(&mut self, input: NickInput) -> Vec<NickAction> {
        match input {
            NickInput::Start { config, now } => self.start(config, now),
            NickInput::Spawn { now } => self.handle_spawn(now),
            NickInput::Chat { message, now } => self.handle_chat(&message, now),
            NickInput::Book { book, now } => self.handle_book(book, now),
            NickInput::Tick { now } => self.tick(now),
            NickInput::Decision {
                candidate_id,
                take,
                now,
            } => self.decide(candidate_id, take, now),
            NickInput::Stop => self.stop(),
        }
    }

    fn start(&mut self, config: NickRollerConfig, now: Instant) -> Vec<NickAction> {
        self.config = config.normalized();
        self.phase = NickRollerPhase::Preparing;
        self.next_locraw_at = Some(now + LOCRAW_DELAY);
        self.next_roll_at = None;
        self.pending_book_deadline = None;
        self.pending_confirmation = None;
        self.candidate = None;
        self.next_candidate_id = 0;
        self.processed_count = 0;
        self.accepted_count = 0;
        self.rejected_count = 0;
        self.ignored_book_results = 0;
        vec![
            self.state(Some("Preparing Hypixel lobby.")),
            NickAction::SendChat("/lobby".to_owned()),
        ]
    }

    fn handle_spawn(&mut self, now: Instant) -> Vec<NickAction> {
        if !self.is_active() {
            return Vec::new();
        }
        if self.pending_confirmation.is_some() {
            return Vec::new();
        }
        self.phase = NickRollerPhase::Preparing;
        self.next_locraw_at = Some(now + LOCRAW_DELAY);
        self.next_roll_at = None;
        vec![self.state(Some("Checking Hypixel location."))]
    }

    fn handle_chat(&mut self, message: &str, now: Instant) -> Vec<NickAction> {
        if !self.is_active() {
            return Vec::new();
        }
        if self.pending_confirmation.is_some() {
            return Vec::new();
        }
        let Some(location) = extract_locraw(message) else {
            return Vec::new();
        };

        self.next_locraw_at = None;
        if location.lobbyname.is_some() {
            self.phase = NickRollerPhase::Rolling;
            self.next_roll_at = Some(now);
            return vec![self.state(Some("Hypixel lobby ready."))];
        }

        self.phase = NickRollerPhase::Preparing;
        self.next_roll_at = None;
        self.next_locraw_at = Some(now + LOCRAW_DELAY);
        vec![
            self.state(Some("Not in a Hypixel lobby; returning to lobby.")),
            NickAction::SendChat("/lobby".to_owned()),
        ]
    }

    fn handle_book(&mut self, book: NickBook, now: Instant) -> Vec<NickAction> {
        if !self.is_active() {
            return Vec::new();
        }
        let result = read_book(&book);

        if let Some(pending) = &self.pending_confirmation {
            let Some(confirmation) = result.confirmation else {
                return Vec::new();
            };
            return self.finish_confirmation(pending.candidate_id, confirmation, now);
        }

        if self.pending_book_deadline.is_none() || self.phase != NickRollerPhase::Rolling {
            return Vec::new();
        }
        self.pending_book_deadline = None;

        let Some(nick) = result.random_nick else {
            return self.handle_invalid_book(now);
        };

        self.ignored_book_results = 0;
        self.process_nick(nick, now)
    }

    fn tick(&mut self, now: Instant) -> Vec<NickAction> {
        if !self.is_active() {
            return Vec::new();
        }

        if let Some(pending) = &self.pending_confirmation {
            if now >= pending.deadline {
                return self.finish_confirmation(pending.candidate_id, None, now);
            }
        }

        if let Some(candidate) = &self.candidate {
            if self.phase == NickRollerPhase::AwaitingDecision
                && self.pending_confirmation.is_none()
                && self.next_roll_at.is_some_and(|deadline| now >= deadline)
            {
                return self.decide(candidate.id, false, now);
            }
        }

        if self
            .pending_book_deadline
            .is_some_and(|deadline| now >= deadline)
        {
            self.pending_book_deadline = None;
            self.schedule_next_roll(now);
            return vec![self.state(Some("Random nickname book timed out."))];
        }

        if self.next_locraw_at.is_some_and(|deadline| now >= deadline) {
            self.next_locraw_at = None;
            return vec![NickAction::SendChat("/locraw".to_owned())];
        }

        if self.phase == NickRollerPhase::Rolling
            && self.pending_confirmation.is_none()
            && self.pending_book_deadline.is_none()
            && self.next_roll_at.is_some_and(|deadline| now >= deadline)
        {
            return self.request_roll(now);
        }

        Vec::new()
    }

    fn decide(&mut self, candidate_id: u64, take: bool, now: Instant) -> Vec<NickAction> {
        let Some(candidate) = self.candidate.take() else {
            return Vec::new();
        };
        if candidate.id != candidate_id || self.phase != NickRollerPhase::AwaitingDecision {
            self.candidate = Some(candidate);
            return Vec::new();
        }

        if !take {
            self.schedule_next_roll(now);
            return vec![self.state(Some("Candidate skipped."))];
        }

        self.begin_confirmation(candidate, now)
    }

    fn stop(&mut self) -> Vec<NickAction> {
        if !self.is_active() {
            return Vec::new();
        }
        self.phase = NickRollerPhase::Stopped;
        self.next_locraw_at = None;
        self.next_roll_at = None;
        self.pending_book_deadline = None;
        self.pending_confirmation = None;
        self.candidate = None;
        vec![self.state(Some("Nick Roller stopped."))]
    }

    fn process_nick(&mut self, nick: String, now: Instant) -> Vec<NickAction> {
        self.processed_count = self.processed_count.saturating_add(1);
        let decision = evaluate_nick(&nick, &self.config.rules);
        let candidate_id = self.next_candidate_id.saturating_add(1);
        self.next_candidate_id = candidate_id;

        if !decision.accepted {
            self.rejected_count = self.rejected_count.saturating_add(1);
            self.schedule_next_roll(now);
            return vec![NickAction::Candidate {
                candidate_id,
                nick,
                accepted: false,
                reasons: decision.reasons,
                processed_count: self.processed_count,
                accepted_count: self.accepted_count,
                rejected_count: self.rejected_count,
                decision_timeout_ms: None,
            }];
        }

        self.accepted_count = self.accepted_count.saturating_add(1);
        let candidate = Candidate {
            id: candidate_id,
            nick: nick.clone(),
        };
        self.candidate = Some(candidate);
        self.phase = NickRollerPhase::AwaitingDecision;
        self.next_roll_at = Some(now + Duration::from_millis(MANUAL_DECISION_TIMEOUT_MS));

        let mut actions = vec![NickAction::Candidate {
            candidate_id,
            nick,
            accepted: true,
            reasons: decision.reasons,
            processed_count: self.processed_count,
            accepted_count: self.accepted_count,
            rejected_count: self.rejected_count,
            decision_timeout_ms: Some(MANUAL_DECISION_TIMEOUT_MS),
        }];
        if self.config.stop_after_found {
            actions.extend(self.decide(candidate_id, true, now));
        } else {
            actions.push(self.state(Some("Waiting for nickname decision.")));
        }
        actions
    }

    fn begin_confirmation(&mut self, candidate: Candidate, now: Instant) -> Vec<NickAction> {
        let expected_nick = candidate.nick.clone();
        self.phase = NickRollerPhase::Verifying;
        self.next_roll_at = None;
        self.pending_confirmation = Some(PendingConfirmation {
            candidate_id: candidate.id,
            expected_nick: expected_nick.clone(),
            deadline: now + NICK_CONFIRMATION_TIMEOUT,
        });
        vec![
            self.state(Some("Applying nickname and waiting for confirmation.")),
            NickAction::SendChat(ACCEPT_NICK_COMMAND.replace("{nick}", &expected_nick)),
        ]
    }

    fn finish_confirmation(
        &mut self,
        candidate_id: u64,
        actual_nick: Option<String>,
        _now: Instant,
    ) -> Vec<NickAction> {
        let Some(pending) = self.pending_confirmation.take() else {
            return Vec::new();
        };
        if pending.candidate_id != candidate_id {
            self.pending_confirmation = Some(pending);
            return Vec::new();
        }

        let success = actual_nick
            .as_deref()
            .is_some_and(|actual| actual.eq_ignore_ascii_case(&pending.expected_nick));
        let reason = if success {
            "matched"
        } else if actual_nick.is_some() {
            "mismatch"
        } else {
            "missing_or_unreadable"
        };
        self.phase = if success {
            NickRollerPhase::Finished
        } else {
            NickRollerPhase::Failed
        };
        self.candidate = None;
        vec![
            NickAction::Verification {
                candidate_id,
                expected_nick: pending.expected_nick,
                actual_nick,
                success,
                reason: reason.to_owned(),
                processed_count: self.processed_count,
            },
            self.state(if success {
                Some("Nickname verified successfully.")
            } else {
                Some("Nickname verification failed.")
            }),
        ]
    }

    fn handle_invalid_book(&mut self, now: Instant) -> Vec<NickAction> {
        self.ignored_book_results = self.ignored_book_results.saturating_add(1);
        if self.ignored_book_results > MAX_IGNORED_BOOK_RESULTS {
            self.phase = NickRollerPhase::Failed;
            return vec![
                NickAction::Attention {
                    code: "invalid_nick_books".to_owned(),
                    message: "Nickname results could not be read.".to_owned(),
                },
                self.state(Some("Nickname results could not be read.")),
            ];
        }

        self.schedule_next_roll(now);
        vec![self.state(Some("Ignored a non-random nickname book."))]
    }

    fn request_roll(&mut self, now: Instant) -> Vec<NickAction> {
        self.next_roll_at = None;
        self.pending_book_deadline = Some(now + Duration::from_millis(self.config.book_timeout_ms));
        vec![NickAction::SendChat(ROLL_BOOK_COMMAND.to_owned())]
    }

    fn schedule_next_roll(&mut self, now: Instant) {
        self.phase = NickRollerPhase::Rolling;
        self.candidate = None;
        self.next_roll_at = Some(now + Duration::from_millis(self.config.next_roll_delay_ms));
    }

    fn state(&self, message: Option<&str>) -> NickAction {
        NickAction::State {
            phase: self.phase,
            message: message.map(str::to_owned),
        }
    }
}

struct Locraw {
    lobbyname: Option<String>,
}

fn extract_locraw(message: &str) -> Option<Locraw> {
    let start = message.find('{')?;
    let end = message.rfind('}')?;
    if end <= start {
        return None;
    }
    let value = serde_json::from_str::<serde_json::Value>(&message[start..=end]).ok()?;
    Some(Locraw {
        lobbyname: value
            .get("lobbyname")
            .and_then(serde_json::Value::as_str)
            .map(str::to_owned),
    })
}

fn read_book(book: &NickBook) -> BookResult {
    let text = book
        .pages
        .iter()
        .map(|page| clean_text(page))
        .filter(|page| !page.is_empty())
        .collect::<Vec<_>>()
        .join(" ");

    let confirmation = text
        .find(NICK_CONFIRMATION_BOOK_MARKER)
        .map(|_| extract_after_marker(&text, NICK_CONFIRMATION_NAME_MARKER));
    let random_nick = text
        .find(RANDOM_NICK_BOOK_MARKER)
        .and_then(|index| extract_after_index(&text, index + RANDOM_NICK_BOOK_MARKER.len()));

    BookResult {
        random_nick,
        confirmation,
    }
}

fn extract_after_marker(text: &str, marker: &str) -> Option<String> {
    let index = text.find(marker)?;
    extract_after_index(text, index + marker.len())
}

fn extract_after_index(text: &str, index: usize) -> Option<String> {
    text[index..]
        .split_whitespace()
        .map(|token| {
            token.trim_matches(|character: char| {
                !character.is_ascii_alphanumeric() && character != '_'
            })
        })
        .find(|token| is_likely_nick(token))
        .map(str::to_owned)
}

fn clean_text(value: &str) -> String {
    let mut result = String::with_capacity(value.len());
    let mut skip_format_code = false;
    for character in value.chars() {
        if skip_format_code {
            skip_format_code = false;
            continue;
        }
        if character == '§' {
            skip_format_code = true;
            continue;
        }
        result.push(character);
    }
    result.split_whitespace().collect::<Vec<_>>().join(" ")
}

struct NickDecision {
    accepted: bool,
    reasons: Vec<String>,
}

fn evaluate_nick(nick: &str, rules: &NickRules) -> NickDecision {
    let normalized = if rules.case_sensitive {
        nick.to_owned()
    } else {
        nick.to_ascii_lowercase()
    };

    if rules.legacy_priority {
        if let Some(sequence) = find_legacy_priority_sequence(nick) {
            return accepted(format!("matched priority sequence {sequence}"));
        }
    }
    if !rules.allow_list.is_empty()
        && matches_literal(&normalized, &rules.allow_list, rules, MatchKind::Equals).is_some()
    {
        return accepted("matched allowList".to_owned());
    }
    if let Some(reason) = priority_rule_reason(&normalized, rules) {
        return accepted(reason);
    }

    let mut reasons = Vec::new();
    if rules
        .exact_length
        .is_some_and(|limit| nick.chars().count() != usize::from(limit))
    {
        if let Some(limit) = rules.exact_length {
            reasons.push(format!(
                "length {} is not exactly {limit}",
                nick.chars().count()
            ));
        }
    }
    if rules
        .min_length
        .is_some_and(|limit| nick.chars().count() < usize::from(limit))
    {
        if let Some(limit) = rules.min_length {
            reasons.push(format!(
                "length {} is shorter than {limit}",
                nick.chars().count()
            ));
        }
    }
    if rules
        .max_length
        .is_some_and(|limit| nick.chars().count() > usize::from(limit))
    {
        if let Some(limit) = rules.max_length {
            reasons.push(format!(
                "length {} is longer than {limit}",
                nick.chars().count()
            ));
        }
    }
    if !reasons.is_empty() {
        return rejected(reasons);
    }

    if !rules.allow_numbers && nick.chars().any(|character| character.is_ascii_digit()) {
        reasons.push("contains numbers but allow_numbers is false".to_owned());
    }
    if !rules.allow_underscore && nick.contains('_') {
        reasons.push("contains underscore but allow_underscore is false".to_owned());
    }
    if nick
        .chars()
        .any(|character| !character.is_ascii_alphanumeric() && character != '_')
    {
        reasons.push("contains non-Minecraft username characters".to_owned());
    }
    if !reasons.is_empty() {
        return rejected(reasons);
    }

    let starts_with = configured(&rules.starts_with)
        && matches_literal(
            &normalized,
            &rules.starts_with,
            rules,
            MatchKind::StartsWith,
        )
        .is_none();
    if starts_with {
        reasons.push("does not match Starts With".to_owned());
    }
    let ends_with = configured(&rules.ends_with)
        && matches_literal(&normalized, &rules.ends_with, rules, MatchKind::EndsWith).is_none();
    if ends_with {
        reasons.push("does not match Ends With".to_owned());
    }
    if configured(&rules.contains) {
        let contains = rules
            .contains
            .iter()
            .map(|value| value.trim())
            .filter(|value| !value.is_empty())
            .collect::<Vec<_>>();
        let matches = match rules.contains_match_mode {
            NickContainsMatchMode::Any => contains
                .iter()
                .any(|value| contains_value(&normalized, value, rules.case_sensitive)),
            NickContainsMatchMode::All => contains
                .iter()
                .all(|value| contains_value(&normalized, value, rules.case_sensitive)),
        };
        if !matches {
            reasons.push(match rules.contains_match_mode {
                NickContainsMatchMode::Any => "does not contain any Contains keyword".to_owned(),
                NickContainsMatchMode::All => "does not contain every Contains keyword".to_owned(),
            });
        }
    }

    if reasons.is_empty() {
        accepted("all enabled rules passed".to_owned())
    } else {
        rejected(reasons)
    }
}

fn priority_rule_reason(nick: &str, rules: &NickRules) -> Option<String> {
    if rules.starts_with_priority {
        if let Some(value) = matches_literal(nick, &rules.starts_with, rules, MatchKind::StartsWith)
        {
            return Some(format!("matched priority Starts With {value}"));
        }
    }
    if rules.ends_with_priority {
        if let Some(value) = matches_literal(nick, &rules.ends_with, rules, MatchKind::EndsWith) {
            return Some(format!("matched priority Ends With {value}"));
        }
    }
    if rules.contains_priority {
        let contains = rules
            .contains
            .iter()
            .map(|value| value.trim())
            .filter(|value| !value.is_empty())
            .collect::<Vec<_>>();
        match rules.contains_match_mode {
            NickContainsMatchMode::Any => contains
                .iter()
                .find(|value| contains_value(nick, value, rules.case_sensitive))
                .map(|value| format!("matched priority Contains {value}")),
            NickContainsMatchMode::All if !contains.is_empty() => contains
                .iter()
                .all(|value| contains_value(nick, value, rules.case_sensitive))
                .then(|| format!("matched priority Contains {}", contains.join(", "))),
            NickContainsMatchMode::All => None,
        }
    } else {
        None
    }
}

#[derive(Clone, Copy)]
enum MatchKind {
    Equals,
    StartsWith,
    EndsWith,
}

fn matches_literal(
    nick: &str,
    values: &[String],
    rules: &NickRules,
    kind: MatchKind,
) -> Option<String> {
    values.iter().find_map(|value| {
        let trimmed = value.trim();
        if trimmed.is_empty() {
            return None;
        }
        let comparison = if rules.case_sensitive {
            trimmed.to_owned()
        } else {
            trimmed.to_ascii_lowercase()
        };
        let matches = match kind {
            MatchKind::Equals => nick == comparison,
            MatchKind::StartsWith => nick.starts_with(&comparison),
            MatchKind::EndsWith => nick.ends_with(&comparison),
        };
        matches.then(|| trimmed.to_owned())
    })
}

fn contains_value(nick: &str, value: &str, case_sensitive: bool) -> bool {
    if case_sensitive {
        nick.contains(value)
    } else {
        nick.contains(&value.to_ascii_lowercase())
    }
}

fn find_legacy_priority_sequence(nick: &str) -> Option<&'static str> {
    let mut characters = nick.chars();
    let first = characters.next()?;
    if !first.is_ascii_uppercase() || characters.any(|character| character.is_ascii_uppercase()) {
        return None;
    }
    PRIORITY_ACCEPT_SEQUENCES
        .iter()
        .copied()
        .find(|sequence| nick.contains(sequence))
}

fn is_likely_nick(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 16
        && value
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || character == '_')
        && !IGNORED_CANDIDATES.contains(&value.to_ascii_lowercase().as_str())
}

fn configured(values: &[String]) -> bool {
    values.iter().any(|value| !value.trim().is_empty())
}

fn normalize_length(value: Option<u8>) -> Option<u8> {
    value.map(|length| length.clamp(1, 16))
}

fn normalize_list(values: Vec<String>) -> Vec<String> {
    values
        .into_iter()
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
        .take(256)
        .collect()
}

fn accepted(reason: String) -> NickDecision {
    NickDecision {
        accepted: true,
        reasons: vec![reason],
    }
}

fn rejected(reasons: Vec<String>) -> NickDecision {
    NickDecision {
        accepted: false,
        reasons,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config(rules: NickRules) -> NickRollerConfig {
        NickRollerConfig {
            rules,
            ..Default::default()
        }
    }

    #[test]
    fn accepts_legacy_priority_sequence_before_other_rules() {
        let rules = NickRules {
            max_length: Some(1),
            ..Default::default()
        };
        let result = evaluate_nick("Aaaa", &rules);

        assert!(result.accepted);
        assert_eq!(result.reasons, ["matched priority sequence aaa"]);
    }

    #[test]
    fn applies_all_configured_non_priority_rules() {
        let rules = NickRules {
            legacy_priority: false,
            max_length: Some(8),
            min_length: Some(3),
            allow_numbers: false,
            allow_underscore: false,
            starts_with: vec!["bot".to_owned()],
            ends_with: vec!["_x".to_owned()],
            contains: vec!["safe".to_owned()],
            contains_match_mode: NickContainsMatchMode::All,
            ..Default::default()
        };
        let result = evaluate_nick("bad", &rules);

        assert!(!result.accepted);
        assert!(result
            .reasons
            .iter()
            .any(|reason| reason.contains("Starts With")));
        assert!(result
            .reasons
            .iter()
            .any(|reason| reason.contains("Ends With")));
        assert!(result
            .reasons
            .iter()
            .any(|reason| reason.contains("Contains")));
    }

    #[test]
    fn priority_rules_short_circuit_character_and_length_failures() {
        let rules = NickRules {
            legacy_priority: false,
            starts_with_priority: true,
            starts_with: vec!["bad".to_owned()],
            max_length: Some(1),
            allow_numbers: false,
            ..Default::default()
        };
        let result = evaluate_nick("bad123", &rules);

        assert!(result.accepted);
        assert_eq!(result.reasons, ["matched priority Starts With bad"]);
    }

    #[test]
    fn extracts_random_and_confirmation_nick_from_book_text() {
        let book = NickBook {
            pages: vec![
                "We've generated a random username for you: AquaVortex".to_owned(),
                "You have finished setting up your nickname! When you go into a game, you will be nicked as [MVP+] AquaVortex".to_owned(),
            ],
        };
        let result = read_book(&book);

        assert_eq!(result.random_nick.as_deref(), Some("AquaVortex"));
        assert_eq!(result.confirmation, Some(Some("AquaVortex".to_owned())));
    }

    #[test]
    fn ignores_regular_words_when_extracting_candidate() {
        let book = NickBook {
            pages: vec!["We've generated a random username for you: the NewPlayer".to_owned()],
        };
        assert_eq!(read_book(&book).random_nick.as_deref(), Some("NewPlayer"));
    }

    #[test]
    fn rolls_then_waits_for_manual_decision_and_confirmation() {
        let start = Instant::now();
        let mut roller = NickRoller::new();
        let mut actions = roller.handle(NickInput::Start {
            config: config(NickRules {
                legacy_priority: false,
                ..Default::default()
            }),
            now: start,
        });
        assert!(actions
            .iter()
            .any(|action| matches!(action, NickAction::SendChat(command) if command == "/lobby")));

        roller.handle(NickInput::Chat {
            message: "{\"server\":\"mini1\",\"lobbyname\":\"lobby1\"}".to_owned(),
            now: start,
        });
        actions = roller.handle(NickInput::Tick {
            now: start + Duration::from_secs(1),
        });
        assert!(actions.iter().any(
            |action| matches!(action, NickAction::SendChat(command) if command == ROLL_BOOK_COMMAND)
        ));

        actions = roller.handle(NickInput::Book {
            book: NickBook {
                pages: vec!["We've generated a random username for you: AquaVtx".to_owned()],
            },
            now: start + Duration::from_secs(1),
        });
        let candidate_id = actions
            .iter()
            .find_map(|action| match action {
                NickAction::Candidate {
                    candidate_id,
                    accepted: true,
                    ..
                } => Some(*candidate_id),
                _ => None,
            })
            .expect("candidate event");

        actions = roller.handle(NickInput::Decision {
            candidate_id,
            take: true,
            now: start + Duration::from_secs(2),
        });
        assert!(actions.iter().any(
            |action| matches!(action, NickAction::SendChat(command) if command.contains("AquaVtx"))
        ));

        actions = roller.handle(NickInput::Book {
            book: NickBook {
                pages: vec!["You have finished setting up your nickname! When you go into a game, you will be nicked as AquaVtx".to_owned()],
            },
            now: start + Duration::from_secs(3),
        });
        assert!(actions
            .iter()
            .any(|action| matches!(action, NickAction::Verification { success: true, .. })));
        assert_eq!(roller.phase(), NickRollerPhase::Finished);
    }
}
