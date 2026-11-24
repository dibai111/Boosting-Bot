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

//! 同步產生者透過單一佇列交給 runtime，處理完成後才通知桌面。
use super::BotEvent;
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc;

#[derive(Clone)]
/// 同步 actor 無法等待容量；單一消費者佇列避免 broadcast 落後時遺失業務事件。
pub(crate) struct BotEventBus {
    business: mpsc::UnboundedSender<BotEvent>,
    business_receiver: Arc<Mutex<Option<mpsc::UnboundedReceiver<BotEvent>>>>,
    notifications: mpsc::UnboundedSender<BotEvent>,
    notification_receiver: Arc<Mutex<Option<mpsc::UnboundedReceiver<BotEvent>>>>,
}
impl BotEventBus {
    pub(crate) fn new() -> Self {
        let (business, business_receiver) = mpsc::unbounded_channel();
        let (notifications, notification_receiver) = mpsc::unbounded_channel();
        Self {
            business,
            business_receiver: Arc::new(Mutex::new(Some(business_receiver))),
            notifications,
            notification_receiver: Arc::new(Mutex::new(Some(notification_receiver))),
        }
    }
    /// runtime 在啟動時接管唯一業務接收端。
    pub(crate) fn take_business_events(&self) -> mpsc::UnboundedReceiver<BotEvent> {
        self.business_receiver
            .lock()
            .expect("business receiver lock")
            .take()
            .expect("business consumer already started")
    }
    /// 桌面接管處理後通知；移除桌面接收端不影響業務消費。
    pub(crate) fn take_notifications(&self) -> mpsc::UnboundedReceiver<BotEvent> {
        self.notification_receiver
            .lock()
            .expect("notification receiver lock")
            .take()
            .expect("notification consumer already started")
    }
    /// 發布業務輸入，不直接通知 UI。
    pub(crate) fn publish(&self, event: BotEvent) {
        let _ = self.business.send(event);
    }
    /// 僅由 runtime 在業務更新完成後轉送。
    pub(crate) fn notify(&self, event: BotEvent) {
        let _ = self.notifications.send(event);
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn a_burst_is_processed_in_order_without_a_desktop_subscriber() {
        let bus = BotEventBus::new();
        let mut input = bus.take_business_events();
        for i in 0..1024 {
            bus.publish(BotEvent::ChatMessage {
                bot_id: "a".to_owned(),
                message: i.to_string(),
            });
        }
        for i in 0..1024 {
            let BotEvent::ChatMessage { message, .. } = input.recv().await.unwrap() else {
                panic!("unexpected event")
            };
            assert_eq!(message, i.to_string());
        }
    }
}
