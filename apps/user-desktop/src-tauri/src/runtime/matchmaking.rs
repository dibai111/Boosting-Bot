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

//! 把前端請求編譯為配對計畫，並管理配對模式的進入與退出。

use super::UserRuntime;
use crate::{
    app_state::CommandResult,
    bot_runtime::RuntimeMode,
    matchmaking::{MatchmakingPlan, MatchmakingSnapshot, StartMatchmakingInput},
};

impl UserRuntime {
    /// 以儲存的玩家名稱編譯計畫，再進入配對模式。
    /// @param input 模式、Bot 選擇、驗證選項及玩家日誌路徑。
    /// @return 啟動快照；驗證、模式衝突或讀檔錯誤。
    pub(crate) async fn start_matchmaking(
        &self,
        input: StartMatchmakingInput,
    ) -> CommandResult<MatchmakingSnapshot> {
        let accounts = self
            .with_store(|store| store.list_accounts().map_err(|error| error.to_string()))
            .await?;
        let plan = MatchmakingPlan::compile(
            input,
            accounts
                .iter()
                .map(|account| (account.id.clone(), account.username.clone())),
        )
        .map_err(|error| error.to_string())?;
        self.reconcile_active_mode().await;
        self.enter_mode(RuntimeMode::Matching).await?;
        let result = self
            .matchmaking
            .start(plan)
            .await
            .map_err(|error| format!("start matchmaking: {error:#}"));
        if result.is_err() {
            self.leave_mode(RuntimeMode::Matching).await;
        }
        result
    }

    /// 停止日誌追蹤並撤回 Bot，釋放配對工作模式。
    /// @return 停止後的配對快照。
    pub(crate) async fn stop_matchmaking(&self) -> MatchmakingSnapshot {
        self.matchmaking.stop().await;
        self.leave_mode(RuntimeMode::Matching).await;
        self.matchmaking.snapshot().await
    }

    /// 取得目前配對狀態副本。
    /// @return 最新配對快照。
    pub(crate) async fn matchmaking_snapshot(&self) -> MatchmakingSnapshot {
        self.matchmaking.snapshot().await
    }
}
