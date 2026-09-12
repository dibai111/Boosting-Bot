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

/** 建立記錄篩選選項、匯出文字，並合併相鄰的 Bot 生命週期訊息。 */
import type { Account, SessionLogEntry } from "../models/types";

export type SessionBotOption = { id: string; username: string };

/**
 * 合併現有帳號及歷史日誌的 Bot，避免刪除帳號後失去名稱。
 * @param accountList 目前帳號清單。
 * @param logs 已有的工作階段日誌。
 * @param labels 歷史 Bot 名稱快取。
 * @param fallbackLabel 名稱缺失時使用的文字。
 * @return 按 ID 去重的 Bot 選項。
 */
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

/**
 * 將畫面日誌轉成匯出文字。
 * @param entry 單筆日誌。
 * @return 時間、操作者及訊息組成的一行文字。
 */
export function formatSessionLog(entry: SessionLogEntry): string {
  return `${entry.timestamp}  ${entry.bot_label ?? "System"}  ${entry.message}`;
}

/** 合併同一個 Bot 連續出現的啟動／停止生命週期訊息，避免 Session Log 顯示重複行。 */
/**
 * 合併同一 Bot 相鄰的連線與 AFK 生命週期訊息。
 * @param previous 前一筆日誌。
 * @param next 剛收到的新日誌。
 * @return 合併後的日誌；不符合條件時為 null。
 */
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
