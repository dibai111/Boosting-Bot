use super::{
    account::MinecraftAccessTokenAccount,
    afk::AdvancedAfk,
    detection::{self, PitchTracker},
    BotCommand, BotConfig, BotEvent, BotGamePhase, BotPhase, GameKind, MatchAttempt,
    SessionEmitter,
};
use azalea::{
    account::Account,
    app::PluginGroup,
    bot::DefaultBotPlugins,
    core::entity_id::MinecraftEntityId,
    ecs::{
        component::Component,
        query::{With, Without},
    },
    entity::{metadata::Player, LocalEntity, LookDirection},
    pathfinder::PathfinderPlugin,
    Client, ClientBuilder, ClientInformation, DefaultPlugins, Event,
};
use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::{Duration, Instant},
};
use tokio::sync::mpsc;

const EVENT_BUFFER: usize = 128;
const TRANSFER_TIMEOUT: Duration = Duration::from_secs(6);
const RETRY_TRANSFER_TIMEOUT: Duration = Duration::from_secs(12);
const PITCH_SCAN_INTERVAL: Duration = Duration::from_millis(100);
const LOW_VIEW_DISTANCE: u8 = 2;
const PITCH_VIEW_DISTANCE: u8 = 16;
const AZALEA_EXIT_GRACE: Duration = Duration::from_millis(200);

#[derive(Clone, Component, Default)]
struct HandlerState {
    inputs: Option<mpsc::Sender<SessionInput>>,
    shutdown_requested: Arc<AtomicBool>,
    exit_requested: Arc<AtomicBool>,
}

enum SessionInput {
    Init(Client),
    Login,
    Spawn,
    Chat(String),
    Tick,
    Disconnect(Option<String>),
    ConnectionFailed(String),
}

struct SessionOutcome {
    reason: String,
}

pub(super) async fn run(
    config: BotConfig,
    commands: mpsc::Receiver<SessionCommand>,
    emitter: SessionEmitter,
) {
    let account = Account::from(MinecraftAccessTokenAccount::new(
        config.username,
        config.uuid,
        config.access_token,
    ));
    let shutdown_requested = Arc::new(AtomicBool::new(false));
    let exit_requested = Arc::new(AtomicBool::new(false));
    let (inputs, input_receiver) = mpsc::channel(EVENT_BUFFER);
    let handler_state = HandlerState {
        inputs: Some(inputs),
        shutdown_requested: Arc::clone(&shutdown_requested),
        exit_requested: Arc::clone(&exit_requested),
    };

    emitter.publish(BotEvent::Status {
        bot_id: config.bot_id.clone(),
        phase: BotPhase::Authenticating,
        message: None,
    });

    let actor = SessionActor::new(
        config.bot_id,
        emitter.clone(),
        shutdown_requested.clone(),
        exit_requested,
        commands,
        input_receiver,
    );
    let mut actor_task = tokio::spawn(actor.run());

    // Azalea 的 PhysicsPlugin 必須保留 movement；只移除不會使用的 pathfinder。
    // 保留 Azalea 的物理插件以維持正常移動，只移除未使用的尋路功能。
    let _exit = ClientBuilder::new_without_plugins()
        .add_plugins(DefaultPlugins)
        .add_plugins(DefaultBotPlugins.build().disable::<PathfinderPlugin>())
        .reconnect_after(None)
        .set_handler(handle_azalea_event)
        .set_state(handler_state)
        .start(account, config.server_address)
        .await;

    shutdown_requested.store(true, Ordering::Release);
    let outcome = match tokio::time::timeout(Duration::from_secs(2), &mut actor_task).await {
        Ok(Ok(outcome)) => outcome,
        Ok(Err(_)) => {
            let message =
                "Bot session stopped unexpectedly while processing server events".to_owned();
            emitter.publish(BotEvent::Error {
                bot_id: Some(emitter.bot_id().to_owned()),
                code: "session_panic".to_owned(),
                message: message.clone(),
            });
            SessionOutcome { reason: message }
        }
        Err(_) => {
            actor_task.abort();
            SessionOutcome {
                reason: "Bot session stopped".to_owned(),
            }
        }
    };
    emitter.publish(BotEvent::Status {
        bot_id: emitter.bot_id().to_owned(),
        phase: BotPhase::Offline,
        message: Some(outcome.reason),
    });
}

