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

/** 組合功能模組並持有唯一執行狀態，透過 patchState 將變更一次交給頁面。 */
import type { UserApi } from "../adapters/api";
import { createBotFeature, type BotFeatureState } from "./bots";
import { createAppEntryFeature, type AppEntryFeatureState } from "./app-entry";
import { createMatchmakingFeature, type MatchmakingFeatureState } from "./matchmaking-runtime";
import type {
  Account,
  BotPhase,
  GameKind,
  GameMode,
  MatchmakingSnapshot,
  RuntimeMode,
  SessionLogEntry,
} from "../models/types";

type FeatureState = AppEntryFeatureState & BotFeatureState & MatchmakingFeatureState;

type MatchmakingInput = {
  selectedBotIds: Set<string>;
  matchMode: GameMode;
  matchGame: GameKind;
  favoriteMatchModes: Set<string>;
  verifyPresence: boolean;
  verifyDuelPitch: boolean;
  requiredMatches: number;
  matchLogPath: string;
};

export type UserAppState = FeatureState;

export interface UserAppControllerOptions {
  api: UserApi;
  appWindowExpandDurationMs: number;
  initialMatchMode: GameMode;
  initialMatchGame: GameKind;
  initialFavoriteMatchModes: ReadonlySet<string>;
  initialVerifyPresence: boolean;
  initialVerifyDuelPitch: boolean;
  initialMatchLogPath: string;
  setWindowMode: (mode: "app") => Promise<void>;
  tick: () => Promise<void>;
  phaseLabel: (phase: BotPhase) => string;
  addLog: (message: string, botId?: string | null, level?: SessionLogEntry["level"]) => void;
  showToast: (message: string, kind: "success" | "error") => void;
  savePreference: (key: string, value: string) => void;
  translate: (key: string) => string;
  onChange: (state: UserAppState) => void;
}

const idleMatchmaking: MatchmakingSnapshot = {
  phase: "idle",
  required_matches: 0,
  matched_bots: 0,
  bots: [],
};

/**
 * 組合 Bot、配對及入場功能，持有單一可更新狀態。
 * @param options API、初始偏好及畫面通知回呼。
 * @return 供頁面使用的操作與狀態同步介面。
 */
