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

/** 管理配對啟停、設定及浮動視窗；後端快照是實際配對進度的依據。 */
import type { createOverlayFeature } from "./overlay.ts";
import type { UserApi } from "../adapters/api";
import type { MessageKey } from "../i18n";
import {
  favoriteModeStorageKey,
  modeOptionsForGame,
  gameKindForMode,
} from "./matchmaking-options.ts";
import { matchmakingTransitionLogs } from "../log/matchmaking-log.ts";
import type {
  Account,
  BotPhase,
  GameKind,
  GameMode,
  MatchmakingSnapshot,
  SessionLogEntry,
} from "../models/types";

export interface MatchmakingFeatureState {
  accounts: Account[];
  selectedBotIds: Set<string>;
  phases: Record<string, BotPhase>;
  matchmaking: MatchmakingSnapshot;
  matchMode: GameMode;
  matchGame: GameKind;
  favoriteMatchModes: Set<string>;
  verifyPresence: boolean;
  verifyDuelPitch: boolean;
  requiredMatches: number;
  matchLogPath: string;
  matchmakingBusy: boolean;
  overlayVisible: boolean;
  logPathDialogOpen: boolean;
}

export interface MatchmakingFeatureDependencies {
  api: Pick<UserApi, "startMatchmaking" | "stopMatchmaking">;
  overlay: Pick<ReturnType<typeof createOverlayFeature>, "show" | "hide" | "toggle">;
  getState: () => MatchmakingFeatureState;
  patchState: (patch: Partial<MatchmakingFeatureState>) => void;
  addLog: (message: string, botId?: string | null, level?: SessionLogEntry["level"]) => void;
  savePreferences: (patch: Record<string, string>) => Promise<void>;
  showToast: (message: string, kind: "success" | "error") => void;
  translate: (key: MessageKey) => string;
}

/**
 * 建立配對操作、偏好管理及浮動視窗控制器。
 * @param deps API、最新狀態讀寫與畫面回呼。
 * @return 配對功能介面。
 */
