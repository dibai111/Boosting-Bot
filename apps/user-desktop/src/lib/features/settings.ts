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

/** 設定草稿的初始值、寫入方式與變更通知。 */
export interface SettingsDependencies {
  initialSettings: Record<string, string>;
  saveSettings: (snapshot: Record<string, string>) => Promise<void>;
  onChange: (settings: Record<string, string>) => void;
}
/**
 * 建立依呼叫順序儲存設定的功能。
 * @param deps 初始設定、寫入方式與草稿更新通知。
 * @return 更新部分設定的介面。每次寫入獨立回報結果，失敗時保留草稿供重試。
 */
export function createSettingsFeature(deps: SettingsDependencies) {
  let draft = { ...deps.initialSettings };
  let writes = Promise.resolve();
  function save(patch: Record<string, string>): Promise<void> {
    draft = { ...draft, ...patch };
    const snapshot = { ...draft };
    const result = writes.then(() => deps.saveSettings(snapshot));
    // 先排好這次寫入，草稿通知即使同步觸發另一次儲存，也會依序執行。
    writes = result.catch(() => undefined);
    deps.onChange({ ...draft });
    return result;
  }
  return { save };
}
