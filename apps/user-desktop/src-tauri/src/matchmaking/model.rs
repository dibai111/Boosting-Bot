use crate::bot_runtime::GameMode;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum MatchmakingPhase {
    Idle,
    AwaitingPlayer,
    Matching,
    Committed,
    InGame,
    Failed,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum BotMatchPhase {
    Waiting,
    Queued,
    Matched,
    Returning,
    Afk,
    Unavailable,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct StartMatchmakingInput {
    pub(crate) mode: GameMode,
    pub(crate) bot_ids: Vec<String>,
    #[serde(default = "default_verify_presence")]
    pub(crate) verify_presence: bool,
    #[serde(default = "default_verify_duel_pitch")]
    pub(crate) verify_duel_pitch: bool,
    pub(crate) required_matches: usize,
    pub(crate) log_path: String,
}

fn default_verify_presence() -> bool {
    true
}

fn default_verify_duel_pitch() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct MatchmakingBotState {
    pub(crate) bot_id: String,
    pub(crate) phase: BotMatchPhase,
    pub(crate) server: Option<String>,
    pub(crate) attempts: u32,
    pub(crate) message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct MatchmakingSnapshot {
    pub(crate) session_id: Option<String>,
    pub(crate) round_id: Option<String>,
    pub(crate) phase: MatchmakingPhase,
    pub(crate) mode: Option<GameMode>,
    pub(crate) player_server: Option<String>,
    pub(crate) required_matches: usize,
    pub(crate) matched_bots: usize,
    pub(crate) bots: Vec<MatchmakingBotState>,
    pub(crate) message: Option<String>,
}

impl MatchmakingSnapshot {
    pub(crate) fn idle() -> Self {
        Self {
            session_id: None,
            round_id: None,
            phase: MatchmakingPhase::Idle,
            mode: None,
            player_server: None,
            required_matches: 0,
            matched_bots: 0,
            bots: Vec::new(),
            message: None,
        }
    }
}
