import type { Account, SessionLogEntry } from "../models/types";

export type SessionBotOption = { id: string; username: string };

export function buildSessionBotOptions(
  accountList: Account[],
  logs: SessionLogEntry[],
  labels: Record<string, string>,
  fallbackLabel: string,
): SessionBotOption[] {
  const options = new Map<string, SessionBotOption>();
  for (const account of accountList) {
    options.set(account.id, { id: account.id, username: account.username });
  }
  for (const entry of logs) {
    if (entry.bot_id && !options.has(entry.bot_id)) {
      options.set(entry.bot_id, {
        id: entry.bot_id,
        username: labels[entry.bot_id] ?? entry.bot_label ?? fallbackLabel,
      });
    }
  }
  return [...options.values()];
}

export function formatSessionLog(entry: SessionLogEntry): string {
  return `${entry.timestamp}  ${entry.bot_label ?? "System"}  ${entry.message}`;
}

/** 合併同一個 Bot 連續出現的啟動／停止生命週期訊息，避免 Session Log 顯示重複行。 */
export function mergeLifecycleLogEntries(
  previous: SessionLogEntry | undefined,
  next: SessionLogEntry,
): SessionLogEntry | null {
  if (!previous || previous.bot_id !== next.bot_id || previous.level !== next.level) return null;

  const messages = new Set([previous.message, next.message]);
  let mergedMessage: string | null = null;
  if (messages.has("Online") && messages.has("AFK started")) {
    mergedMessage = "Online · AFK started";
  } else if (messages.has("Stopped") && messages.has("AFK stopped")) {
    mergedMessage = "Stopped · AFK stopped";
  }
  if (!mergedMessage) return null;

  return { ...previous, message: mergedMessage };
}
