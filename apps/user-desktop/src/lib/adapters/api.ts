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

/** 集中定義 Tauri 指令與事件介面；傳輸欄位須與 Rust 的序列化名稱一致。 */
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type {
  Account,
  BotEvent,
  CreateAccountInput,
  MatchmakingDiagnostic,
  MatchmakingSnapshot,
  StartMatchmakingInput,
  RuntimeMode,
  StartNickRollerInput,
} from "../models/types";

export type UserSettings = Record<string, string>;

// 正式 desktop build 只透過 Tauri command 存取本機資料與 bot runtime。
export const api = {
  /**
   * 讀取使用者設定快照。
   * @return 設定鍵值表的 Promise。
   */
  async loadSettings(): Promise<UserSettings> {
    return invoke("get_user_settings");
  },

  /**
   * 將完整設定交給後端驗證並保存。
   * @param settings 完整設定快照。
   * @return 儲存完成的 Promise；失敗會拒絕。
   */
  async saveSettings(settings: UserSettings): Promise<void> {
    return invoke("save_user_settings", { settings });
  },

  /**
   * 取得不含登入憑據的帳號清單。
   * @return 帳號清單的 Promise。
   */
  async listAccounts(): Promise<Account[]> {
    return invoke("list_accounts");
  },

  /**
   * 啟動登入驗證並保存帳號。
   * @param input 帳號登入方式、憑據及伺服器。
   * @return 新帳號的 Promise。
   */
  async addAccount(input: CreateAccountInput): Promise<Account> {
    return invoke("add_account", { input });
  },

  /**
   * 停止 Bot 並移除指定帳號。
   * @param ids 本機帳號 ID 清單。
   * @return 實際刪除數量的 Promise。
   */
  async deleteAccounts(ids: string[]): Promise<number> {
    return invoke("delete_accounts", { ids });
  },

  /**
   * 更新帳號的伺服器地址。
   * @param id 本機帳號 ID。
   * @param serverAddress 目標伺服器地址。
   * @return 更新完成的 Promise。
   */
  async updateServer(id: string, serverAddress: string): Promise<void> {
    return invoke("update_server_address", { id, serverAddress });
  },

  /**
   * 要求後端啟動單一 Bot。
   * @param id 本機帳號 ID。
   * @return 請求結果的 Promise；連線進度由事件回報。
   */
  async startBot(id: string): Promise<void> {
    return invoke("start_bot", { id });
  },

  /**
   * 要求後端停止單一 Bot。
   * @param id 本機帳號 ID。
   * @return 停止完成的 Promise。
   */
  async stopBot(id: string): Promise<void> {
    return invoke("stop_bot", { id });
  },

  /**
   * 讀取目前配對快照。
   * @return 配對快照的 Promise。
   */
  async matchmakingSnapshot(): Promise<MatchmakingSnapshot> {
    return invoke("get_matchmaking_snapshot");
  },

  /**
   * 以選取的 Bot 及日誌路徑啟動配對。
   * @param input 配對模式、驗證策略及最低數量。
   * @return 啟動快照的 Promise。
   */
  async startMatchmaking(input: StartMatchmakingInput): Promise<MatchmakingSnapshot> {
    return invoke("start_matchmaking", { input });
  },

  /**
   * 停止配對並撤回參與 Bot。
   * @return 停止快照的 Promise。
   */
  async stopMatchmaking(): Promise<MatchmakingSnapshot> {
    return invoke("stop_matchmaking");
  },

  /**
   * 啟動所選 Bot 的 Nick 篩選。
   * @param input Bot ID 與後端篩選設定。
   * @return 執行模式的 Promise。
   */
  async startNickRoller(input: StartNickRollerInput): Promise<RuntimeMode> {
    return invoke("start_nick_roller", { input });
  },

  /**
   * 停止後端追蹤的 Nick 篩選。
   * @return 停止請求的 Promise。
   */
  async stopNickRoller(): Promise<void> {
    return invoke("stop_nick_roller");
  },

  /**
   * 對事件中的候選送出接受或跳過決策。
   * @param botId 候選所屬 Bot。
   * @param candidateId 候選事件編號。
   * @param take 是否接受。
   * @return 發送結果的 Promise；過期候選由後端忽略。
   */
  async answerNickDecision(botId: string, candidateId: number, take: boolean): Promise<void> {
    return invoke("answer_nick_decision", { botId, candidateId, take });
  },

  /**
   * 取得校正後的互斥工作模式。
   * @return 目前模式的 Promise。
   */
  async activeMode(): Promise<RuntimeMode> {
    return invoke("get_active_mode");
  },

  /**
   * 顯示配對浮動視窗。
   * @param botCount 決定浮動視窗高度的 Bot 數。
   * @return 視窗操作的 Promise。
   */
  async showMatchmakingOverlay(botCount: number): Promise<void> {
    return invoke("show_matchmaking_overlay", { botCount });
  },

  /**
   * 切換配對浮動視窗。
   * @param botCount 顯示時採用的 Bot 數。
   * @return 切換後可見狀態的 Promise。
   */
  async toggleMatchmakingOverlay(botCount: number): Promise<boolean> {
    return invoke("toggle_matchmaking_overlay", { botCount });
  },

  /**
   * 隱藏配對浮動視窗。
   * @return 隱藏完成的 Promise。
   */
  async hideMatchmakingOverlay(): Promise<void> {
    return invoke("hide_matchmaking_overlay");
  },

  /**
   * 套用全域配對快捷鍵。
   * @param stopShortcut 配對啟停按鍵字串。
   * @param showOverlayShortcut 浮動視窗切換按鍵字串。
   * @return 設定結果的 Promise。
   */
  async configureMatchmakingShortcuts(
    stopShortcut: string,
    showOverlayShortcut: string,
  ): Promise<void> {
    return invoke("configure_matchmaking_shortcuts", { stopShortcut, showOverlayShortcut });
  },

  /**
   * 透過後端允許清單開啟登入網址。
   * @param url Microsoft HTTPS 驗證網址。
   * @return 開啟結果的 Promise。
   */
  async openExternal(url: string): Promise<void> {
    return invoke("open_external_url", { url });
  },

  /**
   * 匯出目前日誌至唯一檔案。
   * @param contents 待匯出的文字。
   * @param folder 絕對目錄；空字串使用 Downloads。
   * @return 完整輸出路徑的 Promise。
   */
  async exportLog(contents: string, folder: string): Promise<string> {
    return invoke("export_session_log", { contents, folder });
  },

  /**
   * 取得預設日誌匯出目錄。
   * @return Downloads 路徑的 Promise。
   */
  async defaultExportLogPath(): Promise<string> {
    return invoke("default_export_log_path");
  },

  /**
   * 讀取由原生拖放單次授權的 Cookie 檔案。
   * @param path 最近拖放的絕對路徑。
   * @return 檔案文字的 Promise。
   */
  async readCookieFile(path: string): Promise<string> {
    return invoke("read_cookie_file", { path });
  },

  /**
   * 讀取主程序及子程序的專用工作集。
   * @return 記憶體位元組數的 Promise。
   */
  async appMemoryBytes(): Promise<number | null> {
    return invoke("app_memory_bytes");
  },

  /**
   * 訂閱Bot 狀態與業務事件。
   * @param handler 接收Bot 狀態與業務事件的回呼。
   * @return 解除監聽函式的 Promise；呼叫端須在卸載時清理。
   */
  async onBotEvent(handler: (event: BotEvent) => void): Promise<UnlistenFn> {
    return listen<BotEvent>("bot-event", ({ payload }) => handler(payload));
  },

  /**
   * 訂閱完整配對快照。
   * @param handler 接收完整配對快照的回呼。
   * @return 解除監聽函式的 Promise；呼叫端須在卸載時清理。
   */
  async onMatchmakingState(handler: (snapshot: MatchmakingSnapshot) => void): Promise<UnlistenFn> {
    return listen<MatchmakingSnapshot>("matchmaking-state", ({ payload }) => handler(payload));
  },

  /**
   * 訂閱配對診斷。
   * @param handler 接收配對診斷的回呼。
   * @return 解除監聽函式的 Promise；呼叫端須在卸載時清理。
   */
  async onMatchmakingDebug(
    handler: (diagnostic: MatchmakingDiagnostic) => void,
  ): Promise<UnlistenFn> {
    return listen<MatchmakingDiagnostic>("matchmaking-debug", ({ payload }) => handler(payload));
  },

  /**
   * 訂閱配對快捷鍵操作。
   * @param handler 接收配對快捷鍵操作的回呼。
   * @return 解除監聽函式的 Promise；呼叫端須在卸載時清理。
   */
  async onMatchmakingShortcut(
    handler: (action: "stop" | "show_overlay") => void,
  ): Promise<UnlistenFn> {
    return listen<"stop" | "show_overlay">("matchmaking-shortcut", ({ payload }) =>
      handler(payload),
    );
  },

  /**
   * 訂閱浮動視窗可見狀態。
   * @param handler 接收浮動視窗可見狀態的回呼。
   * @return 解除監聽函式的 Promise；呼叫端須在卸載時清理。
   */
  async onMatchmakingOverlayVisibility(handler: (visible: boolean) => void): Promise<UnlistenFn> {
    return listen<boolean>("matchmaking-overlay-visibility", ({ payload }) => handler(payload));
  },
};

export type UserApi = typeof api;