export function createUserAppController(options: UserAppControllerOptions) {
  let state: FeatureState = {
    appEntryPhase: "idle",
    appMemoryBytes: null,
    accounts: [],
    selectedBotIds: new Set<string>(),
    phases: {},
    activeMode: "idle",
    stoppingBotIds: new Set<string>(),
    matchmaking: idleMatchmaking,
    matchmakingBusy: false,
    overlayVisible: false,
    selectedSessionBotId: null,
    deviceCode: null,
    deviceLinkError: "",
    matchMode: options.initialMatchMode,
    matchGame: options.initialMatchGame,
    favoriteMatchModes: new Set(options.initialFavoriteMatchModes),
    verifyPresence: options.initialVerifyPresence,
    verifyDuelPitch: options.initialVerifyDuelPitch,
    requiredMatches: 1,
    matchLogPath: options.initialMatchLogPath,
    logPathDialogOpen: false,
  };

  const getState = (): FeatureState => state;
  const patchState = (patch: Partial<FeatureState>): void => {
    state = { ...state, ...patch };
    options.onChange(state);
  };

  const appEntry = createAppEntryFeature({
    api: options.api,
    getState,
    patchState,
    setWindowMode: options.setWindowMode,
    tick: options.tick,
    appWindowExpandDurationMs: options.appWindowExpandDurationMs,
  });
  const bots = createBotFeature({
    api: options.api,
    getState,
    patchState,
    phaseLabel: options.phaseLabel,
    addLog: options.addLog,
    showToast: options.showToast,
  });
  const matchmaking = createMatchmakingFeature({
    api: options.api,
    getState,
    patchState,
    addLog: options.addLog,
    savePreference: options.savePreference,
    showToast: options.showToast,
    translate: options.translate,
  });
  function setAccounts(accounts: Account[]): void {
    patchState({ accounts });
  }

  function setPhases(phases: Record<string, BotPhase>): void {
    patchState({ phases });
  }

  function setActiveMode(activeMode: RuntimeMode): void {
    patchState({ activeMode });
  }

  function setSelectedBotIds(selectedBotIds: Set<string>): void {
    patchState({ selectedBotIds: new Set(selectedBotIds) });
  }

  /**
   * 將頁面配對輸入一次寫入 controller。
   * @param input 目前選擇、模式及驗證設定。
   * @return 無回傳值；複製 Set 避免共用可變集合。
   */
  function applyMatchmakingInput(input: MatchmakingInput): void {
    patchState({
      selectedBotIds: new Set(input.selectedBotIds),
      matchMode: input.matchMode,
      matchGame: input.matchGame,
      favoriteMatchModes: new Set(input.favoriteMatchModes),
      verifyPresence: input.verifyPresence,
      verifyDuelPitch: input.verifyDuelPitch,
      requiredMatches: input.requiredMatches,
      matchLogPath: input.matchLogPath,
    });
  }

  function setMatchLogPath(matchLogPath: string): void {
    patchState({ matchLogPath });
  }

  function setMatchmakingSnapshot(next: MatchmakingSnapshot): void {
    patchState({ matchmaking: next });
  }

  function setOverlayVisible(overlayVisible: boolean): void {
    patchState({ overlayVisible });
  }

  function clearDeviceCode(): void {
    patchState({ deviceCode: null, deviceLinkError: "" });
  }

  function setDeviceLinkError(deviceLinkError: string): void {
    patchState({ deviceLinkError });
  }

  function closeLogPathDialog(): void {
    patchState({ logPathDialogOpen: false });
  }

  /**
   * 先同步最新頁面輸入，再啟動配對。
   * @param input 本次配對輸入。
   * @return 啟動流程的 Promise。
   */
  async function startMatchmaking(input: MatchmakingInput): Promise<void> {
    applyMatchmakingInput(input);
    await matchmaking.start();
  }

  /**
   * 先同步最新頁面輸入，再切換配對啟停。
   * @param input 本次配對輸入。
   * @return 操作流程的 Promise。
   */
  async function toggleMatchmaking(input: MatchmakingInput): Promise<void> {
    applyMatchmakingInput(input);
    await matchmaking.toggle();
  }

  function continueWithLogPath(matchLogPath: string): void {
    patchState({ matchLogPath });
    matchmaking.continueWithLogPath();
  }

  options.onChange(state);

  return {
    continueWithLogPath,
    closeLogPathDialog,
    clearDeviceCode,
    deleteAccounts: bots.deleteAccounts,
    handleMatchmakingShortcut: matchmaking.handleShortcut,
    handleMatchmakingState: matchmaking.handleMatchmakingState,
    handleBotEvent: bots.handleBotEvent,
    finishAppEntry: appEntry.finishAppEntry,
    hideMatchmakingOverlay: matchmaking.hideOverlay,
    refreshMemory: appEntry.refreshMemory,
    selectMatchGame: matchmaking.selectMatchGame,
    selectMatchMode: matchmaking.selectMatchMode,
    saveServerFor: bots.saveServerFor,
    setActiveMode,
    setAccounts,
    setDeviceLinkError,
    setMatchLogPath,
    setOverlayVisible,
    setPhases,
    setSelectedBotIds,
    setMatchmakingSnapshot,
    showApp: appEntry.showApp,
    startMatchmaking,
    startSelected: bots.startSelected,
    stopAccounts: bots.stopAccounts,
    stopMatchmaking: matchmaking.stop,
    stopSelected: bots.stopSelected,
    stopSession: bots.stopSession,
    toggleDuelPitchVerification: matchmaking.toggleDuelPitchVerification,
    toggleFavorite: matchmaking.toggleFavorite,
    toggleMatchmaking,
    toggleMatchmakingOverlay: matchmaking.toggleOverlay,
    togglePresenceVerification: matchmaking.togglePresenceVerification,
  };
}
