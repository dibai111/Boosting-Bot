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

/** 管理非同步註冊的事件監聽；即使卸載早於註冊完成，也會立即解除監聽。 */
/**
 * 追蹤非同步監聽註冊，卸載早於註冊完成時立即清理。
 * @return track、dispose 及 isDisposed 生命週期介面。
 */
export function createListenerScope() {
  const listeners = new Set<() => void>();
  let disposed = false;

  /**
   * 接管監聽註冊結果，確保卸載後不留下監聽。
   * @param registration 最終回傳解除監聽函式的 Promise。
   * @return 註冊已納入追蹤或已清理的 Promise。
   */
  async function track(registration: Promise<() => void>): Promise<void> {
    const stop = await registration;
    if (disposed) stop();
    else listeners.add(stop);
  }

  /**
   * 解除所有已註冊監聽並標記作用域已卸載。
   * @return 無回傳值；後續完成的註冊也會被清理。
   */
  function dispose(): void {
    disposed = true;
    for (const stop of listeners) stop();
    listeners.clear();
  }

  return { track, dispose, isDisposed: () => disposed };
}
