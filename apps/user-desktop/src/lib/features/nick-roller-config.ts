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

/** 驗證儲存的 Nick 設定，將損壞欄位還原為預設值並維持後端接受的範圍。 */
import type { NickRollerConfig } from "../models/types";

/**
 * 建立獨立的設定物件，避免不同面板共用可變的規則陣列。
 * @return 包含篩選規則及本機音效偏好的預設設定。
 */
export function defaultNickRollerConfig(): NickRollerConfig {
  return {
    book_timeout_ms: 2_000,
    next_roll_delay_ms: 1_000,
    stop_after_found: false,
    show_rejected: false,
    rules: {
      case_sensitive: false,
      exact_length: null,
      min_length: 1,
      max_length: 8,
      allow_numbers: true,
      allow_underscore: true,
      allow_list: [],
      starts_with: [],
      ends_with: [],
      contains: [],
      contains_match_mode: "any",
      starts_with_priority: false,
      ends_with_priority: false,
      contains_priority: false,
      legacy_priority: true,
    },
    sound: { enabled: true, volume: 85 },
  };
}

/**
 * 從 JSON 還原設定，逐欄驗證型別，避免任意儲存值進入表單或 IPC。
 * @param value Registry 設定快照內的 JSON 字串；空字串表示尚未設定。
 * @return 合法設定；未知欄位忽略，無效欄位使用預設值。
 */
export function parseNickRollerConfig(value: string): NickRollerConfig {
  const config = defaultNickRollerConfig();
  let saved: Record<string, unknown>;
  try {
    saved = asRecord(JSON.parse(value));
  } catch {
    return config;
  }
  const rules = asRecord(saved.rules);
  const sound = asRecord(saved.sound);

  config.book_timeout_ms = boundedInteger(
    saved.book_timeout_ms,
    500,
    30_000,
    config.book_timeout_ms,
  );
  config.next_roll_delay_ms = boundedInteger(
    saved.next_roll_delay_ms,
    0,
    60_000,
    config.next_roll_delay_ms,
  );
  for (const key of ["stop_after_found", "show_rejected"] as const) {
    if (typeof saved[key] === "boolean") config[key] = saved[key];
  }
  for (const key of [
    "case_sensitive",
    "allow_numbers",
    "allow_underscore",
    "starts_with_priority",
    "ends_with_priority",
    "contains_priority",
    "legacy_priority",
  ] as const) {
    if (typeof rules[key] === "boolean") config.rules[key] = rules[key];
  }
  for (const key of ["exact_length", "min_length", "max_length"] as const) {
    // null 表示停用長度規則，不能被預設長度覆蓋。
    config.rules[key] =
      rules[key] === null ? null : boundedInteger(rules[key], 1, 16, config.rules[key]);
  }
  for (const key of ["allow_list", "starts_with", "ends_with", "contains"] as const) {
    const values = rules[key];
    if (Array.isArray(values)) {
      config.rules[key] = values
        .filter((entry): entry is string => typeof entry === "string")
        .map((entry) => entry.trim())
        .filter(Boolean)
        .slice(0, 256);
    }
  }
  if (rules.contains_match_mode === "any" || rules.contains_match_mode === "all") {
    config.rules.contains_match_mode = rules.contains_match_mode;
  }
  if (typeof sound.enabled === "boolean") config.sound.enabled = sound.enabled;
  config.sound.volume = boundedInteger(sound.volume, 0, 100, config.sound.volume);
  return config;
}

/**
 * 將未知 JSON 值縮窄為可逐欄讀取的物件。
 * @param value JSON 解碼後的任意值。
 * @return 原物件；null、陣列與純量均回傳空物件。
 */
function asRecord(value: unknown): Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value)
    ? (value as Record<string, unknown>)
    : {};
}

/**
 * 將有限數值取整並限制在閉區間；不將字串或布林值強制轉成數字。
 * @param value 待驗證的數值。
 * @param min 允許的最小值。
 * @param max 允許的最大值。
 * @param fallback 型別不符或數值非有限時使用的預設值。
 * @return 區間內的整數，或傳入的預設值。
 */
function boundedInteger<T extends number | null>(
  value: unknown,
  min: number,
  max: number,
  fallback: T,
): number | T {
  return typeof value === "number" && Number.isFinite(value)
    ? Math.max(min, Math.min(max, Math.round(value)))
    : fallback;
}
