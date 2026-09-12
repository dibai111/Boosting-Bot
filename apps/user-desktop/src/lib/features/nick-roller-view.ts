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

/** 將後端 Nick 事件依時間順序還原成單一 Bot 的畫面狀態。 */
import type { Account, BotEvent, NickRollerPhase, RuntimeMode } from "../models/types";

export type NickBotView = {
  account: Account;
  phase: NickRollerPhase;
  message: string;
  candidateId: number | null;
  candidate: string;
  reasons: string[];
  processed: number;
  matched: number;
  rejected: number;
  decisionDeadline: number | null;
  verification: Extract<BotEvent, { type: "nick_verification" }> | null;
  attention: Extract<BotEvent, { type: "nick_attention" }> | null;
};

/** 每個面板獨立記錄候選期限；事件離開記錄後可由 WeakMap 自動回收。 */
/**
 * 建立保留候選期限的事件投影器，期限快取隨事件回收。
 * @return 將事件投影為單一 Bot 畫面狀態的函式。
 */
export function createNickBotViewBuilder() {
  const candidateDeadlines = new WeakMap<BotEvent, number>();
  /**
   * 按事件順序還原候選、計數及驗證畫面。
   * @param account 目標帳號。
   * @param botEvents 依時間排序且保留物件身分的事件陣列。
   * @param mode 目前後端工作模式。
   * @return 只包含此 Bot 的畫面狀態。
   */
  function buildBotView(account: Account, botEvents: BotEvent[], mode: RuntimeMode): NickBotView {
    const view: NickBotView = {
      account,
      phase: mode === "nick_roller" ? "preparing" : "idle",
      message: "",
      candidateId: null,
      candidate: "",
      reasons: [],
      processed: 0,
      matched: 0,
      rejected: 0,
      decisionDeadline: null,
      verification: null,
      attention: null,
    };

    for (const event of botEvents) {
      if (!("bot_id" in event) || event.bot_id !== account.id) continue;
      if (event.type === "nick_roller_state") {
        view.phase = event.phase;
        view.message = event.message ?? "";
        // 離開決策階段後，舊候選不能繼續顯示接受／跳過操作。
        if (event.phase !== "awaiting_decision") {
          view.decisionDeadline = null;
          view.candidateId = null;
          view.candidate = "";
        }
        if (event.phase === "preparing") {
          view.processed = 0;
          view.matched = 0;
          view.rejected = 0;
          view.reasons = [];
          view.verification = null;
          view.attention = null;
        }
      } else if (event.type === "nick_candidate") {
        view.processed = event.processed_count;
        view.matched = event.accepted_count;
        view.rejected = event.rejected_count;
        view.reasons = event.reasons;
        if (event.accepted) {
          view.phase = "awaiting_decision";
          view.candidateId = event.candidate_id;
          view.candidate = event.nick;
          // 重播相同事件時保留原期限，避免其他事件或選取操作延長倒數。
          if (event.decision_timeout_ms && !candidateDeadlines.has(event)) {
            candidateDeadlines.set(event, Date.now() + event.decision_timeout_ms);
          }
          view.decisionDeadline = candidateDeadlines.get(event) ?? null;
        }
      } else if (event.type === "nick_verification") {
        view.verification = event;
        view.phase = event.success ? "finished" : "failed";
        view.candidateId = null;
        view.candidate = "";
        view.decisionDeadline = null;
      } else if (event.type === "nick_attention") {
        view.attention = event;
      }
    }
    return view;
  }

  return buildBotView;
}
