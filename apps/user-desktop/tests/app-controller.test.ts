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

// 透過與頁面共用的介面驗證配對表單、快捷鍵與晚到事件。
import assert from "node:assert/strict";
import { test } from "node:test";
import {
  createUserAppController,
  type UserAppState,
  type UserAppControllerOptions,
} from "../src/lib/features/app-controller.ts";
import type { StartMatchmakingInput, MatchmakingSnapshot } from "../src/lib/models/types.ts";
import { defaultNickRollerConfig } from "../src/lib/features/nick-roller-config.ts";

const idle: MatchmakingSnapshot = {
  revision: 0,
  phase: "idle",
  required_matches: 0,
  matched_bots: 0,
  bots: [],
};
function setup(overrides: Partial<UserAppControllerOptions["api"]> = {}) {
  let revision = 0;
  let state!: UserAppState;
  const starts: StartMatchmakingInput[] = [];
  const api: UserAppControllerOptions["api"] = {
    startBot: async () => {},
    stopBot: async () => {},
    updateServer: async () => {},
    deleteAccounts: async (ids) => ids.length,
    appMemoryBytes: async () => 0,
    toggleMatchmakingOverlay: async () => ({ revision: 1, visible: false }),
    overlaySnapshot: async () => ({ revision: 0, visible: false }),
    activeMode: async () => ({ revision: 0, mode: "idle", nick_run_id: null }),
    startNickRoller: async () => ({ revision: 1, mode: "nick_roller", nick_run_id: 1 }),
    stopNickRoller: async () => ({ revision: 2, mode: "idle", nick_run_id: null }),
    answerNickDecision: async () => {},
    startMatchmaking: async (input: StartMatchmakingInput) => {
      starts.push(input);
      return {
        ...idle,
        revision: ++revision,
        phase: "awaiting_player" as const,
        bots: input.bot_ids.map((bot_id) => ({ bot_id, phase: "waiting" as const, attempts: 0 })),
      };
    },
    stopMatchmaking: async () => ({ ...idle, revision: ++revision }),
    showMatchmakingOverlay: async () => ({ revision: 1, visible: true }),
    hideMatchmakingOverlay: async () => ({ revision: 2, visible: false }),
    ...overrides,
  };
  const controller = createUserAppController({
    api,
    appWindowExpandDurationMs: 0,
    initialMatchMode: "doubles",
    initialMatchGame: "bedwars",
    initialFavoriteMatchModes: new Set(),
    initialVerifyPresence: false,
    initialVerifyDuelPitch: false,
    initialMatchLogPath: "old.log",
    setWindowMode: async () => {},
    tick: async () => {},
    phaseLabel: (phase) => phase,
    addLog: () => {},
    showToast: () => {},
    savePreferences: async () => {},
    translate: (key) => key,
    onChange: (next) => {
      state = next;
    },
  });
  controller.setAccounts(
    ["a", "b", "c"].map((id) => ({
      id,
      username: id,
      auth_kind: "microsoft",
      server_address: "mc.hypixel.net",
      created_at: "",
      updated_at: "",
    })),
  );
  for (const bot_id of ["a", "b", "c"]) {
    controller.toggleBotSelection(bot_id);
    controller.handleBotEvent({ type: "status", bot_id, phase: "online" });
  }
  return { controller, state: () => state, starts };
}

test("button and shortcut use the same latest draft and normalized minimum", async () => {
  const app = setup();
  app.controller.updateMatchmakingInput({ requiredMatches: 3, matchLogPath: "latest.log" });
  app.controller.handleMatchmakingShortcut("stop");
  await new Promise<void>((resolve) => setImmediate(resolve));
  assert.equal(app.starts[0].required_matches, 3);
  assert.equal(app.starts[0].log_path, "latest.log");
  await app.controller.stopMatchmaking();
  app.controller.toggleBotSelection("c");
  assert.equal(app.state().requiredMatches, 2);
  await app.controller.toggleMatchmaking();
  assert.equal(app.starts[1].required_matches, 2);
  assert.deepEqual(app.starts[1].bot_ids, ["a", "b"]);
});

