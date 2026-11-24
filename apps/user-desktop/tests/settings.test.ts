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

// 驗證實際寫入完成、失敗重試與完整快照的順序。
import assert from "node:assert/strict";
import { test } from "node:test";
import { createSettingsFeature } from "../src/lib/features/settings.ts";

function deferred() {
  let resolve!: () => void;
  let reject!: (error: Error) => void;
  const promise = new Promise<void>((done, fail) => {
    resolve = done;
    reject = fail;
  });
  return { promise, resolve, reject };
}

test("save stays pending until its write completes and queued snapshots preserve all edits", async () => {
  const first = deferred();
  const second = deferred();
  const writes: Record<string, string>[] = [];
  let draft: Record<string, string> = {};
  const settings = createSettingsFeature({
    initialSettings: { theme: "dark" },
    onChange: (next) => {
      draft = next;
    },
    saveSettings: (snapshot) => {
      writes.push(snapshot);
      return writes.length === 1 ? first.promise : second.promise;
    },
  });
  let completed = false;
  const one = settings.save({ locale: "zh-TW" });
  void one.then(() => {
    completed = true;
  });
  const two = settings.save({ sound: "off" });
  await Promise.resolve();
  assert.equal(completed, false);
  assert.deepEqual(writes, [{ theme: "dark", locale: "zh-TW" }]);
  assert.deepEqual(draft, { theme: "dark", locale: "zh-TW", sound: "off" });
  first.resolve();
  await one;
  await Promise.resolve();
  assert.equal(completed, true);
  assert.deepEqual(writes[1], draft);
  second.resolve();
  await two;
});

test("failed writes reject without losing drafts or blocking retries", async () => {
  const first = deferred();
  const writes: Record<string, string>[] = [];
  let draft: Record<string, string> = {};
  const settings = createSettingsFeature({
    initialSettings: {},
    onChange: (next) => {
      draft = next;
    },
    saveSettings: async (snapshot) => {
      writes.push(snapshot);
      if (writes.length === 1) await first.promise;
    },
  });
  const failed = settings.save({ nick: "rules" });
  const rejection = assert.rejects(failed, /disk failed/);
  first.reject(new Error("disk failed"));
  await rejection;
  assert.deepEqual(draft, { nick: "rules" });
  await settings.save({});
  assert.deepEqual(writes, [{ nick: "rules" }, { nick: "rules" }]);
  await settings.save({ volume: "20" });
  assert.deepEqual(writes[2], { nick: "rules", volume: "20" });
});

test("saves triggered by a draft notification cannot write ahead of that draft", async () => {
  const snapshots: Record<string, string>[] = [];
  let nestedSave: Promise<void> | undefined;
  const settings = createSettingsFeature({
    initialSettings: {},
    onChange: (draft) => {
      if (draft.locale && !draft.theme) nestedSave = settings.save({ theme: "dark" });
    },
    saveSettings: async (snapshot) => {
      snapshots.push(snapshot);
    },
  });

  // 模擬畫面接到草稿通知後立刻儲存另一項偏好。
  await settings.save({ locale: "zh-TW" });
  await nestedSave;
  assert.deepEqual(snapshots, [{ locale: "zh-TW" }, { locale: "zh-TW", theme: "dark" }]);
});
