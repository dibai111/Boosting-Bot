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

//! 集中宣告可由 Tauri 呼叫的指令模組，業務流程由 runtime 實作。

pub(crate) mod account;
pub(crate) mod bot;
pub(crate) mod file;
pub(crate) mod matchmaking;
pub(crate) mod nick;
pub(crate) mod overlay;
pub(crate) mod settings;
pub(crate) mod shortcut;
pub(crate) mod system;
pub(crate) mod window;