async fn handle_azalea_event(bot: Client, event: Event, state: HandlerState) {
    if state.shutdown_requested.load(Ordering::Acquire) {
        exit_after_grace(bot, state.exit_requested).await;
        return;
    }
    let Some(inputs) = state.inputs else {
        exit_after_grace(bot, state.exit_requested).await;
        return;
    };

    let input = match event {
        Event::Init => {
            // 非 pitch verification 場景只保留 2 chunks，減少 Azalea 儲存的世界資料。
            bot.set_client_information(ClientInformation {
                view_distance: LOW_VIEW_DISTANCE,
                ..Default::default()
            });
            SessionInput::Init(bot.clone())
        }
        Event::Login => SessionInput::Login,
        Event::Spawn => SessionInput::Spawn,
        Event::Chat(chat) => SessionInput::Chat(chat.message().to_string()),
        Event::Tick => {
            let _ = inputs.try_send(SessionInput::Tick);
            return;
        }
        Event::Disconnect(reason) => {
            SessionInput::Disconnect(reason.map(|message| message.to_string()))
        }
        Event::ConnectionFailed(error) => SessionInput::ConnectionFailed(error.to_string()),
        _ => return,
    };

    let should_exit = matches!(
        &input,
        SessionInput::Disconnect(_) | SessionInput::ConnectionFailed(_)
    );
    if inputs.send(input).await.is_err() || should_exit {
        exit_after_grace(bot, state.exit_requested).await;
    }
}

async fn exit_after_grace(bot: Client, exit_requested: Arc<AtomicBool>) {
    // Let Azalea forward queued disconnect events before AppExit closes its receivers.
    tokio::time::sleep(AZALEA_EXIT_GRACE).await;
    request_client_exit(&bot, &exit_requested);
}

fn request_client_exit(bot: &Client, exit_requested: &AtomicBool) {
    if claim_client_exit(exit_requested) {
        bot.exit();
    }
}

fn claim_client_exit(exit_requested: &AtomicBool) -> bool {
    exit_requested
        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
        .is_ok()
}

pub(super) enum SessionCommand {
    Command(BotCommand),
    Stop { reason: String },
}

struct SessionActor {
    bot_id: String,
    emitter: SessionEmitter,
    shutdown_requested: Arc<AtomicBool>,
    exit_requested: Arc<AtomicBool>,
    commands: mpsc::Receiver<SessionCommand>,
    inputs: mpsc::Receiver<SessionInput>,
    client: Option<Client>,
    spawned: bool,
    connection_closed: bool,
    request_azalea_exit: bool,
    afk: AdvancedAfk,
    matchmaking: MatchController,
    exit_reason: String,
}

impl SessionActor {
    fn new(
        bot_id: String,
        emitter: SessionEmitter,
        shutdown_requested: Arc<AtomicBool>,
        exit_requested: Arc<AtomicBool>,
        commands: mpsc::Receiver<SessionCommand>,
        inputs: mpsc::Receiver<SessionInput>,
    ) -> Self {
        let now = Instant::now();
        Self {
            bot_id,
            emitter,
            shutdown_requested,
            exit_requested,
            commands,
            inputs,
            client: None,
            spawned: false,
            connection_closed: false,
            request_azalea_exit: true,
            afk: AdvancedAfk::new(now),
            matchmaking: MatchController::new(),
            exit_reason: "Bot session ended".to_owned(),
        }
    }

    async fn run(mut self) -> SessionOutcome {
        let mut timers = tokio::time::interval(Duration::from_millis(50));
        timers.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        loop {
            let should_exit = tokio::select! {
                command = self.commands.recv() => match command {
                    Some(command) => self.handle_command(command),
                    None => {
                        self.exit_reason = "Bot runtime closed".to_owned();
                        true
                    }
                },
                input = self.inputs.recv() => match input {
                    Some(input) => self.handle_input(input),
                    None => {
                        self.exit_reason = "Minecraft event stream closed".to_owned();
                        true
                    }
                },
                _ = timers.tick() => {
                    self.matchmaking.check_timeouts(
                        Instant::now(),
                        &self.bot_id,
                        &self.emitter,
                    );
                    false
                },
            };
            if should_exit {
                break;
            }
        }

        let client = self.client.clone();
        self.cleanup();
        if self.request_azalea_exit {
            if let Some(client) = client {
                exit_after_grace(client, Arc::clone(&self.exit_requested)).await;
            }
        }
        SessionOutcome {
            reason: self.exit_reason,
        }
    }

