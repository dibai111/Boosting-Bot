// 驗證卸載與非同步監聽註冊交錯時，所有監聽仍會恰好清理一次。
import assert from "node:assert/strict";
import { test } from "node:test";
import { createListenerScope } from "../src/lib/features/listener-scope.ts";

test("cleans up listeners that finish registering after disposal", async () => {
  const scope = createListenerScope();
  let complete!: (stop: () => void) => void;
  let stops = 0;
  const pending = scope.track(
    new Promise((resolve) => {
      complete = resolve;
    }),
  );
  scope.dispose();
  complete(() => {
    stops += 1;
  });
  await pending;
  scope.dispose();
  assert.equal(stops, 1);
  assert.equal(scope.isDisposed(), true);
});

test("keeps successful registrations available for cleanup when another one fails", async () => {
  const scope = createListenerScope();
  let stops = 0;
  await scope.track(
    Promise.resolve(() => {
      stops += 1;
    }),
  );
  await assert.rejects(scope.track(Promise.reject(new Error("registration failed"))));
  scope.dispose();
  scope.dispose();
  assert.equal(stops, 1);
});
