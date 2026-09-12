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

//! 比對加入訊息中的真實名稱，名稱缺失時不視為配對成功。

/// 比對預期真實名稱與加入日誌中的名稱。
/// @param expected 計畫保存的名稱；缺失時不接受。
/// @param observed 日誌中的玩家名稱。
/// @return 名稱不分大小寫相符時為 true。
pub(crate) fn matches(expected: Option<&str>, observed: &str) -> bool {
    expected.is_some_and(|name| name.eq_ignore_ascii_case(observed))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_names_without_case_sensitivity() {
        assert!(matches(Some("PlayerOne"), "playerone"));
        assert!(!matches(Some("PlayerOne"), "other"));
        assert!(!matches(None, "playerone"));
    }
}