    fn handle_command(&mut self, command: SessionCommand) -> bool {
        match command {
            SessionCommand::Stop { reason } => {
                self.exit_reason = reason;
                self.shutdown_requested.store(true, Ordering::Release);
                true
            }
            SessionCommand::Command(command) => {
                self.dispatch(command);
                false
            }
        }
    }

    fn dispatch(&mut self, command: BotCommand) {
        let Some(client) = self.client.as_ref() else {
            self.emit_not_ready(&command, "Bot has not connected");
            return;
        };
        match command {
            BotCommand::SendChat(message) => {
                if self.spawned {
                    client.chat(message);
                } else {
                    self.emit_error("bot_not_ready", "Bot is not ready for chat");
                }
            }
            BotCommand::BeginMatchAttempt(attempt) => {
                if self.spawned {
                    // 只有需要 pitch verification 時恢復預設視距，避免影響附近玩家偵測。
                    let view_distance = if attempt.requires_pitch_verification {
                        PITCH_VIEW_DISTANCE
                    } else {
                        LOW_VIEW_DISTANCE
                    };
                    client.set_client_information(ClientInformation {
                        view_distance,
                        ..Default::default()
                    });
                    self.matchmaking.begin(client, attempt, Instant::now());
                } else {
                    self.emit_error("match_not_ready", "Bot has not spawned");
                }
            }
            BotCommand::ReturnToLobby => {
                self.matchmaking.return_to_lobby(client, self.spawned);
            }
            BotCommand::PrepareMatchRetry { request_id } => {
                if self.spawned {
                    self.matchmaking
                        .prepare_retry(client, request_id, Instant::now());
                } else {
                    self.emitter.publish(BotEvent::MatchRetryPreparationFailed {
                        bot_id: self.bot_id.clone(),
                        request_id,
                        message: "Bot has not spawned".to_owned(),
                    });
                }
            }
            BotCommand::CancelMatchAttempt => self.matchmaking.cancel(),
        }
    }

    fn handle_input(&mut self, input: SessionInput) -> bool {
        match input {
            SessionInput::Init(client) => {
                self.client = Some(client);
                self.emitter.publish(BotEvent::Status {
                    bot_id: self.bot_id.clone(),
                    phase: BotPhase::Connecting,
                    message: None,
                });
                self.shutdown_requested.load(Ordering::Acquire)
            }
            SessionInput::Login => {
                if let Some(client) = self.client.as_ref() {
                    self.emitter.publish(BotEvent::Profile {
                        bot_id: self.bot_id.clone(),
                        username: client.username(),
                        uuid: Some(client.uuid().to_string()),
                    });
                }
                false
            }
            SessionInput::Spawn => {
                let Some(client) = self.client.as_ref() else {
                    return false;
                };
                let first_spawn = !self.spawned;
                self.spawned = true;
                if self.afk.start(client, Instant::now()) {
                    self.emitter.publish(BotEvent::AfkState {
                        bot_id: self.bot_id.clone(),
                        active: true,
                    });
                }
                if first_spawn {
                    self.emitter.publish(BotEvent::Status {
                        bot_id: self.bot_id.clone(),
                        phase: BotPhase::Online,
                        message: None,
                    });
                }
                false
            }
            SessionInput::Chat(message) => {
                if let Some(safe) = detection::safe_chat(&message) {
                    self.emitter.publish(BotEvent::ChatMessage {
                        bot_id: self.bot_id.clone(),
                        message: safe,
                    });
                }
                self.matchmaking
                    .handle_chat(&message, &self.bot_id, &self.emitter, Instant::now());
                false
            }
            SessionInput::Tick => {
                if let Some(client) = self.client.as_ref() {
                    let now = Instant::now();
                    self.afk.tick(client, now);
                    self.matchmaking
                        .tick(client, now, &self.bot_id, &self.emitter);
                }
                false
            }
            SessionInput::Disconnect(reason) => {
                self.connection_closed = true;
                self.request_azalea_exit = false;
                self.shutdown_requested.store(true, Ordering::Release);
                let message = detection::disconnect_reason(reason.as_deref());
                self.fail(detection::disconnect_code(&message), &message);
                true
            }
            SessionInput::ConnectionFailed(message) => {
                self.connection_closed = true;
                self.request_azalea_exit = false;
                self.shutdown_requested.store(true, Ordering::Release);
                self.fail("connection_error", &message);
                true
            }
        }
    }

