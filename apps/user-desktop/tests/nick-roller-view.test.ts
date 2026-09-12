// 使用固定時鐘驗證候選重播、倒數及驗證結果，毋須連線到遊戲伺服器。
import assert from "node:assert/strict";
import { test } from "node:test";
import { createNickBotViewBuilder } from "../src/lib/features/nick-roller-view.ts";
import type { Account, BotEvent } from "../src/lib/models/types.ts";

test("leaving the decision phase removes stale candidate actions", () => {
  for (const phase of ["stopped", "failed", "finished", "verifying"] as const) {
    const view = createNickBotViewBuilder()(
      account,
      [
        candidate(),
        {
          type: "nick_roller_state",
          bot_id: account.id,
          phase,
        },
      ],
      "idle",
    );
    assert.equal(view.candidateId, null);
    assert.equal(view.candidate, "");
    assert.equal(view.decisionDeadline, null);
  }
});

test("preparing a new run clears the previous run counters and feedback", () => {
  const view = createNickBotViewBuilder()(
    account,
    [
      candidate(),
      {
        type: "nick_verification",
        bot_id: account.id,
        candidate_id: 1,
        expected_nick: "AquaVtx",
        success: false,
        reason: "timeout",
        processed_count: 2,
      },
      {
        type: "nick_attention",
        bot_id: account.id,
        code: "invalid_book",
        message: "Try again",
      },
      {
        type: "nick_roller_state",
        bot_id: account.id,
        phase: "preparing",
      },
    ],
    "nick_roller",
  );
  assert.equal(view.processed, 0);
  assert.equal(view.matched, 0);
  assert.equal(view.rejected, 0);
  assert.equal(view.verification, null);
  assert.equal(view.attention, null);
});

const account: Account = {
  id: "bot-a",
  username: "Alpha",
  auth_kind: "microsoft",
  server_address: "mc.hypixel.net",
  created_at: "",
  updated_at: "",
};
const candidate = (): BotEvent => ({
  type: "nick_candidate",
  bot_id: "bot-a",
  candidate_id: 1,
  nick: "AquaVtx",
  accepted: true,
  reasons: [],
  processed_count: 2,
  accepted_count: 1,
  rejected_count: 1,
  decision_timeout_ms: 100_000,
});

test("replaying a candidate with unrelated events does not restart its deadline", (context) => {
  context.mock.method(Date, "now", () => 1_000);
  const build = createNickBotViewBuilder();
  const event = candidate();
  const first = build(account, [event], "nick_roller");
  assert.equal(first.phase, "awaiting_decision");
  assert.equal(first.candidate, "AquaVtx");
  assert.equal(first.decisionDeadline, 101_000);
  context.mock.method(Date, "now", () => 50_000);
  const replay = build(
    account,
    [event, { type: "nick_roller_state", bot_id: "bot-b", phase: "rolling" }],
    "nick_roller",
  );
  assert.equal(replay.decisionDeadline, first.decisionDeadline);
  // 下一次執行可能重用 candidate_id，但不同事件仍應取得新的期限。
  assert.equal(build(account, [candidate()], "nick_roller").decisionDeadline, 150_000);
});

test("verification clears an actionable candidate and preserves its counters", () => {
  const build = createNickBotViewBuilder();
  const view = build(
    account,
    [
      candidate(),
      {
        type: "nick_verification",
        bot_id: "bot-a",
        candidate_id: 1,
        expected_nick: "AquaVtx",
        actual_nick: "AquaVtx",
        success: true,
        reason: "matched",
        processed_count: 2,
      },
    ],
    "idle",
  );
  assert.equal(view.phase, "finished");
  assert.equal(view.candidateId, null);
  assert.equal(view.decisionDeadline, null);
  assert.equal(view.processed, 2);
  assert.equal(view.rejected, 1);
});
