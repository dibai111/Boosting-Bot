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

/** 將鍵盤事件與儲存的快捷鍵轉成顯示文字；實際合法性由後端解析器決定。 */
/**
 * 將按鍵事件中啟用的修飾鍵轉為後端名稱。
 * @param event 具有 Ctrl、Alt、Shift、Meta 狀態的鍵盤事件。
 * @return 依固定順序排列的修飾鍵名稱。
 */
export function shortcutModifiers(
  event: Pick<KeyboardEvent, "ctrlKey" | "altKey" | "shiftKey" | "metaKey">,
): string[] {
  return [
    event.ctrlKey ? "control" : "",
    event.altKey ? "alt" : "",
    event.shiftKey ? "shift" : "",
    event.metaKey ? "super" : "",
  ].filter(Boolean);
}

/**
 * 正規化常見修飾鍵寫法，供 UI 提前比對衝突。
 * @param shortcut 已儲存或剛錄製的快捷鍵。
 * @return 小寫且不含空白的比較字串；最終合法性仍由後端決定。
 */
export function shortcutIdentity(shortcut: string): string {
  return shortcut.toLowerCase().replaceAll("ctrl", "control").replaceAll(" ", "");
}

/**
 * 將快捷鍵字串轉成可見按鍵名稱。
 * @param shortcut 後端接受的按鍵字串。
 * @return 以加號分隔的顯示文字。
 */
export function shortcutLabel(shortcut: string): string {
  const names: Record<string, string> = {
    control: "Ctrl",
    alt: "Alt",
    shift: "Shift",
    super: "Win",
  };
  return shortcut
    .split("+")
    .map((part) => {
      if (names[part.toLowerCase()]) return names[part.toLowerCase()];
      if (part.startsWith("Key")) return part.slice(3);
      if (part.startsWith("Digit")) return part.slice(5);
      return part;
    })
    .join(" + ");
}
