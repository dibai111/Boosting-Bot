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

//! 驗證 Nick 設定及目標帳號，追蹤本次篩選的 Bot 集合。

use super::UserRuntime;
use crate::{
    app_state::CommandResult,
    bot_runtime::{BotCommand, NickRollerConfig, RuntimeMode},
    input_limits::{validate_length, MAX_BOT_ID_BYTES},
};
use anyhow::bail;
use local_store::AccountRecord;
use serde::Deserialize;

const MAX_NICK_BOTS: usize = 32;

#[derive(Debug, Deserialize)]
/// 指定參與 Nick 篩選的帳號 ID 與後端篩選設定。
pub(crate) struct StartNickRollerInput {
    pub(crate) bot_ids: Vec<String>,
    pub(crate) config: NickRollerConfig,
}

impl UserRuntime {
    /// 驗證帳號及 Hypixel 地址後，向所選 Bot 發送篩選指令。
    /// @param input Bot 清單及篩選設定；ID 會正規化與去重。
    /// @return 至少一個指令送出成功時回傳 NickRoller，否則回傳錯誤。
    pub(crate) async fn start_nick_roller(
        &self,
        mut input: StartNickRollerInput,
    ) -> CommandResult<RuntimeMode> {
        normalize_bot_ids(&mut input.bot_ids).map_err(|error| error.to_string())?;
        validate_input(&input).map_err(|error| error.to_string())?;
        let accounts = self
            .with_store(|store| store.list_accounts().map_err(|error| error.to_string()))
            .await?;
        validate_accounts(&input.bot_ids, &accounts).map_err(|error| error.to_string())?;

        self.reconcile_active_mode().await;
        self.enter_mode(RuntimeMode::NickRoller).await?;
        {
            let mut bot_ids = self.nick_bot_ids.lock().await;
            bot_ids.clear();
            bot_ids.extend(input.bot_ids.iter().cloned());
        }
        let config = input.config.clone().normalized();

        let mut failures = Vec::new();
        for bot_id in &input.bot_ids {
            let command = BotCommand::StartNickRoller {
                config: config.clone(),
            };
            if let Err(error) = self.bots.command(bot_id, command).await {
                self.nick_bot_ids.lock().await.remove(bot_id);
                failures.push(format!("{bot_id}: {error:#}"));
            }
        }

        if self.nick_bot_ids.lock().await.is_empty() {
            self.leave_mode(RuntimeMode::NickRoller).await;
            let message = if failures.is_empty() {
                "no Nick Roller bot could be started".to_owned()
            } else {
                failures.join("; ")
            };
            return Err(message);
        }

        Ok(RuntimeMode::NickRoller)
    }

    /// 向本次追蹤的 Bot 發送停止指令並清空模式追蹤。
    /// @return 停止請求結果；已關閉的 Bot 通道可忽略。
    pub(crate) async fn stop_nick_roller(&self) -> CommandResult<()> {
        let bot_ids = self
            .nick_bot_ids
            .lock()
            .await
            .iter()
            .cloned()
            .collect::<Vec<_>>();
        for bot_id in bot_ids {
            let _ = self.bots.command(&bot_id, BotCommand::StopNickRoller).await;
        }
        self.nick_bot_ids.lock().await.clear();
        self.leave_mode(RuntimeMode::NickRoller).await;
        Ok(())
    }

    /// 將使用者對候選的決策交回 Bot 狀態機。
    /// @param bot_id 候選所屬 Bot ID。
    /// @param candidate_id 事件提供的候選編號。
    /// @param take true 表示套用，false 表示跳過。
    /// @return 模式檢查與指令發送結果；候選時效由狀態機檢查。
    pub(crate) async fn answer_nick_decision(
        &self,
        bot_id: String,
        candidate_id: u64,
        take: bool,
    ) -> CommandResult<()> {
        validate_length(&bot_id, "bot id", MAX_BOT_ID_BYTES).map_err(|error| error.to_string())?;
        if self.active_mode().await != RuntimeMode::NickRoller {
            return Err("Nick Roller is not active".to_owned());
        }
        self.bots
            .command(&bot_id, BotCommand::NickDecision { candidate_id, take })
            .await
            .map_err(|error| error.to_string())
    }
}

fn normalize_bot_ids(bot_ids: &mut Vec<String>) -> anyhow::Result<()> {
    for bot_id in bot_ids.iter_mut() {
        *bot_id = bot_id.trim().to_owned();
    }
    bot_ids.retain(|bot_id| !bot_id.is_empty());
    bot_ids.sort_unstable();
    bot_ids.dedup();
    if bot_ids.is_empty() {
        bail!("select at least one bot")
    }
    if bot_ids.len() > MAX_NICK_BOTS {
        bail!("Nick Roller supports at most {MAX_NICK_BOTS} bots")
    }
    for bot_id in bot_ids {
        validate_length(bot_id, "bot id", MAX_BOT_ID_BYTES)?;
    }
    Ok(())
}

fn validate_input(input: &StartNickRollerInput) -> anyhow::Result<()> {
    let config = input.config.clone().normalized();
    if config
        .rules
        .min_length
        .zip(config.rules.max_length)
        .is_some_and(|(min, max)| min > max)
    {
        bail!("Nick Roller minimum length cannot exceed maximum length")
    }
    if config.rules.exact_length.is_some()
        && (config.rules.min_length.is_some() || config.rules.max_length.is_some())
    {
        bail!("choose exact length or minimum/maximum length, not both")
    }
    Ok(())
}

fn validate_accounts(bot_ids: &[String], accounts: &[AccountRecord]) -> anyhow::Result<()> {
    for bot_id in bot_ids {
        let Some(account) = accounts.iter().find(|account| account.id == *bot_id) else {
            bail!("one or more selected bots no longer exist")
        };
        if !is_hypixel_address(&account.server_address) {
            bail!("Nick Roller requires the selected bot server to be mc.hypixel.net")
        }
    }
    Ok(())
}

fn is_hypixel_address(address: &str) -> bool {
    address
        .trim()
        .split(':')
        .next()
        .is_some_and(|host| host.eq_ignore_ascii_case("mc.hypixel.net"))
}
