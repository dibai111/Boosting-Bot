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

/** 依序執行視窗調整、畫面展開和入場動畫；序號用來忽略被取代的非同步流程。 */
import { waitForViewportLayout } from "../../../../shared-ui/viewport-fit";
import type { UserApi } from "../adapters/api";

export type AppEntryPhase = "idle" | "resizing" | "preparing" | "expanding" | "playing";

export interface AppEntryFeatureState {
  appEntryPhase: AppEntryPhase;
  appMemoryBytes: number | null;
}

export interface AppEntryFeatureDependencies {
  api: Pick<UserApi, "appMemoryBytes">;
  getState: () => AppEntryFeatureState;
  patchState: (patch: Partial<AppEntryFeatureState>) => void;
  setWindowMode: (mode: "app") => Promise<void>;
  tick: () => Promise<void>;
  appWindowExpandDurationMs: number;
}

function waitForMotion(duration: number): Promise<void> {
  if (duration <= 0 || window.matchMedia("(prefers-reduced-motion: reduce)").matches) {
    return Promise.resolve();
  }
  return new Promise((resolve) => window.setTimeout(resolve, duration));
}

/**
 * 協調主視窗調整、入場動畫與記憶體讀取。
 * @param deps API、畫面同步及動畫時間設定。
 * @return 入場與記憶體操作介面。
 */
export function createAppEntryFeature(deps: AppEntryFeatureDependencies) {
  let entrySequence = 0;

  /**
   * 按順序展開主視窗，新的入場流程會使舊流程失效。
   * @return 入場流程的 Promise。
   */
  async function showApp(): Promise<void> {
    const sequence = ++entrySequence;
    try {
      deps.patchState({ appEntryPhase: "resizing" });
      await deps.tick();
      await waitForViewportLayout();
      if (sequence !== entrySequence) return;

      const expandDuration = window.matchMedia("(prefers-reduced-motion: reduce)").matches
        ? 0
        : deps.appWindowExpandDurationMs;
      await deps.setWindowMode("app");
      if (sequence !== entrySequence) return;

      deps.patchState({ appEntryPhase: "preparing" });
      await deps.tick();
      await waitForViewportLayout();
      if (sequence !== entrySequence) return;

      deps.patchState({ appEntryPhase: expandDuration > 0 ? "expanding" : "playing" });
      await deps.tick();
      await waitForMotion(expandDuration);
      if (sequence !== entrySequence) return;

      deps.patchState({ appEntryPhase: "playing" });
      if (expandDuration === 0) finishAppEntry();
    } catch {
      if (sequence === entrySequence) deps.patchState({ appEntryPhase: "idle" });
    }
  }

  function finishAppEntry(): void {
    if (deps.getState().appEntryPhase === "playing") {
      deps.patchState({ appEntryPhase: "idle" });
    }
  }

  /**
   * 讀取後端記憶體數值，失敗時顯示未知狀態。
   * @return 讀取與畫面同步的 Promise。
   */
  async function refreshMemory(): Promise<void> {
    const appMemoryBytes = await deps.api.appMemoryBytes().catch(() => null);
    deps.patchState({ appMemoryBytes });
  }

  return { finishAppEntry, refreshMemory, showApp };
}