    fn cleanup(&mut self) {
        self.spawned = false;
        self.matchmaking.cancel();
        let client = if self.connection_closed {
            None
        } else {
            self.client.as_ref()
        };
        if self.afk.stop(client) {
            self.emitter.publish(BotEvent::AfkState {
                bot_id: self.bot_id.clone(),
                active: false,
            });
        }
    }

    fn emit_not_ready(&self, command: &BotCommand, message: &str) {
        if let BotCommand::PrepareMatchRetry { request_id } = command {
            self.emitter.publish(BotEvent::MatchRetryPreparationFailed {
                bot_id: self.bot_id.clone(),
                request_id: request_id.clone(),
                message: message.to_owned(),
            });
        } else {
            self.emit_error("bot_not_ready", message);
        }
    }

    fn emit_error(&self, code: &str, message: &str) {
        self.emitter.publish(BotEvent::Error {
            bot_id: Some(self.bot_id.clone()),
            code: code.to_owned(),
            message: message.to_owned(),
        });
    }

    fn fail(&mut self, code: &str, message: &str) {
        self.exit_reason = message.to_owned();
        self.emit_error(code, message);
    }
}

struct MatchController {
    active: Option<ActiveMatch>,
    retry: Option<PendingRetry>,
    pitch: PitchTracker,
}

struct ActiveMatch {
    attempt: MatchAttempt,
    waiting_for_transfer: bool,
    transfer_deadline: Instant,
    game_state: Option<BotGamePhase>,
    pitch_armed: bool,
    next_pitch_scan: Instant,
}

struct PendingRetry {
    request_id: String,
    deadline: Instant,
}

impl MatchController {
    fn new() -> Self {
        Self {
            active: None,
            retry: None,
            pitch: PitchTracker::new(),
        }
    }

    fn begin(&mut self, client: &Client, attempt: MatchAttempt, now: Instant) {
        self.cancel();
        client.chat(attempt.mode.play_command());
        self.active = Some(ActiveMatch {
            attempt,
            waiting_for_transfer: true,
            transfer_deadline: now + TRANSFER_TIMEOUT,
            game_state: None,
            pitch_armed: false,
            next_pitch_scan: now,
        });
    }

    fn cancel(&mut self) {
        self.active = None;
        self.retry = None;
        self.pitch.reset();
    }

    fn return_to_lobby(&mut self, client: &Client, spawned: bool) {
        self.cancel();
        if spawned {
            client.chat("/l");
        }
    }

    fn prepare_retry(&mut self, client: &Client, request_id: String, now: Instant) {
        self.cancel();
        self.retry = Some(PendingRetry {
            request_id,
            deadline: now + RETRY_TRANSFER_TIMEOUT,
        });
        client.chat("/limbo");
    }

    fn handle_chat(&mut self, message: &str, bot_id: &str, emitter: &SessionEmitter, now: Instant) {
        // /limbo 不一定觸發 Azalea Spawn；Hypixel 的確認聊天才代表 retry 已可繼續。
        if detection::limbo_spawn(message) {
            if let Some(retry) = self.retry.take() {
                emitter.publish(BotEvent::MatchRetryReady {
                    bot_id: bot_id.to_owned(),
                    request_id: retry.request_id,
                });
                return;
            }
        }

        if self.active.is_some() {
            if let Some((current, total)) = detection::queue_progress(message) {
                emitter.publish(BotEvent::QueueProgress {
                    bot_id: bot_id.to_owned(),
                    current,
                    total,
                });
            }
        }

        if self
            .active
            .as_ref()
            .is_some_and(|active| active.waiting_for_transfer)
        {
            if let Some(rejection) = detection::command_rejection(message) {
                let Some(active) = self.active.take() else {
                    return;
                };
                emit_attempt_failure(emitter, bot_id, active.attempt, "command_spam", rejection);
                return;
            }

            if let Some(server) = detection::server_transfer(message) {
                let (attempt, start_pitch_observation) = {
                    let Some(active) = self.active.as_mut() else {
                        return;
                    };
                    active.waiting_for_transfer = false;
                    let start_pitch_observation = active.attempt.mode.kind() == GameKind::Duels;
                    if start_pitch_observation {
                        active.pitch_armed = true;
                        active.next_pitch_scan = now;
                    }
                    (active.attempt.clone(), start_pitch_observation)
                };
                if start_pitch_observation {
                    emitter.publish(BotEvent::MatchmakingDebug {
                        bot_id: bot_id.to_owned(),
                        message: "[duels pitch] observation started".to_owned(),
                    });
                }
                emitter.publish(BotEvent::MatchAttemptResult {
                    bot_id: bot_id.to_owned(),
                    session_id: attempt.session_id,
                    round_id: attempt.round_id,
                    target_generation: attempt.target_generation,
                    attempt_id: attempt.attempt_id,
                    server,
                });
            }
        }

        let Some(active) = self.active.as_mut() else {
            return;
        };
        let Some(state) = detection::game_state(active.attempt.mode.kind(), message) else {
            return;
        };
        if active.game_state == Some(state) {
            return;
        }
        active.game_state = Some(state);
        emitter.publish(BotEvent::BotGameState {
            bot_id: bot_id.to_owned(),
            round_id: active.attempt.round_id.clone(),
            target_generation: active.attempt.target_generation,
            state,
        });
    }

