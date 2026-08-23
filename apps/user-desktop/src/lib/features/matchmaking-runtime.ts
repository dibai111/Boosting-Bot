import type { UserApi } from "../adapters/api";
import {
  favoriteModeStorageKey,
  modeOptionsForGame,
} from "./matchmaking";
import { matchmakingTransitionLogs } from "../log/matchmaking-log";
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
  api: Pick<
    UserApi,
    | "startMatchmaking"
    | "stopMatchmaking"
    | "showMatchmakingOverlay"
    | "toggleMatchmakingOverlay"
    | "hideMatchmakingOverlay"
  >;
  getState: () => MatchmakingFeatureState;
  patchState: (patch: Partial<MatchmakingFeatureState>) => void;
  addLog: (message: string, botId?: string | null, level?: SessionLogEntry["level"]) => void;
  savePreference: (key: string, value: string) => void;
  showToast: (message: string, kind: "success" | "error") => void;
  translate: (key: string) => string;
}

export function createMatchmakingFeature(deps: MatchmakingFeatureDependencies) {
  function toggleFavorite(value: string): void {
    const next = new Set(deps.getState().favoriteMatchModes);
    if (next.has(value)) next.delete(value);
    else next.add(value);
    deps.patchState({ favoriteMatchModes: next });
    deps.savePreference(favoriteModeStorageKey, JSON.stringify([...next]));
  }

  function togglePresenceVerification(): void {
    const next = !deps.getState().verifyPresence;
    deps.patchState({ verifyPresence: next });
    deps.savePreference("botting-verify-presence", String(next));
  }

  function toggleDuelPitchVerification(): void {
    const next = !deps.getState().verifyDuelPitch;
    deps.patchState({ verifyDuelPitch: next });
    deps.savePreference("botting-verify-duel-pitch", String(next));
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
    const option = modeOptionsForGame(state.matchGame, state.favoriteMatchModes)
      .find((candidate) => candidate.value === value);
    if (option) deps.patchState({ matchMode: option.value });
  }

  function handleMatchmakingState(next: MatchmakingSnapshot): void {
    const previous = deps.getState().matchmaking;
    for (const entry of matchmakingTransitionLogs(previous, next)) {
      deps.addLog(entry.message, entry.botId, entry.level);
    }
    deps.patchState({ matchmaking: next });
    if (previous.phase !== "idle" && next.phase === "idle") {
      void deps.api.hideMatchmakingOverlay();
    }
  }

  async function start(): Promise<void> {
    const state = deps.getState();
    const botIds = state.accounts
      .filter((account) => state.selectedBotIds.has(account.id) && state.phases[account.id] === "online")
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
      deps.savePreference("botting-match-mode", state.matchMode);
      deps.savePreference("botting-player-log-path", logPath);
      deps.savePreference("botting-verify-presence", String(state.verifyPresence));
      deps.savePreference("botting-verify-duel-pitch", String(state.verifyDuelPitch));
      const matchmaking = await deps.api.startMatchmaking({
        mode: state.matchMode,
        bot_ids: botIds,
        bot_usernames: Object.fromEntries(state.accounts
          .filter((account) => botIds.includes(account.id) && account.username.trim())
          .map((account) => [account.id, account.username.trim()])),
        verify_presence: state.verifyPresence,
        verify_duel_pitch: state.verifyDuelPitch,
        required_matches: Math.min(Math.max(1, state.requiredMatches), botIds.length),
        log_path: logPath,
      });
      deps.patchState({ matchmaking });
      deps.addLog(`matchmaking started with ${botIds.length} bots`);
      await deps.api.showMatchmakingOverlay(matchmaking.bots.length);
      deps.patchState({ overlayVisible: true });
    } catch (error) {
      deps.showToast(String(error), "error");
    } finally {
      deps.patchState({ matchmakingBusy: false });
    }
  }

  async function stop(): Promise<void> {
    deps.patchState({ matchmakingBusy: true });
    try {
      const matchmaking = await deps.api.stopMatchmaking();
      deps.patchState({ matchmaking });
      deps.addLog("matchmaking stopped");
      await deps.api.hideMatchmakingOverlay();
      deps.patchState({ overlayVisible: false });
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
    deps.savePreference("botting-player-log-path", path);
    void start();
  }

  async function toggleOverlay(): Promise<void> {
    try {
      const state = deps.getState();
      const overlayVisible = await deps.api.toggleMatchmakingOverlay(
        state.matchmaking.bots.length || state.selectedBotIds.size,
      );
      deps.patchState({ overlayVisible });
    } catch (error) {
      deps.showToast(String(error), "error");
    }
  }

  async function hideOverlay(): Promise<void> {
    await deps.api.hideMatchmakingOverlay();
    deps.patchState({ overlayVisible: false });
  }

  function handleShortcut(action: "stop" | "show_overlay"): void {
    const state = deps.getState();
    if (state.matchmakingBusy) return;
    if (action === "stop") void toggle();
    else if (isMatchActive(state)) void toggleOverlay();
  }

  return {
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
