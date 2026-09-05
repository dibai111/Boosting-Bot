export type AuthKind = "microsoft" | "access_token" | "cookie";
export type BotPhase = "offline" | "starting" | "authenticating" | "connecting" | "online" | "stopping" | "error";
export type RuntimeMode = "idle" | "matching" | "nick_roller";
export type NickRollerPhase = "idle" | "preparing" | "rolling" | "awaiting_decision" | "verifying" | "finished" | "failed" | "stopped";
export type NickContainsMatchMode = "any" | "all";
export type GameKind = "bedwars" | "duels" | "skywars";
export type GameMode =
  | "solo"
  | "doubles"
  | "threes"
  | "fours"
  | "four_v_four"
  | "duels_uhc"
  | "duels_classic"
  | "duels_skywars"
  | "duels_combo"
  | "duels_bow"
  | "duels_nodebuff"
  | "duels_sumo"
  | "duels_blitz"
  | "duels_op"
  | "duels_mega_walls"
  | "duels_bow_spleef"
  | "duels_bridge"
  | "duels_bedwars"
  | "duels_bed_rush"
  | "duels_boxing"
  | "duels_quakecraft"
  | "skywars_solo_normal"
  | "skywars_solo_insane"
  | "skywars_doubles_normal"
  | "skywars_doubles_insane"
  | "skywars_solo_lucky"
  | "skywars_doubles_lucky"
  | "skywars_mini_normal";
export type MatchmakingPhase = "idle" | "awaiting_player" | "matching" | "committed" | "in_game" | "failed";
export type BotMatchPhase = "waiting" | "queued" | "matched" | "returning" | "afk" | "unavailable";

export interface Account {
  id: string;
  username: string;
  profile_id?: string | null;
  auth_kind: AuthKind;
  session_expires_at?: string | null;
  credential_checked_at?: string | null;
  server_address: string;
  created_at: string;
  updated_at: string;
}

export interface CreateAccountInput {
  username: string;
  auth_kind: AuthKind;
  credential?: string | null;
  server_address: string;
}

export interface SessionLogEntry {
  id: number;
  timestamp: string;
  bot_id: string | null;
  bot_label: string | null;
  level: "info" | "error";
  message: string;
}

export interface StartMatchmakingInput {
  mode: GameMode;
  bot_ids: string[];
  bot_usernames: Record<string, string>;
  verify_presence: boolean;
  verify_duel_pitch: boolean;
  required_matches: number;
  log_path: string;
}

export interface NickRules {
  case_sensitive: boolean;
  exact_length: number | null;
  min_length: number | null;
  max_length: number | null;
  allow_numbers: boolean;
  allow_underscore: boolean;
  allow_list: string[];
  starts_with: string[];
  ends_with: string[];
  contains: string[];
  contains_match_mode: NickContainsMatchMode;
  starts_with_priority: boolean;
  ends_with_priority: boolean;
  contains_priority: boolean;
  legacy_priority: boolean;
}

export interface NickRollerConfig {
  book_timeout_ms: number;
  next_roll_delay_ms: number;
  stop_after_found: boolean;
  show_rejected: boolean;
  rules: NickRules;
  sound: {
    enabled: boolean;
    volume: number;
  };
}

export type NickRollerEngineConfig = Omit<NickRollerConfig, "sound" | "show_rejected">;

export interface StartNickRollerInput {
  bot_ids: string[];
  config: NickRollerEngineConfig;
}

export interface MatchmakingBotState {
  bot_id: string;
  phase: BotMatchPhase;
  server?: string | null;
  attempts: number;
  message?: string | null;
}

export interface MatchmakingSnapshot {
  session_id?: string | null;
  round_id?: string | null;
  phase: MatchmakingPhase;
  mode?: GameMode | null;
  player_server?: string | null;
  required_matches: number;
  matched_bots: number;
  bots: MatchmakingBotState[];
  message?: string | null;
}

export interface MatchmakingDiagnostic {
  bot_id?: string | null;
  message: string;
}

export type BotEvent =
  | { type: "runtime_mode"; mode: RuntimeMode }
  | { type: "status"; bot_id: string; phase: BotPhase; message?: string | null }
  | { type: "profile"; bot_id: string; username: string; uuid?: string | null }
  | { type: "device_code"; request_id: string; user_code: string; verification_uri: string; expires_in?: number | null }
  | { type: "microsoft_auth_result"; request_id: string; username?: string | null; uuid?: string | null; error?: string | null }
  | { type: "nick_roller_state"; bot_id: string; phase: NickRollerPhase; message?: string | null }
  | { type: "nick_candidate"; bot_id: string; candidate_id: number; nick: string; accepted: boolean; reasons: string[]; processed_count: number; accepted_count: number; rejected_count: number; decision_timeout_ms?: number | null }
  | { type: "nick_verification"; bot_id: string; candidate_id: number; expected_nick: string; actual_nick?: string | null; success: boolean; reason: string; processed_count: number }
  | { type: "nick_attention"; bot_id: string; code: string; message: string }
  | { type: "afk_state"; bot_id: string; active: boolean }
  | { type: "match_attempt_result"; bot_id: string; session_id: string; round_id: string; target_generation: number; attempt_id: string; server: string }
  | { type: "match_attempt_failed"; bot_id: string; session_id: string; round_id: string; target_generation: number; attempt_id: string; code: string; message: string }
  | { type: "match_retry_ready"; bot_id: string; request_id: string }
  | { type: "match_retry_preparation_failed"; bot_id: string; request_id: string; message: string }
  | { type: "queue_progress"; bot_id: string; current: number; total: number }
  | { type: "duel_pitch_observed"; bot_id: string; round_id: string; target_generation: number; attempt_id: string; direction: "up" | "down"; pitch: number }
  | { type: "matchmaking_debug"; bot_id: string; message: string }
  | { type: "bot_game_state"; bot_id: string; round_id: string; target_generation: number; state: "started" | "ended" }
  | { type: "chat_message"; bot_id: string; message: string }
  | { type: "error"; bot_id?: string | null; code: string; message: string };
