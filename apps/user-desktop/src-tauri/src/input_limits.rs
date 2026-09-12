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

//! 集中限制外部字串輸入的位元組長度，避免不同指令使用不一致的上限。

use anyhow::{bail, Result};

pub(crate) const MAX_BOT_ID_BYTES: usize = 128;
pub(crate) const MAX_CREDENTIAL_BYTES: usize = 2 * 1024 * 1024;
pub(crate) const MAX_LOG_PATH_BYTES: usize = 4 * 1024;
pub(crate) const MAX_SERVER_ADDRESS_BYTES: usize = 255;
pub(crate) const MAX_USERNAME_BYTES: usize = 64;

/// 按 UTF-8 位元組限制外部字串長度。
/// @param value 待驗證字串。
/// @param field 錯誤訊息中的欄位名稱。
/// @param max_bytes 允許的最大位元組數。
/// @return 符合限制為 Ok，超長則回傳錯誤。
pub(crate) fn validate_length(value: &str, field: &str, max_bytes: usize) -> Result<()> {
    if value.len() > max_bytes {
        bail!("{field} exceeds {max_bytes} bytes");
    }
    Ok(())
}
