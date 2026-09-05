import type { UserApi } from "../adapters/api";
import type {
  Account,
  BotPhase,
  MatchmakingSnapshot,
  RuntimeMode,
  SessionLogEntry,
  BotEvent,
} from "../models/types";

const knownBotErrorCodes = new Set([
  "banned",
  "connection_error",
  "kicked",
  "session_panic",
  "start_failed",
]);
const silentStatusPhases: ReadonlySet<BotPhase> = new Set(["starting", "connecting", "stopping"]);

export interface BotFeatureState {
  accounts: Account[];
  selectedBotIds: Set<string>;
  phases: Record<string, BotPhase>;
  activeMode: RuntimeMode;
  stoppingBotIds: Set<string>;
  matchmaking: MatchmakingSnapshot;
  matchmakingBusy: boolean;
  overlayVisible: boolean;
  selectedSessionBotId: string | null;
  deviceCode: { code: string; url: string } | null;
  deviceLinkError: string;
}

export interface BotFeatureDependencies {
  api: Pick<
    UserApi,
    | "startBot"
    | "stopBot"
    | "deleteAccounts"
    | "updateServer"
    | "stopMatchmaking"
    | "hideMatchmakingOverlay"
  >;
  getState: () => BotFeatureState;
  patchState: (patch: Partial<BotFeatureState>) => void;
  phaseLabel: (phase: BotPhase) => string;
  addLog: (message: string, botId?: string | null, level?: SessionLogEntry["level"]) => void;
  showToast: (message: string, kind: "success" | "error") => void;
}

