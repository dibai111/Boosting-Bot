use super::{session, BotConfig, BotEvent, BotPhase, SessionEmitter};
use anyhow::{Context, Result};
use std::thread::JoinHandle;
use tokio::sync::{mpsc, watch};

pub(super) struct SessionHandle {
    pub(super) generation: u64,
    pub(super) commands: mpsc::Sender<session::SessionCommand>,
    thread: Option<JoinHandle<()>>,
    done: watch::Receiver<bool>,
}

impl SessionHandle {
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
