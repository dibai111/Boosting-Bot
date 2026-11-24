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

/** 按事件增量更新 Nick 狀態；目前狀態與可截斷的活動紀錄各自保存。 */
import type { Account, BotEvent, NickRollerPhase } from "../models/types";

export type NickBotState = {
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

export type NickBotView = NickBotState & { account: Account };
export type NickEvent = Extract<
  BotEvent,
  { type: "nick_roller_state" | "nick_candidate" | "nick_verification" | "nick_attention" }
>;

/** 建立尚無候選的獨立 Bot 狀態。 */
export function emptyNickBotState(phase: NickRollerPhase = "idle"): NickBotState {
  return {
    phase,
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
}

/** 只處理新事件；receivedAt 為接收時間的 Unix 毫秒，不因重繪或面板重掛而改變。 */
export function projectNickEvent(
  previous: NickBotState | undefined,
  event: NickEvent,
  receivedAt: number,
): NickBotState {
  const view = { ...(previous ?? emptyNickBotState()) };
  if (event.type === "nick_roller_state") {
    if (event.phase === "preparing")
      return { ...emptyNickBotState("preparing"), message: event.message ?? "" };
    view.phase = event.phase;
    view.message = event.message ?? "";
    if (event.phase !== "awaiting_decision") {
      view.candidateId = null;
      view.candidate = "";
      view.decisionDeadline = null;
    }
  } else if (event.type === "nick_candidate") {
    view.processed = event.processed_count;
    view.matched = event.accepted_count;
    view.rejected = event.rejected_count;
    view.reasons = event.reasons;
    if (event.accepted) {
      const sameCandidate =
        view.phase === "awaiting_decision" && view.candidateId === event.candidate_id;
      view.phase = "awaiting_decision";
      view.candidateId = event.candidate_id;
      view.candidate = event.nick;
      if (!sameCandidate)
        view.decisionDeadline = event.decision_timeout_ms
          ? receivedAt + event.decision_timeout_ms
          : null;
    }
  } else if (event.type === "nick_verification") {
    view.verification = event;
    view.phase = event.success ? "finished" : "failed";
    view.processed = event.processed_count;
    view.candidateId = null;
    view.candidate = "";
    view.decisionDeadline = null;
  } else view.attention = event;
  return view;
}
