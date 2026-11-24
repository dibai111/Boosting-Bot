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

/** 集中原生視窗查詢、事件與操作回覆的新舊判定。 */
import type { UserApi } from "../adapters/api";
import type { OverlaySnapshot } from "../models/types";
export interface OverlayDependencies {
  api: Pick<
    UserApi,
    | "showMatchmakingOverlay"
    | "hideMatchmakingOverlay"
    | "toggleMatchmakingOverlay"
    | "overlaySnapshot"
  >;
  onChange: (snapshot: OverlaySnapshot) => void;
}
/**
 * 建立應用生命週期內的浮動視窗操作入口。
 * @param deps 原生 adapter 與狀態通知。
 * @return 操作 Promise 在原生執行完成後返回；失敗由呼叫端呈現。
 */
export function createOverlayFeature(deps: OverlayDependencies) {
  let revision = -1;
  function apply(snapshot: OverlaySnapshot): boolean {
    if (snapshot.revision <= revision) return false;
    revision = snapshot.revision;
    deps.onChange(snapshot);
    return true;
  }
  async function show(botCount: number): Promise<void> {
    apply(await deps.api.showMatchmakingOverlay(botCount));
  }
  async function hide(): Promise<void> {
    apply(await deps.api.hideMatchmakingOverlay());
  }
  async function toggle(botCount: number): Promise<void> {
    apply(await deps.api.toggleMatchmakingOverlay(botCount));
  }
  async function refresh(): Promise<void> {
    apply(await deps.api.overlaySnapshot());
  }
  return { apply, show, hide, toggle, refresh };
}
