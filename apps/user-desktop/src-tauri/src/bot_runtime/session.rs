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

//! 將 Azalea 事件送入單一 actor，協調 AFK、配對與 Nick 流程的生命週期。

use super::{
    account::MinecraftAccessTokenAccount,
    afk::AdvancedAfk,
    detection,
    match_controller::MatchController,
    nick_roller::{NickAction, NickBook, NickInput, NickRoller, NickRollerPhase},
    BotCommand, BotConfig, BotEvent, BotPhase, SessionEmitter,
};
use azalea::{
    account::Account,
    app::PluginGroup,
    bot::DefaultBotPlugins,
    ecs::component::Component,
    inventory::{components::WrittenBookContent, ItemStack},
    pathfinder::PathfinderPlugin,
    registry::builtin::ItemKind,
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
const LOW_VIEW_DISTANCE: u8 = 2;
const PITCH_VIEW_DISTANCE: u8 = 16;
const AZALEA_EXIT_GRACE: Duration = Duration::from_millis(200);

#[derive(Clone, Component, Default)]
/// Azalea 回呼共用的輸入通道與原子停止旗標。
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

/// 組合 Azalea 連線與 actor，待兩者收尾後發布離線事件。
/// @param config 已驗證的連線設定。
/// @param commands 外部指令接收端。
/// @param emitter 此連線的事件發送器。
/// @return 整個 Bot 工作階段結束後完成。
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

    // 保留物理插件維持正常移動，只移除不使用的尋路功能。
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

/// 把 Azalea 事件轉入 actor；通道滿時可略過 Tick。
/// @param bot 目前連線 client。
/// @param event Azalea 提供的原始事件。
/// @param state 共享輸入通道及停止旗標。
/// @return 事件入列或退出安排完成後返回。
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
        // tick 可合併；通道已滿時略過本次，避免累積過期的週期工作。
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
    // 先讓 Azalea 轉送已排入佇列的斷線事件，再以 AppExit 關閉接收端。
    tokio::time::sleep(AZALEA_EXIT_GRACE).await;
    request_client_exit(&bot, &exit_requested);
}

fn request_client_exit(bot: &Client, exit_requested: &AtomicBool) {
    if claim_client_exit(exit_requested) {
        bot.exit();
    }
}

/// 以原子交換確保每個 client 只發出一次退出要求。
/// @param exit_requested 此連線共享的退出旗標。
/// @return 本次取得退出責任時為 true。
fn claim_client_exit(exit_requested: &AtomicBool) -> bool {
    exit_requested
        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
        .is_ok()
}

pub(super) enum SessionCommand {
    Command(BotCommand),
    Stop { reason: String },
}

