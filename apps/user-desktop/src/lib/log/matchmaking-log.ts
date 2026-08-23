import type { MatchmakingBotState, MatchmakingSnapshot, SessionLogEntry } from "../models/types";

export type MatchmakingLogMessage = {
  botId: string | null;
  message: string;
  level?: SessionLogEntry["level"];
};

export function matchmakingTransitionLogs(
  previous: MatchmakingSnapshot,
  next: MatchmakingSnapshot,
): MatchmakingLogMessage[] {
  const logs: MatchmakingLogMessage[] = [];

  if (next.player_server && next.player_server !== previous.player_server) {
    logs.push({ botId: null, message: `player server detected: ${next.player_server}` });
  }

  if (next.phase !== previous.phase) {
    if (next.phase === "committed") {
      logs.push({ botId: null, message: `minimum reached: ${next.matched_bots}/${next.required_matches} bots matched` });
    } else if (next.phase === "in_game") {
      logs.push({ botId: null, message: "game started" });
    } else if (next.phase === "failed") {
      logs.push({ botId: null, message: `matchmaking stopped: ${next.message ?? "round failed"}`, level: "error" });
    }
  }

  const previousBots = new Map(previous.bots.map((bot) => [bot.bot_id, bot]));
  for (const bot of next.bots) {
    appendBotTransition(logs, previousBots.get(bot.bot_id), bot, next);
  }
  return logs;
}

function appendBotTransition(
  logs: MatchmakingLogMessage[],
  previous: MatchmakingBotState | undefined,
  next: MatchmakingBotState,
  snapshot: MatchmakingSnapshot,
): void {
  const phaseChanged = previous?.phase !== next.phase;
  const messageChanged = previous?.message !== next.message;

  if (
    matchesRetryPhase(next.phase)
    && (!next.message || next.message.includes("; retrying in "))
  ) {
    return;
  } else if (next.phase === "returning" && (phaseChanged || messageChanged)) {
    if (next.message?.startsWith("Limbo ready; retrying")) {
      return;
    } else if (
      next.server
      && snapshot.player_server
      && next.server.toLowerCase() === snapshot.player_server.toLowerCase()
    ) {
      logs.push({ botId: next.bot_id, message: `detected server ${next.server}` });
      logs.push({ botId: next.bot_id, message: lowerFirst(next.message ?? "Verification failed; entering limbo") });
    } else if (next.server && snapshot.player_server) {
      logs.push({
        botId: next.bot_id,
        message: `detected server ${next.server}; does not match ${snapshot.player_server}; entering limbo`,
      });
    } else {
      logs.push({ botId: next.bot_id, message: lowerFirst(next.message ?? "Attempt failed; entering limbo") });
    }
  } else if (next.phase === "matched" && phaseChanged) {
    const server = next.server ?? snapshot.player_server;
    logs.push({
      botId: next.bot_id,
      message: server
        ? `detected server ${server}; matched player server ${server}`
        : "matched player server",
    });
  } else if (next.phase === "unavailable" && (phaseChanged || messageChanged)) {
    logs.push({ botId: next.bot_id, message: `bot unavailable: ${next.message ?? "unknown error"}`, level: "error" });
  } else if (next.phase === "afk" && phaseChanged) {
    logs.push({ botId: next.bot_id, message: "matched game started" });
  }
}

function matchesRetryPhase(phase: MatchmakingBotState["phase"]): boolean {
  return phase === "queued" || phase === "returning";
}

function lowerFirst(value: string): string {
  return value.charAt(0).toLowerCase() + value.slice(1);
}
