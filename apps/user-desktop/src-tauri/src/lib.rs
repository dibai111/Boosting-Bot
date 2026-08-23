mod app_state;
mod auth;
mod bot_runtime;
mod commands;
mod hypixel;
mod input_limits;
mod matchmaking;
mod platform;
mod runtime;

use app_state::AppState;
use bot_runtime::{BotEventBus, BotRuntime};
use commands::file::DroppedFileAccess;
use local_store::Store;
use matchmaking::{MatchmakingDiagnostic, MatchmakingSession};
use platform::shortcuts;
use runtime::UserRuntime;
use std::sync::Arc;
use tauri::{Emitter, Manager};
use tokio::sync::broadcast::error::RecvError;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(setup_app)
        .invoke_handler(tauri::generate_handler![
            commands::account::list_accounts,
            commands::account::add_account,
            commands::account::delete_accounts,
            commands::account::update_server_address,
            commands::bot::start_bot,
            commands::bot::stop_bot,
            commands::matchmaking::start_matchmaking,
            commands::matchmaking::stop_matchmaking,
            commands::matchmaking::get_matchmaking_snapshot,
            commands::overlay::show_matchmaking_overlay,
            commands::overlay::toggle_matchmaking_overlay,
            commands::overlay::hide_matchmaking_overlay,
            commands::shortcut::configure_matchmaking_shortcuts,
            commands::settings::get_user_settings,
            commands::settings::save_user_settings,
            commands::file::open_external_url,
            commands::file::export_session_log,
            commands::file::default_export_log_path,
            commands::file::read_cookie_file,
            commands::system::app_memory_bytes,
            commands::window::set_main_window_layout
        ])
        .on_window_event(|window, event| match event {
            tauri::WindowEvent::DragDrop(tauri::DragDropEvent::Drop { paths, .. })
                if window.label() == "main" =>
            {
                window
                    .state::<DroppedFileAccess>()
                    .authorize_native_drop(window.label(), paths);
            }
            tauri::WindowEvent::CloseRequested { api, .. } => {
                api.prevent_close();
                if window.label() == commands::overlay::MATCHMAKING_OVERLAY_LABEL {
                    let _ = window.hide();
                    return;
                }
                let app = window.app_handle().clone();
                let runtime = app.state::<AppState>().runtime();
                tauri::async_runtime::spawn(async move {
                    runtime.shutdown().await;
                    app.exit(0);
                });
            }
            _ => {}
        })
        .run(tauri::generate_context!())
        .expect("error while running Botting User App");
}

fn setup_app(app: &mut tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    app.manage(DroppedFileAccess::default());
    app.manage(shortcuts::MatchmakingShortcuts);
    shortcuts::start(app.handle().clone())?;

    let bot_events = BotEventBus::new();
    let bots = Arc::new(BotRuntime::new(bot_events.clone()));
    let matchmaking = Arc::new(MatchmakingSession::new(Arc::clone(&bots)));
    let runtime = Arc::new(UserRuntime::new(
        Store::open()?,
        bots,
        bot_events,
        reqwest::Client::new(),
        matchmaking,
    ));

    app.manage(AppState::new(runtime));
    forward_bot_events(app.handle().clone());
    forward_matchmaking_snapshots(app.handle().clone());
    forward_matchmaking_diagnostics(app.handle().clone());
    Ok(())
}

fn forward_bot_events(app: tauri::AppHandle) {
    let mut events = app.state::<AppState>().runtime().subscribe_bot_events();
    tauri::async_runtime::spawn(async move {
        loop {
            let event = match events.recv().await {
                Ok(event) => event,
                Err(RecvError::Lagged(_)) => continue,
                Err(RecvError::Closed) => break,
            };
            app.state::<AppState>()
                .runtime()
                .handle_bot_event(&event)
                .await;
            let _ = app.emit("bot-event", event);
        }
    });
}

fn forward_matchmaking_snapshots(app: tauri::AppHandle) {
    let mut snapshots = app.state::<AppState>().runtime().subscribe_matchmaking();
    tauri::async_runtime::spawn(async move {
        while snapshots.changed().await.is_ok() {
            let _ = app.emit("matchmaking-state", snapshots.borrow().clone());
        }
    });
}

fn forward_matchmaking_diagnostics(app: tauri::AppHandle) {
    let mut diagnostics = app
        .state::<AppState>()
        .runtime()
        .subscribe_matchmaking_diagnostics();
    tauri::async_runtime::spawn(async move {
        loop {
            match diagnostics.recv().await {
                Ok(diagnostic) => {
                    let _ = app.emit("matchmaking-debug", diagnostic);
                }
                Err(RecvError::Lagged(skipped)) => {
                    let _ = app.emit(
                        "matchmaking-debug",
                        MatchmakingDiagnostic {
                            bot_id: None,
                            message: format!("Skipped {skipped} debug messages"),
                        },
                    );
                }
                Err(RecvError::Closed) => break,
            }
        }
    });
}
