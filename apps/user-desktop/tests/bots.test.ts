// 驗證停止失敗、事件交錯及刪除帳號後的前端狀態，避免 UI 與實際 Bot 脫節。
import assert from "node:assert/strict";
import { test } from "node:test";
import { createBotFeature, type BotFeatureState } from "../src/lib/features/bots.ts";
import type { Account } from "../src/lib/models/types.ts";

const account: Account = {
  id: "bot-a",
  username: "Alpha",
  auth_kind: "microsoft",
  server_address: "mc.hypixel.net",
  created_at: "",
  updated_at: "",
};

function setup(stopBot: (id: string) => Promise<void>) {
  let state: BotFeatureState = {
    accounts: [account],
    selectedBotIds: new Set([account.id]),
    phases: { [account.id]: "online" },
    activeMode: "idle",
    stoppingBotIds: new Set(),
    matchmaking: { phase: "idle", required_matches: 0, matched_bots: 0, bots: [] },
    matchmakingBusy: false,
    overlayVisible: false,
    selectedSessionBotId: account.id,
    deviceCode: null,
    deviceLinkError: "",
  };
  const errors: string[] = [];
  const feature = createBotFeature({
    api: {
      startBot: async () => undefined,
      stopBot,
      deleteAccounts: async () => 1,
      updateServer: async () => undefined,
      stopMatchmaking: async () => state.matchmaking,
      hideMatchmakingOverlay: async () => undefined,
    },
    getState: () => state,
    patchState: (patch) => {
      state = { ...state, ...patch };
    },
    phaseLabel: (phase) => phase,
    addLog: () => undefined,
    showToast: (message) => {
      errors.push(message);
    },
  });
  return { feature, state: () => state, errors };
}

test("failed stop restores the previous phase and allows later status events", async () => {
  const app = setup(async () => {
    throw new Error("stop failed");
  });
  const pending = app.feature.stopSelected();
  assert.equal(app.state().phases[account.id], "stopping");
  await pending;
  assert.equal(app.state().phases[account.id], "online");
  assert.equal(app.state().stoppingBotIds.size, 0);
  assert.equal(app.errors.length, 1);
  app.feature.handleBotEvent({ type: "status", bot_id: account.id, phase: "connecting" });
  assert.equal(app.state().phases[account.id], "connecting");
});

test("a rejected stop cannot overwrite an offline event received while waiting", async () => {
  let reject!: (error: Error) => void;
  const app = setup(
    () =>
      new Promise((_, fail) => {
        reject = fail;
      }),
  );
  const pending = app.feature.stopSelected();
  app.feature.handleBotEvent({ type: "status", bot_id: account.id, phase: "offline" });
  reject(new Error("channel closed"));
  await pending;
  assert.equal(app.state().phases[account.id], "offline");
  assert.equal(app.state().stoppingBotIds.size, 0);
});

test("successful stop completes even when no offline event arrives", async () => {
  const app = setup(async () => undefined);
  await app.feature.stopSelected();
  assert.equal(app.state().phases[account.id], "offline");
  assert.equal(app.state().stoppingBotIds.size, 0);
});

test("deleting the selected session removes its derived state", async () => {
  const app = setup(async () => undefined);
  await app.feature.deleteAccounts([account.id]);
  assert.equal(app.state().accounts.length, 0);
  assert.equal(app.state().selectedBotIds.size, 0);
  assert.deepEqual(app.state().phases, {});
  assert.equal(app.state().selectedSessionBotId, null);
});
