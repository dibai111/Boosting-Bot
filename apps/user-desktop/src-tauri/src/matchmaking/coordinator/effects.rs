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

//! 將連線與計時操作從配對決策抽離，讓狀態轉換能獨立測試。
use super::input::ScheduledTimer;
use crate::bot_runtime::{BotCommand, BotRuntime};
use anyhow::Result;
use std::{
    future::Future,
    pin::Pin,
    sync::Arc,
    time::{Duration, Instant},
};

pub(super) type CommandResult<'a> = Pin<Box<dyn Future<Output = Result<()>> + Send + 'a>>;

/// 正式環境透過此介面操作連線與計時器，測試可記錄指令並控制時鐘。
pub(super) trait MatchmakingEffects: Send + Sync {
    fn command<'a>(&'a self, bot_id: &'a str, command: BotCommand) -> CommandResult<'a>;
    fn now(&self) -> Instant;
    fn schedule(&self, delay: Duration, timer: ScheduledTimer);
}

pub(super) struct RuntimeEffects(pub Arc<BotRuntime>);
impl MatchmakingEffects for RuntimeEffects {
    fn command<'a>(&'a self, bot_id: &'a str, command: BotCommand) -> CommandResult<'a> {
        Box::pin(self.0.command(bot_id, command))
    }
    fn now(&self) -> Instant {
        Instant::now()
    }
    fn schedule(&self, delay: Duration, timer: ScheduledTimer) {
        tokio::spawn(async move {
            tokio::time::sleep(delay).await;
            timer.deliver().await;
        });
    }
}
