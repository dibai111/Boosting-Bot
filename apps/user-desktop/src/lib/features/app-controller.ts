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
import { createOverlayFeature, type OverlayDependencies } from "./overlay.ts";
import type { UserApi } from "../adapters/api";
import type { MessageKey } from "../i18n";
import {
  createNickFeature,
  type NickFeatureState,
  type NickFeatureDependencies,
} from "./nick-roller.ts";
import { createBotFeature, type BotFeatureState, type BotFeatureDependencies } from "./bots.ts";
import {
  createAppEntryFeature,
  type AppEntryFeatureState,
  type AppEntryFeatureDependencies,
} from "./app-entry.ts";
import {
  createMatchmakingFeature,
  type MatchmakingFeatureState,
  type MatchmakingFeatureDependencies,
} from "./matchmaking-feature.ts";
import type {
  Account,
  BotPhase,
  BotEvent,
  GameKind,
  GameMode,
  MatchmakingSnapshot,
  RuntimeModeSnapshot,
  SessionLogEntry,
} from "../models/types";

export type UserAppState = AppEntryFeatureState &
  BotFeatureState &
  MatchmakingFeatureState &
  NickFeatureState;

export interface UserAppControllerOptions {
  api: BotFeatureDependencies["api"] &
    AppEntryFeatureDependencies["api"] &
    MatchmakingFeatureDependencies["api"] &
    NickFeatureDependencies["api"] &
    OverlayDependencies["api"] &
    Pick<UserApi, "activeMode">;
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
  savePreferences: (patch: Record<string, string>) => Promise<void>;
  translate: (key: MessageKey) => string;
  onChange: (state: UserAppState) => void;
}

const idleMatchmaking: MatchmakingSnapshot = {
  revision: 0,
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
  let state: UserAppState = {
    nickBots: {},
    nickEvents: [],
    nickBusy: false,
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

  const getState = (): UserAppState => state;
  const patchState = (patch: Partial<UserAppState>): void => {
    state = { ...state, ...patch };
    // 最低配對數只依賴草稿與已選線上帳號；其他事件毋須重新掃描帳號。
    if (patch.accounts || patch.selectedBotIds || patch.phases || "requiredMatches" in patch) {
      const selectedOnlineCount = state.accounts.filter(
        (account) => state.selectedBotIds.has(account.id) && state.phases[account.id] === "online",
      ).length;
      const requestedCount = Number.isFinite(state.requiredMatches)
        ? Math.floor(state.requiredMatches)
        : 1;
      state.requiredMatches = Math.max(1, Math.min(requestedCount, selectedOnlineCount));
    }
    if (patch.accounts) {
      const accountIds = new Set(state.accounts.map((account) => account.id));
      state.nickBots = Object.fromEntries(
        Object.entries(state.nickBots).filter(([id]) => accountIds.has(id)),
      );
    }
    options.onChange(state);
  };

  let runtimeMode: RuntimeModeSnapshot = { revision: -1, mode: "idle", nick_run_id: null };
  const overlay = createOverlayFeature({
    api: options.api,
    onChange: (snapshot) => patchState({ overlayVisible: snapshot.visible }),
  });
  const appEntry = createAppEntryFeature({
    api: options.api,
    getState,
    patchState,
    setWindowMode: options.setWindowMode,
    tick: options.tick,
    appWindowExpandDurationMs: options.appWindowExpandDurationMs,
  });
  const bots = createBotFeature({
    hideOverlay: overlay.hide,
    applyMatchmakingSnapshot: (snapshot) => matchmaking.applySnapshot(snapshot),
    api: options.api,
    getState,
    patchState,
    phaseLabel: options.phaseLabel,
    addLog: options.addLog,
    showToast: options.showToast,
  });
  const matchmaking = createMatchmakingFeature({
    overlay,
    api: options.api,
    getState,
    patchState,
    addLog: options.addLog,
    savePreferences: options.savePreferences,
    showToast: options.showToast,
    translate: options.translate,
  });
  const nick = createNickFeature({
    applyRuntimeMode,
    api: options.api,
    getState,
    patchState,
    translate: options.translate,
    showToast: options.showToast,
  });
  function handleBotEvent(event: BotEvent): void {
    if (event.type === "runtime_mode") {
      applyRuntimeMode(event);
      return;
    }
    nick.handleBotEvent(event);
    bots.handleBotEvent(event);
  }
  function setAccounts(accounts: Account[]): void {
    patchState({
      accounts,
      phases: Object.fromEntries(
        accounts.map((account) => [account.id, state.phases[account.id] ?? "offline"]),
      ),
    });
  }

  function applyRuntimeMode(next: RuntimeModeSnapshot): boolean {
    if (next.revision <= runtimeMode.revision) return false;
    const newNickRun = next.mode === "nick_roller" && next.nick_run_id !== runtimeMode.nick_run_id;
    runtimeMode = next;
    if (newNickRun) nick.beginRun();
    patchState({ activeMode: next.mode });
    return true;
  }
  async function refreshRuntimeMode(): Promise<void> {
    applyRuntimeMode(await options.api.activeMode());
  }
  function handleMatchmakingState(next: MatchmakingSnapshot): void {
    const accepted = matchmaking.handleMatchmakingState(next);
    if (
      accepted &&
      (next.phase === "idle" || next.phase === "failed") &&
      state.activeMode === "matching"
    ) {
      void refreshRuntimeMode().catch((error) => options.showToast(String(error), "error"));
    }
  }

  function toggleBotSelection(botId: string): void {
    const next = new Set(state.selectedBotIds);
    if (next.has(botId)) next.delete(botId);
    else next.add(botId);
    patchState({ selectedBotIds: next });
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
    handleMatchmakingState,
    handleBotEvent,
    startNickRoller: nick.start,
    stopNickRoller: nick.stop,
    answerNickDecision: nick.decide,
    updateMatchmakingInput: matchmaking.updateInput,
    finishAppEntry: appEntry.finishAppEntry,
    hideMatchmakingOverlay: matchmaking.hideOverlay,
    refreshMemory: appEntry.refreshMemory,
    selectMatchGame: matchmaking.selectMatchGame,
    selectMatchMode: matchmaking.selectMatchMode,
    saveServerFor: bots.saveServerFor,
    refreshRuntimeMode,
    setAccounts,
    setDeviceLinkError,
    handleOverlayState: overlay.apply,
    refreshOverlay: overlay.refresh,
    toggleBotSelection,
    setMatchmakingSnapshot: matchmaking.applySnapshot,
    showApp: appEntry.showApp,
    startSelected: bots.startSelected,
    stopMatchmaking: matchmaking.stop,
    stopSelected: bots.stopSelected,
    stopSession: bots.stopSession,
    toggleDuelPitchVerification: matchmaking.toggleDuelPitchVerification,
    toggleFavorite: matchmaking.toggleFavorite,
    toggleMatchmaking: matchmaking.toggle,
    toggleMatchmakingOverlay: matchmaking.toggleOverlay,
    togglePresenceVerification: matchmaking.togglePresenceVerification,
  };
}
