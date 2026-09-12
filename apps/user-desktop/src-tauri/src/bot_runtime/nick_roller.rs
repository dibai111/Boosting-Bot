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

//! 以輸入事件推進 Nick 狀態機，輸出待執行動作；規則評估由 nick_rules 負責。

use super::nick_rules::{evaluate_nick, NickRules};
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

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default)]
/// 後端 Nick 篩選設定；不包含只影響 UI 的音效或顯示偏好。
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
    /// 將等待時間及篩選規則限制在後端支援範圍。
    /// @return 正規化後的設定。
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
/// Nick 狀態機階段，序列化值供前端顯示及操作判斷。
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
    /// 判斷篩選是否已成功、失敗或被使用者停止。
    /// @return 終止階段為 true；Idle 不視為終止事件。
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
/// 從 Minecraft 書本物件擷取出的可見頁面文字。
pub(super) struct NickBook {
    pub(super) pages: Vec<String>,
}

#[derive(Debug)]
/// 包含業務事件及單調時間的狀態機輸入，不直接操作連線。
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

/// 狀態機輸出的有序副作用描述，由 SessionActor 執行。
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

/// Nick 篩選與候選確認狀態機；候選編號在同一連線內延續。
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
    /// 初始化空閒 Nick 狀態機及候選計數器。
    /// @return 尚未啟動的 NickRoller。
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

    /// 判斷是否仍在準備、篩選、決策或驗證。
    /// @return 正在執行篩選時為 true。
    pub(super) fn is_active(&self) -> bool {
        self.phase.is_active()
    }

    /// 將一筆輸入推進為新狀態及待執行動作。
    /// @param input 帶來源事件與時間的 NickInput。
    /// @return 有序動作清單；過期或無關輸入回傳空清單。
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
        // 同一連線重新啟動篩選仍延續編號，避免上一輪遲到的決策命中新候選。
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
            return self.finish_confirmation(pending.candidate_id, confirmation);
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
                return self.finish_confirmation(pending.candidate_id, None);
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

    // 候選編號必須相符，遲到的介面決定不能套用到下一個候選。
    /// 只處理目前候選的決策，舊編號不會消耗新候選。
    /// @param candidate_id 前端收到的候選編號。
    /// @param take true 表示套用，false 表示跳過。
    /// @param now 排程下一輪或確認期限的基準時間。
    /// @return 套用或跳過所需的動作。
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

    /// 按優先規則與一般條件評估新名稱，累計接受／拒絕數。
    /// @param nick 書本解析出的 Minecraft 名稱。
    /// @param now 候選決策期限的基準時間。
    /// @return 候選事件及可能的自動確認動作。
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

    /// 比對服務端回傳名稱並結束候選驗證。
    /// @param candidate_id 目前等待驗證的候選編號。
    /// @param actual_nick 服務端確認名稱；逾時或無法讀取時為 None。
    /// @return 驗證結果及終止狀態事件。
    fn finish_confirmation(
        &mut self,
        candidate_id: u64,
        actual_nick: Option<String>,
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

/// 解析聊天中內嵌的 locraw JSON。
/// @param message 可能含 JSON 位置回應的聊天文字。
/// @return 位置資料；無法解析時為 None。
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

// 書本標記保留伺服器原文，勿隨中文註解或介面翻譯更動。
/// 以服務端固定標記辨識隨機名稱及套用確認。
/// @param book 已去除 Minecraft 物件格式的書本頁面。
/// @return 解析出的候選名稱及確認結果。
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

fn is_likely_nick(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 16
        && value
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || character == '_')
        && !IGNORED_CANDIDATES.contains(&value.to_ascii_lowercase().as_str())
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
