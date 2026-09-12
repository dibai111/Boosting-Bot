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

//! 集中公開登入流程及穩定的錯誤代碼，供指令層轉成介面訊息。

pub(crate) mod access_token;
pub(crate) mod cookie;
pub(crate) mod microsoft;
pub(crate) mod profile;

pub(crate) const ACCESS_TOKEN_INVALID: &str = "access_token_invalid";
pub(crate) const ACCESS_TOKEN_EXPIRED: &str = "access_token_expired";
