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

export function gameKindForMode(mode: GameMode): GameKind {
  return modeGroups.find((group) => group.options.some((option) => option.value === mode))?.value ?? "bedwars";
}

export function modeOptionsForGame(game: GameKind, favorites: ReadonlySet<string> = new Set()): ModeOption[] {
  const options = modeGroups.find((group) => group.value === game)?.options ?? [];
  return [...options].sort(
    (left, right) => Number(favorites.has(right.value)) - Number(favorites.has(left.value)),
  );
}

export function loadFavoriteMatchModes(settings: Readonly<Record<string, string>> = {}): Set<string> {
  try {
    const saved = JSON.parse(settings[favoriteModeStorageKey] ?? "[]");
    if (Array.isArray(saved)) {
      return new Set(saved.filter((value): value is GameMode => modeOptions.some((option) => option.value === value)));
    }
  } catch {
    // Invalid local settings should not prevent the app from opening.
  }
  return new Set();
}
