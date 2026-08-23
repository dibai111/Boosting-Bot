use super::BotEvent;
use tokio::sync::broadcast;

#[derive(Clone)]
pub(crate) struct BotEventBus {
    sender: broadcast::Sender<BotEvent>,
}

impl BotEventBus {
    pub(crate) fn new() -> Self {
        let (sender, _) = broadcast::channel(256);
        Self { sender }
    }

    pub(crate) fn subscribe(&self) -> broadcast::Receiver<BotEvent> {
        self.sender.subscribe()
    }

    pub(crate) fn publish(&self, event: BotEvent) {
        let _ = self.sender.send(event);
    }
}
