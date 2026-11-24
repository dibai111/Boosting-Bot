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

// 查詢、事件與命令回覆共用版本判定，晚到資料不能倒退視窗狀態。
import assert from "node:assert/strict";
import { test } from "node:test";
import { createOverlayFeature } from "../src/lib/features/overlay.ts";
import type { OverlaySnapshot } from "../src/lib/models/types.ts";

test("late toggle and initial query replies cannot overwrite a newer hide event", async () => {
  let resolveToggle!: (snapshot: OverlaySnapshot) => void;
  let resolveRead!: (snapshot: OverlaySnapshot) => void;
  let visible = false;
  const changes: OverlaySnapshot[] = [];
  const overlay = createOverlayFeature({
    api: {
      toggleMatchmakingOverlay: () =>
        new Promise((resolve) => {
          resolveToggle = resolve;
        }),
      overlaySnapshot: () =>
        new Promise((resolve) => {
          resolveRead = resolve;
        }),
      showMatchmakingOverlay: async () => ({ revision: 4, visible: true }),
      hideMatchmakingOverlay: async () => ({ revision: 5, visible: false }),
    },
    onChange: (snapshot) => {
      visible = snapshot.visible;
      changes.push(snapshot);
    },
  });
  const toggle = overlay.toggle(2);
  const read = overlay.refresh();
  overlay.apply({ revision: 3, visible: false });
  resolveToggle({ revision: 2, visible: true });
  resolveRead({ revision: 1, visible: true });
  await Promise.all([toggle, read]);
  assert.equal(visible, false);
  assert.equal(changes.length, 1);
  assert.equal(overlay.apply({ revision: 3, visible: true }), false);
  await overlay.show(2);
  assert.equal(visible, true);
  await overlay.hide();
  assert.equal(visible, false);
});

test("a failed native operation rejects without publishing a successful state", async () => {
  const changes: OverlaySnapshot[] = [];
  const overlay = createOverlayFeature({
    api: {
      showMatchmakingOverlay: async () => {
        throw new Error("show failed");
      },
      hideMatchmakingOverlay: async () => ({ revision: 1, visible: false }),
      toggleMatchmakingOverlay: async () => ({ revision: 2, visible: true }),
      overlaySnapshot: async () => ({ revision: 0, visible: false }),
    },
    onChange: (snapshot) => changes.push(snapshot),
  });
  await assert.rejects(overlay.show(1), /show failed/);
  assert.deepEqual(changes, []);
  await overlay.refresh();
  assert.deepEqual(changes, [{ revision: 0, visible: false }]);
});
