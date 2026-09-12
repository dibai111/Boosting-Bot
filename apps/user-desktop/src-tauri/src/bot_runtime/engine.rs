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

//! 為每個 Bot 建立獨立執行緒與 Tokio runtime，管理指令通道及結束通知。

use super::{session, BotConfig, BotEvent, BotPhase, SessionEmitter};
use anyhow::{Context, Result};
use std::thread::JoinHandle;
use tokio::sync::{mpsc, watch};

/// 持有單一 Bot 執行緒、指令通道及完成通知的生命週期控制柄。
pub(super) struct SessionHandle {
    pub(super) generation: u64,
    pub(super) commands: mpsc::Sender<session::SessionCommand>,
    thread: Option<JoinHandle<()>>,
    done: watch::Receiver<bool>,
}

impl SessionHandle {
    /// 送出停止指令並等待執行緒結束；五秒通知逾時不代表 join 有時限。
    /// @param reason 提供給工作階段與 UI 的停止原因。
    /// @return 執行緒 join 返回後完成。
    pub(super) async fn stop(mut self, reason: &str) {
        let _ = self
            .commands
            .send(session::SessionCommand::Stop {
                reason: reason.to_owned(),
            })
            .await;

        if !*self.done.borrow() {
            let mut done = self.done.clone();
            let _ = tokio::time::timeout(std::time::Duration::from_secs(5), async move {
                while !*done.borrow() && done.changed().await.is_ok() {}
            })
            .await;
        }

        if let Some(thread) = self.thread.take() {
            let _ = tokio::task::spawn_blocking(move || thread.join()).await;
        }
    }
}

/// 為 Bot 建立獨立執行緒與單執行緒 Tokio runtime。
/// @param config 已驗證的 Bot 連線設定。
/// @param generation 這次連線的世代編號。
/// @param emitter 只轉送本世代事件的發送器。
/// @return 可停止的工作階段 handle，或執行緒建立錯誤。
pub(super) fn spawn(
    config: BotConfig,
    generation: u64,
    emitter: SessionEmitter,
) -> Result<SessionHandle> {
    let (done_sender, done) = watch::channel(false);
    let (command_sender, command_receiver) = mpsc::channel(32);

    let thread_name = format!("bot-{}", short_id(&config.bot_id));
    let thread_emitter = emitter.clone();
    let thread = std::thread::Builder::new()
        .name(thread_name)
        .spawn(move || {
            match tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
            {
                Ok(runtime) => {
                    runtime.block_on(session::run(config, command_receiver, thread_emitter))
                }
                Err(error) => {
                    let message = format!("build bot runtime: {error}");
                    thread_emitter.publish(BotEvent::Error {
                        bot_id: Some(thread_emitter.bot_id().to_owned()),
                        code: "start_failed".to_owned(),
                        message: message.clone(),
                    });
                    thread_emitter.publish(BotEvent::Status {
                        bot_id: thread_emitter.bot_id().to_owned(),
                        phase: BotPhase::Offline,
                        message: Some(message),
                    });
                }
            }
            let _ = done_sender.send(true);
        })
        .context("spawn Azalea bot thread")?;

    Ok(SessionHandle {
        generation,
        commands: command_sender,
        thread: Some(thread),
        done,
    })
}

fn short_id(bot_id: &str) -> String {
    let value = bot_id
        .chars()
        .filter(|character| character.is_ascii_alphanumeric())
        .take(8)
        .collect::<String>();
    if value.is_empty() {
        "session".to_owned()
    } else {
        value
    }
}
