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

/** 持有 Nick 目前狀態、活動紀錄與非同步操作，畫面只傳入使用者意圖。 */
import type { UserApi } from "../adapters/api";
import type { MessageKey } from "../i18n";
import type {
  Account,
  BotEvent,
  BotPhase,
  NickRollerEngineConfig,
  RuntimeMode,
  RuntimeModeSnapshot,
} from "../models/types";
import {
  emptyNickBotState,
  projectNickEvent,
  type NickBotState,
  type NickEvent,
} from "./nick-roller-view.ts";

export interface NickFeatureState {
  accounts: Account[];
  phases: Record<string, BotPhase>;
  selectedBotIds: Set<string>;
  activeMode: RuntimeMode;
  nickBots: Record<string, NickBotState>;
  nickEvents: NickEvent[];
  nickBusy: boolean;
}
export interface NickFeatureDependencies {
  api: Pick<UserApi, "startNickRoller" | "stopNickRoller" | "answerNickDecision">;
  applyRuntimeMode: (snapshot: RuntimeModeSnapshot) => boolean;
  getState: () => NickFeatureState;
  patchState: (patch: Partial<NickFeatureState>) => void;
  translate: (key: MessageKey) => string;
  showToast: (message: string, kind: "success" | "error") => void;
  now?: () => number;
}

/**
 * 建立持續存在於應用生命週期內的 Nick 功能。
 * @param deps Bot 操作、狀態讀寫、通知與可替換的 Unix 毫秒時鐘。
 * @return 事件處理、啟停及目前候選決策的介面。
 */
export function createNickFeature(deps: NickFeatureDependencies) {
  const now = deps.now ?? Date.now;
  /** 只在確認新 Nick 執行時清空狀態，重複或晚到模式快照不會觸發。 */
  function beginRun(): void {
    const state = deps.getState();
    deps.patchState({
      nickBots: Object.fromEntries(
        [...state.selectedBotIds].map((id) => [id, emptyNickBotState("preparing")]),
      ),
      nickEvents: [],
    });
  }
  /** 接收新事件並保留每個 Bot 的目前狀態；歷史只保存最近 200 筆。 */
  function handleBotEvent(event: BotEvent): void {
    if (
      event.type === "nick_roller_state" ||
      event.type === "nick_candidate" ||
      event.type === "nick_verification" ||
      event.type === "nick_attention"
    ) {
      const state = deps.getState();
      deps.patchState({
        nickBots: {
          ...state.nickBots,
          [event.bot_id]: projectNickEvent(state.nickBots[event.bot_id], event, now()),
        },
        nickEvents: [...state.nickEvents.slice(-199), event],
      });
    }
  }
  /** 驗證目前選取與規則後啟動，重複請求在忙碌期間忽略。 */
  async function start(config: NickRollerEngineConfig): Promise<void> {
    const state = deps.getState();
    if (state.nickBusy) return;
    const botIds = state.accounts
      .filter(
        (account) => state.selectedBotIds.has(account.id) && state.phases[account.id] === "online",
      )
      .map((account) => account.id);
    let errorKey: MessageKey | null = null;
    if (state.activeMode !== "idle") errorKey = "nickRollerRunningLock";
    else if (!botIds.length) errorKey = "nickRollerNoOnlineBots";
    else if (
      config.rules.exact_length !== null &&
      (config.rules.min_length !== null || config.rules.max_length !== null)
    )
      errorKey = "nickRollerExactLength";
    if (errorKey) {
      deps.showToast(deps.translate(errorKey), "error");
      return;
    }
    deps.patchState({ nickBusy: true });
    try {
      deps.applyRuntimeMode(
        await deps.api.startNickRoller({
          bot_ids: botIds,
          config,
        }),
      );
      deps.showToast(deps.translate("nickRollerStarted"), "success");
    } catch (error) {
      const message = String(error);
      deps.showToast(
        message.includes("mc.hypixel.net") ? deps.translate("nickRollerHypixelOnly") : message,
        "error",
      );
    } finally {
      deps.patchState({ nickBusy: false });
    }
  }
  async function stop(): Promise<void> {
    if (deps.getState().nickBusy) return;
    deps.patchState({ nickBusy: true });
    try {
      deps.applyRuntimeMode(await deps.api.stopNickRoller());
      deps.showToast(deps.translate("nickRollerStopped"), "success");
    } catch (error) {
      deps.showToast(String(error), "error");
    } finally {
      deps.patchState({ nickBusy: false });
    }
  }
  /** 使用目前狀態的候選編號，避免畫面傳入已過期的候選快照。 */
  async function decide(botId: string, take: boolean): Promise<void> {
    const state = deps.getState();
    const bot = state.nickBots[botId];
    if (state.activeMode !== "nick_roller" || !bot || bot.candidateId === null) return;
    try {
      await deps.api.answerNickDecision(botId, bot.candidateId, take);
    } catch (error) {
      deps.showToast(String(error), "error");
    }
  }
  return { beginRun, handleBotEvent, start, stop, decide };
}
