import type { UserApi } from "../adapters/api";
import {
  createBotFeature,
  type BotFeatureState,
} from "./bots";
import { createAppEntryFeature, type AppEntryFeatureState } from "./app-entry";
import {
  createMatchmakingFeature,
  type MatchmakingFeatureState,
} from "./matchmaking-runtime";
import type {
  Account,
  BotPhase,
  GameKind,
  GameMode,
  MatchmakingSnapshot,
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

export function createUserAppController(options: UserAppControllerOptions) {
  let state: FeatureState = {
    appEntryPhase: "idle",
    appMemoryBytes: null,
    accounts: [],
    selectedBotIds: new Set<string>(),
    phases: {},
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

  function setSelectedBotIds(selectedBotIds: Set<string>): void {
    patchState({ selectedBotIds: new Set(selectedBotIds) });
  }

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

  async function startMatchmaking(input: MatchmakingInput): Promise<void> {
    applyMatchmakingInput(input);
    await matchmaking.start();
  }

  async function toggleMatchmaking(input: MatchmakingInput): Promise<void> {
    applyMatchmakingInput(input);
    await matchmaking.toggle();
  }

  function continueWithLogPath(matchLogPath: string): void {
    patchState({ matchLogPath });
    matchmaking.continueWithLogPath();
  }

  function selectMatchGame(game: GameKind): void {
    matchmaking.selectMatchGame(game);
  }

  function selectMatchMode(mode: string): void {
    matchmaking.selectMatchMode(mode);
  }

  function toggleFavorite(mode: string): void {
    matchmaking.toggleFavorite(mode);
  }

  function togglePresenceVerification(): void {
    matchmaking.togglePresenceVerification();
  }

  function toggleDuelPitchVerification(): void {
    matchmaking.toggleDuelPitchVerification();
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
    selectMatchGame,
    selectMatchMode,
    saveServerFor: bots.saveServerFor,
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
    toggleDuelPitchVerification,
    toggleFavorite,
    toggleMatchmaking,
    toggleMatchmakingOverlay: matchmaking.toggleOverlay,
    togglePresenceVerification,
  };
}
