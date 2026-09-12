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

//! 定義 Nick 篩選規則、輸入正規化及優先規則；不依賴連線或狀態機。

use serde::{Deserialize, Serialize};

const PRIORITY_ACCEPT_SEQUENCES: [&str; 5] = ["aaa", "eee", "iii", "ooo", "uuu"];

#[derive(Debug, Clone, Copy, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum NickContainsMatchMode {
    #[default]
    Any,
    All,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default)]
/// 名稱篩選條件；優先規則刻意先於一般長度與字元限制。
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
    /// 修整規則字串、限制清單數量及長度範圍。
    /// @return 可供狀態機使用的規則。
    pub(super) fn normalized(mut self) -> Self {
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

/// 名稱是否通過，以及供 UI 或日誌顯示的判斷原因。
pub(super) struct NickDecision {
    pub(super) accepted: bool,
    pub(super) reasons: Vec<String>,
}

/// 先檢查優先規則，再依長度、字元與文字條件判斷候選。
/// @param nick 待評估名稱。
/// @param rules 目前設定的篩選規則。
/// @return 接受結果及具體原因；保留優先規則短路語意。
pub(super) fn evaluate_nick(nick: &str, rules: &NickRules) -> NickDecision {
    let normalized = if rules.case_sensitive {
        nick.to_owned()
    } else {
        nick.to_ascii_lowercase()
    };

    // 優先規則刻意先於長度與字元限制，符合即接受；調整順序會改變篩選語意。
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
    let length = nick.chars().count();
    if let Some(limit) = rules
        .exact_length
        .filter(|limit| length != usize::from(*limit))
    {
        reasons.push(format!("length {length} is not exactly {limit}"));
    }
    if let Some(limit) = rules
        .min_length
        .filter(|limit| length < usize::from(*limit))
    {
        reasons.push(format!("length {length} is shorter than {limit}"));
    }
    if let Some(limit) = rules
        .max_length
        .filter(|limit| length > usize::from(*limit))
    {
        reasons.push(format!("length {length} is longer than {limit}"));
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
        let mut contains = rules
            .contains
            .iter()
            .map(|value| value.trim())
            .filter(|value| !value.is_empty());
        let matches = match rules.contains_match_mode {
            NickContainsMatchMode::Any => {
                contains.any(|value| contains_value(&normalized, value, rules.case_sensitive))
            }
            NickContainsMatchMode::All => {
                contains.all(|value| contains_value(&normalized, value, rules.case_sensitive))
            }
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
}
