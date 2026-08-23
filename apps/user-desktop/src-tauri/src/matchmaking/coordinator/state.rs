use super::super::{
    detector::duel_pitch,
    modes::{bedwars, duels::QueueObservation},
};
use super::super::{MatchmakingPlan, MatchmakingSnapshot};
use std::collections::{HashMap, HashSet};
use std::time::Instant;

/// 配對期間的所有可變資料，集中管理避免 Coordinator 持有零散狀態。
pub(crate) struct SessionState {
    pub(crate) snapshot: MatchmakingSnapshot,
    pub(crate) plan: Option<MatchmakingPlan>,
    pub(crate) target_generation: u64,
    pub(crate) active_attempts: HashMap<String, String>,
    pub(crate) retry_requests: HashMap<String, PendingRetry>,
    pub(crate) round_started_at: Option<Instant>,
    pub(crate) player_queue: Option<QueueObservation>,
    pub(crate) player_bedwars_queue: Option<bedwars::QueueObservation>,
    pub(crate) player_queue_usernames: HashSet<String>,
    pub(crate) bot_queues: HashMap<String, QueueObservation>,
    pub(crate) bot_bedwars_queues: HashMap<String, bedwars::QueueObservation>,
    pub(crate) bot_transfers: HashSet<String>,
    pub(crate) presence_checks: HashMap<String, PresenceCheck>,
    pub(crate) duel_pitch_checks: HashMap<String, duel_pitch::VerificationCheck>,
    pub(crate) duel_pitch_observations: HashMap<String, duel_pitch::Observation>,
}

pub(crate) struct PendingRetry {
    pub(crate) request_id: String,
    pub(crate) generation: u64,
    pub(crate) retry_delay: std::time::Duration,
}

pub(crate) struct PresenceCheck {
    pub(crate) username: String,
    pub(crate) generation: u64,
    pub(crate) attempt_id: String,
    pub(crate) server: String,
}

impl SessionState {
    pub(crate) fn new(snapshot: MatchmakingSnapshot) -> Self {
        Self {
            snapshot,
            plan: None,
            target_generation: 0,
            active_attempts: HashMap::new(),
            retry_requests: HashMap::new(),
            round_started_at: None,
            player_queue: None,
            player_bedwars_queue: None,
            player_queue_usernames: HashSet::new(),
            bot_queues: HashMap::new(),
            bot_bedwars_queues: HashMap::new(),
            bot_transfers: HashSet::new(),
            presence_checks: HashMap::new(),
            duel_pitch_checks: HashMap::new(),
            duel_pitch_observations: HashMap::new(),
        }
    }

    pub(crate) fn reset_round(&mut self) {
        self.clear_round_tracking();
        // 舊 worker event 不可以在新 round 重新寫入目前 session。
        self.target_generation = self.target_generation.wrapping_add(1);
    }
    pub(crate) fn clear_round_tracking(&mut self) {
        // 每輪觀察資料必須完全隔離，否則上一輪結果可能令新一輪誤判為成功。
        self.active_attempts.clear();
        self.retry_requests.clear();
        self.round_started_at = None;
        self.player_queue = None;
        self.player_bedwars_queue = None;
        self.player_queue_usernames.clear();
        self.bot_queues.clear();
        self.bot_bedwars_queues.clear();
        self.bot_transfers.clear();
        self.presence_checks.clear();
        self.duel_pitch_checks.clear();
        self.duel_pitch_observations.clear();
    }

    pub(crate) fn clear_bot_attempt_tracking(&mut self, bot_id: &str) {
        self.bot_queues.remove(bot_id);
        self.bot_bedwars_queues.remove(bot_id);
        self.bot_transfers.remove(bot_id);
        self.presence_checks.remove(bot_id);
        self.duel_pitch_checks.remove(bot_id);
        self.duel_pitch_observations.remove(bot_id);
    }
}