    fn tick(&mut self, client: &Client, now: Instant, bot_id: &str, emitter: &SessionEmitter) {
        let Some(active) = self.active.as_mut() else {
            return;
        };
        if !active.pitch_armed || now < active.next_pitch_scan {
            return;
        }
        active.next_pitch_scan = now + PITCH_SCAN_INTERVAL;

        let players =
            client.nearest_entity_ids_by::<(), (With<Player>, Without<LocalEntity>)>(|()| true);
        for entity in players {
            let Ok((entity_id, pitch)) = client
                .try_query_entity::<(&MinecraftEntityId, &LookDirection), _>(
                    entity,
                    |(id, look)| (id.0, look.x_rot()),
                )
            else {
                continue;
            };
            let Some(direction) = self.pitch.observe(entity_id, pitch, now) else {
                continue;
            };

            active.pitch_armed = false;
            emitter.publish(BotEvent::MatchmakingDebug {
                bot_id: bot_id.to_owned(),
                message: format!(
                    "[duels pitch] gesture confirmed: direction={}, pitch={pitch:.1}deg",
                    direction.as_str()
                ),
            });
            emitter.publish(BotEvent::DuelPitchObserved {
                bot_id: bot_id.to_owned(),
                round_id: active.attempt.round_id.clone(),
                target_generation: active.attempt.target_generation,
                attempt_id: active.attempt.attempt_id.clone(),
                direction,
                pitch: pitch.to_radians(),
            });
            break;
        }
    }

    fn check_timeouts(&mut self, now: Instant, bot_id: &str, emitter: &SessionEmitter) {
        if self
            .active
            .as_ref()
            .is_some_and(|active| active.waiting_for_transfer && now >= active.transfer_deadline)
        {
            let Some(active) = self.active.take() else {
                return;
            };
            emit_attempt_failure(
                emitter,
                bot_id,
                active.attempt,
                "server_transfer_timeout",
                "server transfer chat timed out".to_owned(),
            );
        }

        if self
            .retry
            .as_ref()
            .is_some_and(|retry| now >= retry.deadline)
        {
            let Some(retry) = self.retry.take() else {
                return;
            };
            emitter.publish(BotEvent::MatchRetryPreparationFailed {
                bot_id: bot_id.to_owned(),
                request_id: retry.request_id,
                message: "server transfer timed out".to_owned(),
            });
        }
    }
}

fn emit_attempt_failure(
    emitter: &SessionEmitter,
    bot_id: &str,
    attempt: MatchAttempt,
    code: &str,
    message: String,
) {
    emitter.publish(BotEvent::MatchAttemptFailed {
        bot_id: bot_id.to_owned(),
        session_id: attempt.session_id,
        round_id: attempt.round_id,
        target_generation: attempt.target_generation,
        attempt_id: attempt.attempt_id,
        code: code.to_owned(),
        message,
    });
}

#[cfg(test)]
mod tests {
    use super::claim_client_exit;
    use std::sync::atomic::AtomicBool;

    #[test]
    fn client_exit_is_claimed_once() {
        let exit_requested = AtomicBool::new(false);

        assert!(claim_client_exit(&exit_requested));
        assert!(!claim_client_exit(&exit_requested));
    }
}
