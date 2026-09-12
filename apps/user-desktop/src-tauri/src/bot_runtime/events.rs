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

//! 提供有界的廣播事件匯流排；沒有訂閱者時允許直接丟棄事件。

use super::BotEvent;
use tokio::sync::broadcast;

#[derive(Clone)]
/// 容量 256 的事件廣播匯流排；沒有訂閱者時允許丟棄事件。
pub(crate) struct BotEventBus {
    sender: broadcast::Sender<BotEvent>,
}

impl BotEventBus {
    /// 建立 Bot 事件廣播通道。
    /// @return 可複製的事件匯流排。
    pub(crate) fn new() -> Self {
        let (sender, _) = broadcast::channel(256);
        Self { sender }
    }

    /// 訂閱之後發布的事件。
    /// @return 接收端；落後超過容量時需處理 Lagged。
    pub(crate) fn subscribe(&self) -> broadcast::Receiver<BotEvent> {
        self.sender.subscribe()
    }

    /// 廣播一筆 Bot 事件。
    /// @param event 可序列化的 Bot 狀態或業務事件。
    /// @return 無回傳值；沒有接收端不視為失敗。
    pub(crate) fn publish(&self, event: BotEvent) {
        let _ = self.sender.send(event);
    }
}
