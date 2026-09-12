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

/** 提供遊戲模式選項、收藏排序與設定還原；選項值須對應後端 GameMode。 */
import type { GameKind, GameMode } from "../models/types";

export type ModeOption = { value: GameMode; label: string };
export type ModeGroup = { value: GameKind; label: string; options: ModeOption[] };

export const favoriteModeStorageKey = "botting-favorite-modes";

export const modeGroups: ModeGroup[] = [
  {
    value: "bedwars",
    label: "Bed Wars",
    options: [
      { value: "solo", label: "Solo" },
      { value: "doubles", label: "Doubles" },
      { value: "threes", label: "3v3v3v3" },
      { value: "fours", label: "4v4v4v4" },
      { value: "four_v_four", label: "4v4" },
    ],
  },
  {
    value: "duels",
    label: "Duels",
    options: [
      { value: "duels_uhc", label: "UHC Duel" },
      { value: "duels_classic", label: "Classic Duel" },
      { value: "duels_skywars", label: "SkyWars Duel" },
      { value: "duels_combo", label: "Combo Duel" },
      { value: "duels_bow", label: "Bow Duel" },
      { value: "duels_nodebuff", label: "NoDebuff Duel" },
      { value: "duels_sumo", label: "Sumo Duel" },
      { value: "duels_blitz", label: "Blitz Duel" },
      { value: "duels_op", label: "OP Duel" },
      { value: "duels_mega_walls", label: "MegaWalls Duel" },
      { value: "duels_bow_spleef", label: "Bow Spleef Duel" },
      { value: "duels_bridge", label: "The Bridge Duel" },
      { value: "duels_bedwars", label: "Bed Wars Duel" },
      { value: "duels_bed_rush", label: "Bed Rush Duel" },
      { value: "duels_boxing", label: "Boxing Duel" },
      { value: "duels_quakecraft", label: "Quakecraft Duel" },
    ],
  },
  {
    value: "skywars",
    label: "SkyWars",
    options: [
      { value: "skywars_solo_normal", label: "Solo Normal" },
      { value: "skywars_solo_insane", label: "Solo Insane" },
      { value: "skywars_doubles_normal", label: "Doubles Normal" },
      { value: "skywars_doubles_insane", label: "Doubles Insane" },
      { value: "skywars_solo_lucky", label: "Solo Lucky Blocks" },
      { value: "skywars_doubles_lucky", label: "Doubles Lucky Blocks" },
      { value: "skywars_mini_normal", label: "Mini Normal" },
    ],
  },
];

export const modeOptions = modeGroups.flatMap((group) => group.options);

/**
 * 將傳輸模式對應到遊戲選項群組。
 * @param mode 後端 GameMode 值。
 * @return 遊戲種類；未知值回退至 bedwars。
 */
export function gameKindForMode(mode: GameMode): GameKind {
  return (
    modeGroups.find((group) => group.options.some((option) => option.value === mode))?.value ??
    "bedwars"
  );
}

/**
 * 取得模式副本並將收藏穩定排序到前方。
 * @param game 遊戲種類。
 * @param favorites 已收藏模式的集合。
 * @return 不會修改原始 modeGroups 的選項陣列。
 */
export function modeOptionsForGame(
  game: GameKind,
  favorites: ReadonlySet<string> = new Set(),
): ModeOption[] {
  const options = modeGroups.find((group) => group.value === game)?.options ?? [];
  return [...options].sort(
    (left, right) => Number(favorites.has(right.value)) - Number(favorites.has(left.value)),
  );
}

/**
 * 解析設定中的收藏清單，排除未知模式。
 * @param settings 已讀取的完整偏好快照。
 * @return 有效模式的 Set；損壞設定回傳空集合。
 */
export function loadFavoriteMatchModes(
  settings: Readonly<Record<string, string>> = {},
): Set<string> {
  try {
    const saved = JSON.parse(settings[favoriteModeStorageKey] ?? "[]");
    if (Array.isArray(saved)) {
      return new Set(
        saved.filter((value): value is GameMode =>
          modeOptions.some((option) => option.value === value),
        ),
      );
    }
  } catch {
    // 設定損壞時回復空的收藏清單，讓應用程式仍可開啟。
  }
  return new Set();
}