/// 以單一 actor 擁有連線、AFK、配對及 Nick 可變狀態，避免跨回呼交錯修改。
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
    nick_roller: NickRoller,
    last_nick_book: Option<ItemStack>,
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
            nick_roller: NickRoller::new(),
            last_nick_book: None,
            exit_reason: "Bot session ended".to_owned(),
        }
    }

    /// 輪詢指令、連線事件與逾時計時器，停止時清理所有子流程。
    /// @return 包含最終停止原因的 SessionOutcome。
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

    /// 將業務指令分派至 Nick 或一般連線流程。
    /// @param command BotRuntime 傳入的指令。
    /// @return 無回傳值；未就緒時發布錯誤事件。
    fn dispatch(&mut self, command: BotCommand) {
        match command {
            BotCommand::StartNickRoller { config } => {
                let Some(client) = self.client.clone() else {
                    self.emit_nick_failure("Bot has not connected");
                    return;
                };
                if !self.spawned {
                    self.emit_nick_failure("Bot has not spawned");
                    return;
                }
                self.matchmaking.cancel();
                self.stop_afk(&client);
                self.last_nick_book = None;
                let actions = self.nick_roller.handle(NickInput::Start {
                    config,
                    now: Instant::now(),
                });
                self.apply_nick_actions(Some(&client), actions);
            }
            BotCommand::StopNickRoller => {
                self.last_nick_book = None;
                let actions = self.nick_roller.handle(NickInput::Stop);
                let client = self.client.clone();
                self.apply_nick_actions(client.as_ref(), actions);
            }
            BotCommand::NickDecision { candidate_id, take } => {
                let actions = self.nick_roller.handle(NickInput::Decision {
                    candidate_id,
                    take,
                    now: Instant::now(),
                });
                let client = self.client.clone();
                self.apply_nick_actions(client.as_ref(), actions);
            }
            other => {
                let Some(client) = self.client.clone() else {
                    self.emit_not_ready(&other, "Bot has not connected");
                    return;
                };
                self.dispatch_standard(other, &client);
            }
        }
    }

    /// 執行聊天、配對及回大廳指令，維持 Nick 互斥限制。
    /// @param command 一般 Bot 業務指令。
    /// @param client 已取得的 Azalea client。
    /// @return 無回傳值；執行結果透過事件回報。
    fn dispatch_standard(&mut self, command: BotCommand, client: &Client) {
        match command {
            BotCommand::SendChat(message) => {
                if self.spawned {
                    client.chat(message);
                } else {
                    self.emit_error("bot_not_ready", "Bot is not ready for chat");
                }
            }
            BotCommand::BeginMatchAttempt(attempt) => {
                if self.nick_roller.is_active() {
                    self.emit_error("nick_roller_active", "Nick Roller is currently active");
                } else if self.spawned {
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
            BotCommand::StartNickRoller { .. }
            | BotCommand::StopNickRoller
            | BotCommand::NickDecision { .. } => {
                self.emit_error("invalid_command", "Nick Roller command was not dispatched");
            }
        }
    }

    /// 以連線事件推進 Bot 生命週期與業務狀態機。
    /// @param input 已轉換的 Azalea 輸入。
    /// @return true 表示 actor 應結束，false 表示繼續。
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
                let Some(client) = self.client.clone() else {
                    return false;
                };
                let first_spawn = !self.spawned;
                self.spawned = true;
                if self.nick_roller.is_active() {
                    let actions = self.nick_roller.handle(NickInput::Spawn {
                        now: Instant::now(),
                    });
                    self.apply_nick_actions(Some(&client), actions);
                } else if self.afk.start(&client, Instant::now()) {
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
                let now = Instant::now();
                let nick_actions = self.nick_roller.handle(NickInput::Chat {
                    message: message.clone(),
                    now,
                });
                let client = self.client.clone();
                self.apply_nick_actions(client.as_ref(), nick_actions);
                self.matchmaking
                    .handle_chat(&message, &self.bot_id, &self.emitter, now);
                false
            }
            SessionInput::Tick => {
                if let Some(client) = self.client.clone() {
                    let now = Instant::now();
                    if self.nick_roller.is_active() {
                        if let Some(book) = self.read_nick_book(&client) {
                            let actions = self.nick_roller.handle(NickInput::Book { book, now });
                            self.apply_nick_actions(Some(&client), actions);
                        }
                        let actions = self.nick_roller.handle(NickInput::Tick { now });
                        self.apply_nick_actions(Some(&client), actions);
                    } else {
                        self.afk.tick(&client, now);
                        self.matchmaking
                            .tick(&client, now, &self.bot_id, &self.emitter);
                    }
                }
                false
            }
            SessionInput::Disconnect(reason) => {
                let actions = self.nick_roller.handle(NickInput::Stop);
                self.apply_nick_actions(None, actions);
                self.last_nick_book = None;
                self.connection_closed = true;
                self.request_azalea_exit = false;
                self.shutdown_requested.store(true, Ordering::Release);
                let message = detection::disconnect_reason(reason.as_deref());
                self.fail(detection::disconnect_code(&message), &message);
                true
            }
            SessionInput::ConnectionFailed(message) => {
                let actions = self.nick_roller.handle(NickInput::Stop);
                self.apply_nick_actions(None, actions);
                self.last_nick_book = None;
                self.connection_closed = true;
                self.request_azalea_exit = false;
                self.shutdown_requested.store(true, Ordering::Release);
                self.fail("connection_error", &message);
                true
            }
        }
    }

    /// 清除 Nick、配對、AFK 及書本快取，斷線後不再對 client 發送動作。
    /// @return 無回傳值。
    fn cleanup(&mut self) {
        let client = if self.connection_closed {
            None
        } else {
            self.client.clone()
        };
        self.spawned = false;
        if self.nick_roller.is_active() {
            let actions = self.nick_roller.handle(NickInput::Stop);
            self.apply_nick_actions(client.as_ref(), actions);
        }
        self.last_nick_book = None;
        self.matchmaking.cancel();
        if self.afk.stop(client.as_ref()) {
            self.emitter.publish(BotEvent::AfkState {
                bot_id: self.bot_id.clone(),
                active: false,
            });
        }
    }

    /// 依序執行 Nick 狀態機輸出的聊天及 UI 事件。
    /// @param client 可用的連線；斷線時為 None。
    /// @param actions 此輪狀態轉移產生的有序動作。
    /// @return 無回傳值；結束時視連線狀態恢復 AFK。
    fn apply_nick_actions(&mut self, client: Option<&Client>, actions: Vec<NickAction>) {
        let mut terminal = false;
        for action in actions {
            match action {
                NickAction::SendChat(message) => {
                    if self.spawned {
                        if let Some(client) = client {
                            client.chat(message);
                        }
                    }
                }
                NickAction::State { phase, message } => {
                    terminal |= phase.is_terminal();
                    self.emitter.publish(BotEvent::NickRollerState {
                        bot_id: self.bot_id.clone(),
                        phase,
                        message,
                    });
                }
                NickAction::Candidate {
                    candidate_id,
                    nick,
                    accepted,
                    reasons,
                    processed_count,
                    accepted_count,
                    rejected_count,
                    decision_timeout_ms,
                } => self.emitter.publish(BotEvent::NickCandidate {
                    bot_id: self.bot_id.clone(),
                    candidate_id,
                    nick,
                    accepted,
                    reasons,
                    processed_count,
                    accepted_count,
                    rejected_count,
                    decision_timeout_ms,
                }),
                NickAction::Verification {
                    candidate_id,
                    expected_nick,
                    actual_nick,
                    success,
                    reason,
                    processed_count,
                } => self.emitter.publish(BotEvent::NickVerification {
                    bot_id: self.bot_id.clone(),
                    candidate_id,
                    expected_nick,
                    actual_nick,
                    success,
                    reason,
                    processed_count,
                }),
                NickAction::Attention { code, message } => {
                    self.emitter.publish(BotEvent::NickAttention {
                        bot_id: self.bot_id.clone(),
                        code,
                        message,
                    });
                }
            }
        }

        if terminal {
            self.last_nick_book = None;
            self.start_afk_if_ready(client);
        }
    }

    fn start_afk_if_ready(&mut self, client: Option<&Client>) {
        let Some(client) = client else {
            return;
        };
        if self.spawned && !self.nick_roller.is_active() && self.afk.start(client, Instant::now()) {
            self.emitter.publish(BotEvent::AfkState {
                bot_id: self.bot_id.clone(),
                active: true,
            });
        }
    }

    fn stop_afk(&mut self, client: &Client) {
        if self.afk.stop(Some(client)) {
            self.emitter.publish(BotEvent::AfkState {
                bot_id: self.bot_id.clone(),
                active: false,
            });
        }
    }

    // 背包每 tick 都可讀到同一本書；只在內容改變時交給 Nick 狀態機。
    /// 只在背包中的書本內容變更時解讀 Nick 結果。
    /// @param client 目前已建立的 Azalea 連線。
    /// @return 新書本頁面；沒有書本或內容未變時為 None。
    fn read_nick_book(&mut self, client: &Client) -> Option<NickBook> {
        let item = client
            .get_inventory()
            .slots()?
            .into_iter()
            .find(|item| item.kind() == ItemKind::WrittenBook);
        let Some(item) = item else {
            self.last_nick_book = None;
            return None;
        };
        if self.last_nick_book.as_ref() == Some(&item) {
            return None;
        }
        self.last_nick_book = Some(item.clone());
        let pages = item
            .get_component::<WrittenBookContent>()
            .map(|content| {
                content
                    .pages
                    .iter()
                    .map(|page| page.filtered.as_ref().unwrap_or(&page.raw).to_string())
                    .collect()
            })
            .unwrap_or_default();
        Some(NickBook { pages })
    }

    fn emit_nick_failure(&self, message: &str) {
        self.emitter.publish(BotEvent::NickRollerState {
            bot_id: self.bot_id.clone(),
            phase: NickRollerPhase::Failed,
            message: Some(message.to_owned()),
        });
        self.emit_error("nick_not_ready", message);
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