test("non-finite inputs normalize and running inputs stay unchanged", async () => {
  const app = setup();
  app.controller.updateMatchmakingInput({ requiredMatches: NaN });
  assert.equal(app.state().requiredMatches, 1);
  app.controller.updateMatchmakingInput({ requiredMatches: 100 });
  assert.equal(app.state().requiredMatches, 3);
  await app.controller.toggleMatchmaking();
  app.controller.updateMatchmakingInput({ matchLogPath: "changed.log", requiredMatches: 1 });
  assert.equal(app.state().matchLogPath, "old.log");
  assert.equal(app.state().requiredMatches, 3);
});

test("initial and event snapshots both synchronize the displayed mode family", () => {
  const app = setup();
  app.controller.setMatchmakingSnapshot({
    ...idle,
    revision: 1,
    phase: "awaiting_player",
    mode: "duels_sumo",
  });
  assert.equal(app.state().matchGame, "duels");
  assert.equal(app.state().matchMode, "duels_sumo");
  app.controller.handleMatchmakingState({
    ...idle,
    revision: 2,
    phase: "awaiting_player",
    mode: "skywars_solo_normal",
  });
  assert.equal(app.state().matchGame, "skywars");
  assert.equal(app.state().matchMode, "skywars_solo_normal");
});

test("late startup and stop replies cannot overwrite a newer event or initial snapshot", async () => {
  let resolveStart!: (snapshot: MatchmakingSnapshot) => void;
  let resolveStop!: (snapshot: MatchmakingSnapshot) => void;
  let overlayShows = 0;
  const app = setup({
    startMatchmaking: () =>
      new Promise((resolve) => {
        resolveStart = resolve;
      }),
    stopMatchmaking: () =>
      new Promise((resolve) => {
        resolveStop = resolve;
      }),
    showMatchmakingOverlay: async () => {
      overlayShows++;
      return { revision: 1, visible: true };
    },
  });
  const pending = app.controller.toggleMatchmaking();
  await new Promise<void>((resolve) => setImmediate(resolve));
  app.controller.handleMatchmakingState({
    ...idle,
    revision: 3,
    session_id: "a",
    phase: "committed",
    matched_bots: 1,
  });
  resolveStart({ ...idle, revision: 1, session_id: "a", phase: "awaiting_player" });
  await pending;
  assert.equal(app.state().matchmaking.phase, "committed");
  assert.equal(app.state().matchmaking.revision, 3);
  app.controller.setMatchmakingSnapshot({
    ...idle,
    revision: 2,
    session_id: "a",
    phase: "matching",
  });
  assert.equal(app.state().matchmaking.phase, "committed");
  const stopping = app.controller.stopMatchmaking();
  app.controller.handleMatchmakingState({
    ...idle,
    revision: 5,
    session_id: "b",
    phase: "matching",
  });
  resolveStop({ ...idle, revision: 4 });
  await stopping;
  assert.equal(app.state().matchmaking.session_id, "b");
  assert.equal(app.state().matchmaking.phase, "matching");
  assert.equal(overlayShows, 1);
});

test("a late start reply cannot reopen an overlay after a terminal event", async () => {
  let resolveStart!: (snapshot: MatchmakingSnapshot) => void;
  let overlayShows = 0;
  const app = setup({
    startMatchmaking: () =>
      new Promise((resolve) => {
        resolveStart = resolve;
      }),
    showMatchmakingOverlay: async () => {
      overlayShows++;
      return { revision: 1, visible: true };
    },
  });
  const pending = app.controller.toggleMatchmaking();
  await new Promise<void>((resolve) => setImmediate(resolve));
  app.controller.handleMatchmakingState({ ...idle, revision: 2, session_id: "a", phase: "failed" });
  resolveStart({ ...idle, revision: 1, session_id: "a", phase: "awaiting_player" });
  await pending;
  assert.equal(app.state().matchmaking.phase, "failed");
  assert.equal(overlayShows, 0);
});

test("stop session and duplicate events obey the same snapshot revision", async () => {
  const app = setup({ stopMatchmaking: async () => ({ ...idle, revision: 1 }) });
  app.controller.handleMatchmakingState({
    ...idle,
    revision: 2,
    phase: "committed",
    session_id: "new",
  });
  app.controller.handleMatchmakingState({ ...idle, revision: 2, phase: "idle" });
  await app.controller.stopSession();
  assert.equal(app.state().matchmaking.phase, "committed");
  assert.equal(app.state().matchmaking.revision, 2);
});