export function createBotFeature(deps: BotFeatureDependencies) {
  const recentErrorMessages = new Map<string, string>();

  async function startSelected(): Promise<void> {
    const state = deps.getState();
    const targets = state.accounts.filter((account) => {
      if (!state.selectedBotIds.has(account.id)) return false;
      const phase = state.phases[account.id] ?? "offline";
      return phase === "offline" || phase === "error";
    });
    if (!targets.length) return;

    deps.patchState({
      stoppingBotIds: new Set(
        [...state.stoppingBotIds].filter((id) => !targets.some((account) => account.id === id)),
      ),
    });
    deps.addLog(`starting ${targets.length} selected bots`);
    const results = await Promise.allSettled(
      targets.map(async (account) => {
        const current = deps.getState();
        deps.patchState({
          phases: { ...current.phases, [account.id]: "starting" },
        });
        deps.addLog(`connecting to ${account.server_address}`, account.id);
        try {
          await deps.api.startBot(account.id);
        } catch (error) {
          const failed = deps.getState();
          deps.patchState({ phases: { ...failed.phases, [account.id]: "error" } });
          deps.addLog(error instanceof Error ? error.message : String(error), account.id, "error");
          throw error;
        }
      }),
    );
    const failed = results.filter((result) => result.status === "rejected").length;
    if (failed) deps.addLog(`${targets.length - failed}/${targets.length} selected bots started`);

  }

  async function stopAccounts(targets: Account[]): Promise<void> {
    if (!targets.length) return;
    const state = deps.getState();
    deps.patchState({
      stoppingBotIds: new Set([...state.stoppingBotIds, ...targets.map((account) => account.id)]),
      phases: {
        ...state.phases,
        ...Object.fromEntries(targets.map((account) => [account.id, "offline" as BotPhase])),
      },
    });
    const results = await Promise.allSettled(targets.map((account) => deps.api.stopBot(account.id)));
    const failed = results.find(
      (result): result is PromiseRejectedResult => result.status === "rejected",
    );
    if (failed) deps.showToast(String(failed.reason), "error");
  }

  async function stopSelected(): Promise<void> {
    const state = deps.getState();
    await stopAccounts(
      state.accounts.filter((account) => {
        if (!state.selectedBotIds.has(account.id)) return false;
        const phase = state.phases[account.id] ?? "offline";
        return phase === "online"
          || phase === "starting"
          || phase === "authenticating"
          || phase === "connecting";
      }),
    );
  }

  async function stopSession(): Promise<void> {
    const state = deps.getState();
    const matchmaking = await deps.api.stopMatchmaking();
    deps.patchState({ matchmaking });
    await deps.api.hideMatchmakingOverlay();
    await stopAccounts(state.accounts.filter((account) => (state.phases[account.id] ?? "offline") !== "offline"));
  }

  async function saveServerFor(account: Account, serverAddress: string): Promise<void> {
    const nextAddress = serverAddress.trim();
    if (!nextAddress || nextAddress === account.server_address) return;
    await deps.api.updateServer(account.id, nextAddress);
    const state = deps.getState();
    deps.patchState({
      accounts: state.accounts.map((item) =>
        item.id === account.id ? { ...item, server_address: nextAddress } : item,
      ),
    });
  }

  async function deleteAccounts(ids: string[]): Promise<void> {
    if (!ids.length) return;
    await deps.api.deleteAccounts(ids);
    const state = deps.getState();
    const removed = new Set(ids);
    deps.patchState({
      accounts: state.accounts.filter((account) => !removed.has(account.id)),
      selectedBotIds: new Set([...state.selectedBotIds].filter((id) => !removed.has(id))),
    });
  }

  function handleBotEvent(event: BotEvent): void {
    const state = deps.getState();
    if (event.type === "status") {
      if (state.stoppingBotIds.has(event.bot_id) && event.phase !== "offline") return;
      if (event.phase === "starting"
        || event.phase === "authenticating"
        || event.phase === "connecting"
        || event.phase === "online") {
        recentErrorMessages.delete(event.bot_id);
      }
      const previousPhase = state.phases[event.bot_id];
      const repeatsVisiblePhase = previousPhase !== undefined
        && previousPhase !== event.phase
        && !event.message
        && deps.phaseLabel(previousPhase) === deps.phaseLabel(event.phase);
      const repeatsError = event.phase === "offline"
        && event.message !== null
        && event.message !== undefined
        && recentErrorMessages.get(event.bot_id) === event.message;
      const stoppingBotIds = event.phase === "offline"
        ? new Set([...state.stoppingBotIds].filter((id) => id !== event.bot_id))
        : state.stoppingBotIds;
      deps.patchState({
        stoppingBotIds,
        phases: { ...state.phases, [event.bot_id]: event.phase },
      });
      if (!silentStatusPhases.has(event.phase) && !repeatsVisiblePhase && !repeatsError) {
        const message = event.message === "Stopped from Botting" ? "Stopped" : event.message || deps.phaseLabel(event.phase);
        deps.addLog(message, event.bot_id, event.phase === "error" ? "error" : "info");
      }
    } else if (event.type === "runtime_mode") {
      deps.patchState({ activeMode: event.mode });
    } else if (event.type === "profile") {
      deps.patchState({
        accounts: state.accounts.map((account) => account.id === event.bot_id
          ? { ...account, username: event.username, credential_checked_at: new Date().toISOString() }
          : account),
      });
    } else if (event.type === "device_code") {
      deps.patchState({ deviceCode: { code: event.user_code, url: event.verification_uri }, deviceLinkError: "" });
    } else if (event.type === "microsoft_auth_result") {
      deps.patchState({ deviceCode: null });
    } else if (event.type === "afk_state") {
      deps.addLog(event.active ? "AFK started" : "AFK stopped", event.bot_id);
    } else if (event.type === "match_attempt_failed") {
      if (!isCommandSpamFailure(event.code, event.message)) {
        deps.addLog(`Matchmaking failed: ${event.message}`, event.bot_id, "error");
      }
    } else if (event.type === "chat_message") {
      deps.addLog(`[chat] ${event.message}`, event.bot_id);
    } else if (event.type === "matchmaking_debug") {
      deps.addLog(`[match debug] ${event.message}`, event.bot_id);
    } else if (event.type === "error") {
      if (event.bot_id && state.stoppingBotIds.has(event.bot_id)) return;
      deps.patchState({
        phases: event.bot_id ? { ...state.phases, [event.bot_id]: "error" } : state.phases,
      });
      const message = botErrorMessage(event.code, event.message);
      if (event.bot_id) recentErrorMessages.set(event.bot_id, message);
      deps.addLog(message, event.bot_id ?? null, "error");
    }
  }

  return {
    deleteAccounts,
    handleBotEvent,
    saveServerFor,
    startSelected,
    stopAccounts,
    stopSelected,
    stopSession,
  };
}

function botErrorMessage(code: string, message: string): string {
  return knownBotErrorCodes.has(code) ? message : `${code}: ${message}`;
}

function isCommandSpamFailure(code: string, message: string): boolean {
  return code === "command_spam"
    || /please don't spam the command|sending commands too fast/i.test(message);
}
