import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type {
  Account,
  BotEvent,
  CreateAccountInput,
  MatchmakingDiagnostic,
  MatchmakingSnapshot,
  StartMatchmakingInput,
} from "../models/types";

export type UserSettings = Record<string, string>;

// 正式 desktop build 只透過 Tauri command 存取本機資料與 bot runtime。
export const api = {
  async loadSettings(): Promise<UserSettings> {
    return invoke("get_user_settings");
  },

  async saveSettings(settings: UserSettings): Promise<void> {
    return invoke("save_user_settings", { settings });
  },

  async listAccounts(): Promise<Account[]> {
    return invoke("list_accounts");
  },

  async addAccount(input: CreateAccountInput): Promise<Account> {
    return invoke("add_account", { input });
  },

  async deleteAccounts(ids: string[]): Promise<number> {
    return invoke("delete_accounts", { ids });
  },

  async updateServer(id: string, serverAddress: string): Promise<void> {
    return invoke("update_server_address", { id, serverAddress });
  },

  async startBot(id: string): Promise<void> {
    return invoke("start_bot", { id });
  },

  async stopBot(id: string): Promise<void> {
    return invoke("stop_bot", { id });
  },

  async matchmakingSnapshot(): Promise<MatchmakingSnapshot> {
    return invoke("get_matchmaking_snapshot");
  },

  async startMatchmaking(input: StartMatchmakingInput): Promise<MatchmakingSnapshot> {
    return invoke("start_matchmaking", { input });
  },

  async stopMatchmaking(): Promise<MatchmakingSnapshot> {
    return invoke("stop_matchmaking");
  },

  async showMatchmakingOverlay(botCount: number): Promise<void> {
    return invoke("show_matchmaking_overlay", { botCount });
  },

  async toggleMatchmakingOverlay(botCount: number): Promise<boolean> {
    return invoke("toggle_matchmaking_overlay", { botCount });
  },

  async hideMatchmakingOverlay(): Promise<void> {
    return invoke("hide_matchmaking_overlay");
  },

  async configureMatchmakingShortcuts(stopShortcut: string, showOverlayShortcut: string): Promise<void> {
    return invoke("configure_matchmaking_shortcuts", { stopShortcut, showOverlayShortcut });
  },

  async openExternal(url: string): Promise<void> {
    return invoke("open_external_url", { url });
  },

  async exportLog(contents: string, folder: string): Promise<string> {
    return invoke("export_session_log", { contents, folder });
  },

  async defaultExportLogPath(): Promise<string> {
    return invoke("default_export_log_path");
  },

  async readCookieFile(path: string): Promise<string> {
    return invoke("read_cookie_file", { path });
  },

  async appMemoryBytes(): Promise<number | null> {
    return invoke("app_memory_bytes");
  },

  async onBotEvent(handler: (event: BotEvent) => void): Promise<UnlistenFn> {
    return listen<BotEvent>("bot-event", ({ payload }) => handler(payload));
  },

  async onMatchmakingState(handler: (snapshot: MatchmakingSnapshot) => void): Promise<UnlistenFn> {
    return listen<MatchmakingSnapshot>("matchmaking-state", ({ payload }) => handler(payload));
  },

  async onMatchmakingDebug(handler: (diagnostic: MatchmakingDiagnostic) => void): Promise<UnlistenFn> {
    return listen<MatchmakingDiagnostic>("matchmaking-debug", ({ payload }) => handler(payload));
  },

  async onMatchmakingShortcut(handler: (action: "stop" | "show_overlay") => void): Promise<UnlistenFn> {
    return listen<"stop" | "show_overlay">("matchmaking-shortcut", ({ payload }) => handler(payload));
  },

  async onMatchmakingOverlayVisibility(handler: (visible: boolean) => void): Promise<UnlistenFn> {
    return listen<boolean>("matchmaking-overlay-visibility", ({ payload }) => handler(payload));
  },

};

export type UserApi = typeof api;