test("late initial mode queries cannot overwrite events or reset current Nick candidates", async () => {
  let resolveMode!: (snapshot: import("../src/lib/models/types.ts").RuntimeModeSnapshot) => void;
  const app = setup({
    activeMode: () =>
      new Promise((resolve) => {
        resolveMode = resolve;
      }),
  });
  const read = app.controller.refreshRuntimeMode();
  app.controller.handleBotEvent({
    type: "runtime_mode",
    revision: 2,
    mode: "nick_roller",
    nick_run_id: 2,
  });
  app.controller.handleBotEvent({
    type: "nick_candidate",
    bot_id: "a",
    candidate_id: 1,
    nick: "Alpha",
    accepted: true,
    reasons: [],
    processed_count: 1,
    accepted_count: 1,
    rejected_count: 0,
  });
  resolveMode({ revision: 1, mode: "idle", nick_run_id: null });
  await read;
  assert.equal(app.state().activeMode, "nick_roller");
  assert.equal(app.state().nickBots.a.candidateId, 1);
  app.controller.handleBotEvent({
    type: "runtime_mode",
    revision: 2,
    mode: "nick_roller",
    nick_run_id: 2,
  });
  assert.equal(app.state().nickBots.a.candidateId, 1);
  app.controller.handleBotEvent({
    type: "runtime_mode",
    revision: 3,
    mode: "idle",
    nick_run_id: null,
  });
  app.controller.handleBotEvent({
    type: "runtime_mode",
    revision: 4,
    mode: "nick_roller",
    nick_run_id: 3,
  });
  assert.equal(app.state().nickBots.a.candidateId, null);
  assert.equal(app.state().nickEvents.length, 0);
});

test("mode query for the current Nick run does not reset it when the matching event arrives", async () => {
  const app = setup({
    activeMode: async () => ({ revision: 2, mode: "nick_roller", nick_run_id: 9 }),
  });
  await app.controller.refreshRuntimeMode();
  app.controller.handleBotEvent({
    type: "nick_candidate",
    bot_id: "a",
    candidate_id: 2,
    nick: "Beta",
    accepted: true,
    reasons: [],
    processed_count: 1,
    accepted_count: 1,
    rejected_count: 0,
  });
  app.controller.handleBotEvent({
    type: "runtime_mode",
    revision: 2,
    mode: "nick_roller",
    nick_run_id: 9,
  });
  assert.equal(app.state().nickBots.a.candidate, "Beta");
});

test("late Nick start and stop replies preserve a newer run and its candidate", async () => {
  type Mode = import("../src/lib/models/types.ts").RuntimeModeSnapshot;
  let resolveStart!: (snapshot: Mode) => void;
  let resolveStop!: (snapshot: Mode) => void;
  const app = setup({
    startNickRoller: () =>
      new Promise((resolve) => {
        resolveStart = resolve;
      }),
    stopNickRoller: () =>
      new Promise((resolve) => {
        resolveStop = resolve;
      }),
  });
  const starting = app.controller.startNickRoller(defaultNickRollerConfig());
  app.controller.handleBotEvent({
    type: "runtime_mode",
    revision: 2,
    mode: "idle",
    nick_run_id: null,
  });
  resolveStart({ revision: 1, mode: "nick_roller", nick_run_id: 1 });
  await starting;
  assert.equal(app.state().activeMode, "idle");

  app.controller.handleBotEvent({
    type: "runtime_mode",
    revision: 3,
    mode: "nick_roller",
    nick_run_id: 2,
  });
  const stopping = app.controller.stopNickRoller();
  app.controller.handleBotEvent({
    type: "runtime_mode",
    revision: 5,
    mode: "nick_roller",
    nick_run_id: 3,
  });
  app.controller.handleBotEvent({
    type: "nick_candidate",
    bot_id: "a",
    candidate_id: 7,
    nick: "Gamma",
    accepted: true,
    reasons: [],
    processed_count: 1,
    accepted_count: 1,
    rejected_count: 0,
  });
  resolveStop({ revision: 4, mode: "idle", nick_run_id: null });
  await stopping;
  assert.equal(app.state().activeMode, "nick_roller");
  assert.equal(app.state().nickBots.a.candidateId, 7);
});
