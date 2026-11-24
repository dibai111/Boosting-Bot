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

// 透過功能介面驗證持續狀態、候選期限與操作，不依賴面板生命週期。
import assert from "node:assert/strict";
import { test } from "node:test";
import { createNickFeature, type NickFeatureState } from "../src/lib/features/nick-roller.ts";
import type { BotEvent, NickRollerEngineConfig } from "../src/lib/models/types.ts";

const account = {
  id: "bot-a",
  username: "Alpha",
  auth_kind: "microsoft" as const,
  server_address: "mc.hypixel.net",
  created_at: "",
  updated_at: "",
};
const candidate = (): Extract<BotEvent, { type: "nick_candidate" }> => ({
  type: "nick_candidate",
  bot_id: account.id,
  candidate_id: 1,
  nick: "AquaVtx",
  accepted: true,
  reasons: [],
  processed_count: 2,
  accepted_count: 1,
  rejected_count: 1,
  decision_timeout_ms: 100_000,
});
function setup() {
  let state: NickFeatureState = {
    accounts: [account],
    phases: { [account.id]: "online" },
    selectedBotIds: new Set([account.id]),
    activeMode: "nick_roller",
    nickBots: {},
    nickEvents: [],
    nickBusy: false,
  };
  let now = 1_000;
  const decisions: unknown[] = [];
  const starts: unknown[] = [];
  const feature = createNickFeature({
    applyRuntimeMode: (snapshot) => {
      state = { ...state, activeMode: snapshot.mode };
      return true;
    },
    api: {
      startNickRoller: async (input) => {
        starts.push(input);
        return { revision: 1, mode: "nick_roller", nick_run_id: 1 };
      },
      stopNickRoller: async () => ({ revision: 2, mode: "idle", nick_run_id: null }),
      answerNickDecision: async (...args) => {
        decisions.push(args);
      },
    },
    getState: () => state,
    patchState: (patch) => {
      state = { ...state, ...patch };
    },
    translate: (key) => key,
    showToast: () => {},
    now: () => now,
  });
  return {
    feature,
    state: () => state,
    setTime: (value: number) => {
      now = value;
    },
    patch: (value: Partial<NickFeatureState>) => {
      state = { ...state, ...value };
    },
    decisions,
    starts,
  };
}

test("truncating other bots' history preserves a pending candidate and its deadline", () => {
  const app = setup();
  app.feature.handleBotEvent(candidate());
  app.setTime(50_000);
  for (let i = 0; i < 250; i++)
    app.feature.handleBotEvent({ type: "nick_roller_state", bot_id: "bot-b", phase: "rolling" });
  assert.equal(app.state().nickEvents.length, 200);
  assert.equal(app.state().nickBots[account.id].phase, "awaiting_decision");
  assert.equal(app.state().nickBots[account.id].candidateId, 1);
  assert.equal(app.state().nickBots[account.id].decisionDeadline, 101_000);
});

test("unselected candidates use receive time and a replay does not extend their deadline", () => {
  const app = setup();
  app.patch({ selectedBotIds: new Set() });
  app.feature.handleBotEvent(candidate());
  app.setTime(50_000);
  app.patch({ selectedBotIds: new Set([account.id]) });
  app.feature.handleBotEvent(candidate());
  assert.equal(app.state().nickBots[account.id].decisionDeadline, 101_000);
});

test("terminal phases clear candidate actions", async () => {
  for (const phase of ["stopped", "failed", "finished", "verifying"] as const) {
    const app = setup();
    app.feature.handleBotEvent(candidate());
    app.feature.handleBotEvent({ type: "nick_roller_state", bot_id: account.id, phase });
    await app.feature.decide(account.id, true);
    assert.equal(app.state().nickBots[account.id].candidateId, null);
    assert.equal(app.state().nickBots[account.id].decisionDeadline, null);
    assert.equal(app.decisions.length, 0);
  }
});

test("verification preserves counts and a new run clears previous results", () => {
  const app = setup();
  app.feature.handleBotEvent(candidate());
  app.feature.handleBotEvent({
    type: "nick_verification",
    bot_id: account.id,
    candidate_id: 1,
    expected_nick: "AquaVtx",
    success: true,
    reason: "matched",
    processed_count: 2,
  });
  assert.equal(app.state().nickBots[account.id].phase, "finished");
  assert.equal(app.state().nickBots[account.id].processed, 2);
  assert.equal(app.state().nickBots[account.id].rejected, 1);
  assert.equal(app.state().nickBots[account.id].candidateId, null);
  app.feature.beginRun();
  assert.equal(app.state().nickBots[account.id].verification, null);
  assert.equal(app.state().nickBots[account.id].processed, 0);
  assert.equal(app.state().nickEvents.length, 0);
  app.setTime(50_000);
  app.feature.handleBotEvent(candidate());
  assert.equal(app.state().nickBots[account.id].decisionDeadline, 150_000);
});

test("decisions use the current candidate and cannot be sent in another mode", async () => {
  const app = setup();
  app.feature.handleBotEvent(candidate());
  await app.feature.decide(account.id, true);
  assert.deepEqual(app.decisions, [[account.id, 1, true]]);
  app.patch({ activeMode: "matching" });
  await app.feature.decide(account.id, false);
  assert.equal(app.decisions.length, 1);
});

test("Nick start rejects a conflicting mode and sends only selected online bots", async () => {
  const app = setup();
  const config = {
    rules: { exact_length: null, min_length: null, max_length: null },
  } as NickRollerEngineConfig;
  await app.feature.start(config);
  assert.equal(app.starts.length, 0);
  app.patch({ activeMode: "idle" });
  await app.feature.start(config);
  assert.deepEqual(app.starts, [{ bot_ids: [account.id], config }]);
  assert.equal(app.state().nickBusy, false);
});