export function createMatchmakingFeature(deps: MatchmakingFeatureDependencies) {
  /** 更新同一份配對草稿；按鈕與快捷鍵都讀取此狀態。 */
  function updateInput(
    patch: Partial<Pick<MatchmakingFeatureState, "requiredMatches" | "matchLogPath">>,
  ): void {
    if (!isMatchActive(deps.getState())) deps.patchState(patch);
  }

  function toggleFavorite(value: string): void {
    const next = new Set(deps.getState().favoriteMatchModes);
    if (next.has(value)) next.delete(value);
    else next.add(value);
    deps.patchState({ favoriteMatchModes: next });
    void deps
      .savePreferences({ [favoriteModeStorageKey]: JSON.stringify([...next]) })
      .catch((error) => deps.showToast(String(error), "error"));
  }

  function togglePresenceVerification(): void {
    const next = !deps.getState().verifyPresence;
    deps.patchState({ verifyPresence: next });
    void deps
      .savePreferences({ "botting-verify-presence": String(next) })
      .catch((error) => deps.showToast(String(error), "error"));
  }

  function toggleDuelPitchVerification(): void {
    const next = !deps.getState().verifyDuelPitch;
    deps.patchState({ verifyDuelPitch: next });
    void deps
      .savePreferences({ "botting-verify-duel-pitch": String(next) })
      .catch((error) => deps.showToast(String(error), "error"));
  }

  function selectMatchGame(game: GameKind): void {
    const state = deps.getState();
    if (isMatchActive(state) || state.matchGame === game) return;
    deps.patchState({
      matchGame: game,
      matchMode: modeOptionsForGame(game, state.favoriteMatchModes)[0]?.value ?? "doubles",
    });
  }

  function selectMatchMode(value: string): void {
    const state = deps.getState();
    if (isMatchActive(state)) return;
    const option = modeOptionsForGame(state.matchGame, state.favoriteMatchModes).find(
      (candidate) => candidate.value === value,
    );
    if (option) deps.patchState({ matchMode: option.value });
  }

  /** 同步初始或後續快照與所屬遊戲，不產生初始載入日誌。 */
  function applySnapshot(next: MatchmakingSnapshot): boolean {
    if (next.revision <= deps.getState().matchmaking.revision) return false;
    deps.patchState({
      matchmaking: next,
      ...(next.mode && next.phase !== "idle"
        ? { matchMode: next.mode, matchGame: gameKindForMode(next.mode) }
        : {}),
    });
    return true;
  }

  /**
   * 比較相鄰快照，更新進度並記錄有意義的轉換。
   * @param next 後端提供的新快照。
   * @return 是否接受這份新快照。
   */
  function handleMatchmakingState(next: MatchmakingSnapshot): boolean {
    const previous = deps.getState().matchmaking;
    if (!applySnapshot(next)) return false;
    for (const entry of matchmakingTransitionLogs(previous, next)) {
      deps.addLog(entry.message, entry.botId, entry.level);
    }
    if (previous.phase !== "idle" && next.phase === "idle") {
      void deps.overlay.hide().catch((error) => deps.showToast(String(error), "error"));
    }
    return true;
  }

  /**
   * 驗證線上 Bot 及日誌路徑後啟動配對並顯示浮動視窗。
   * @return 啟動流程的 Promise；錯誤透過通知顯示。
   */
  async function start(): Promise<void> {
    const state = deps.getState();
    if (state.matchmakingBusy) return;
    const botIds = state.accounts
      .filter(
        (account) => state.selectedBotIds.has(account.id) && state.phases[account.id] === "online",
      )
      .map((account) => account.id);
    if (!botIds.length) {
      deps.showToast(deps.translate("onlineBotRequired"), "error");
      return;
    }
    if (!state.matchLogPath.trim()) {
      deps.patchState({ logPathDialogOpen: true });
      return;
    }

    deps.patchState({ matchmakingBusy: true });
    try {
      const logPath = state.matchLogPath.trim();
      await deps.savePreferences({
        "botting-match-mode": state.matchMode,
        "botting-player-log-path": logPath,
        "botting-verify-presence": String(state.verifyPresence),
        "botting-verify-duel-pitch": String(state.verifyDuelPitch),
      });
      const matchmaking = await deps.api.startMatchmaking({
        mode: state.matchMode,
        bot_ids: botIds,
        verify_presence: state.verifyPresence,
        verify_duel_pitch: state.verifyDuelPitch,
        required_matches: Math.min(Math.max(1, state.requiredMatches), botIds.length),
        log_path: logPath,
      });
      applySnapshot(matchmaking);
      deps.addLog(`matchmaking started with ${botIds.length} bots`);
      const current = deps.getState();
      if (current.matchmaking.session_id === matchmaking.session_id && isMatchActive(current)) {
        await deps.overlay.show(current.matchmaking.bots.length);
      }
    } catch (error) {
      deps.showToast(String(error), "error");
    } finally {
      deps.patchState({ matchmakingBusy: false });
    }
  }

  /**
   * 停止配對、記錄結果並隱藏浮動視窗。
   * @return 停止流程的 Promise；錯誤透過通知顯示。
   */
  async function stop(): Promise<void> {
    if (deps.getState().matchmakingBusy) return;
    deps.patchState({ matchmakingBusy: true });
    try {
      const matchmaking = await deps.api.stopMatchmaking();
      applySnapshot(matchmaking);
      deps.addLog("matchmaking stopped");
      if (!isMatchActive(deps.getState())) {
        await deps.overlay.hide();
      }
    } catch (error) {
      deps.showToast(String(error), "error");
    } finally {
      deps.patchState({ matchmakingBusy: false });
    }
  }

  async function toggle(): Promise<void> {
    if (isMatchActive(deps.getState())) await stop();
    else await start();
  }

  function continueWithLogPath(): void {
    const path = deps.getState().matchLogPath.trim();
    if (!path) return;
    deps.patchState({ matchLogPath: path, logPathDialogOpen: false });
    void start();
  }

  async function toggleOverlay(): Promise<void> {
    try {
      const state = deps.getState();
      await deps.overlay.toggle(state.matchmaking.bots.length || state.selectedBotIds.size);
    } catch (error) {
      deps.showToast(String(error), "error");
    }
  }

  async function hideOverlay(): Promise<void> {
    await deps.overlay.hide();
  }

  /**
   * 忽略忙碌時的快捷鍵，依操作切換配對或浮動視窗。
   * @param action stop 表示配對啟停，show_overlay 表示切換浮動視窗。
   * @return 無回傳值；非同步流程自行顯示錯誤。
   */
  function handleShortcut(action: "stop" | "show_overlay"): void {
    const state = deps.getState();
    if (state.matchmakingBusy) return;
    if (action === "stop") void toggle();
    else if (isMatchActive(state)) void toggleOverlay();
  }

  return {
    applySnapshot,
    updateInput,
    continueWithLogPath,
    handleMatchmakingState,
    handleShortcut,
    hideOverlay,
    selectMatchGame,
    selectMatchMode,
    start,
    stop,
    toggle,
    toggleDuelPitchVerification,
    toggleFavorite,
    toggleOverlay,
    togglePresenceVerification,
  };
}

function isMatchActive(state: MatchmakingFeatureState): boolean {
  return state.matchmaking.phase !== "idle" && state.matchmaking.phase !== "failed";
}
