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

// 設定損壞或由較新版本寫入未知語言時，畫面仍須能取得有效翻譯。
import assert from "node:assert/strict";
import { test } from "node:test";
import { parseLocale, translate } from "../src/lib/i18n.ts";

test("unknown saved locales fall back to a usable English dictionary", () => {
  for (const value of [undefined, "", "fr", "__proto__"]) {
    assert.equal(parseLocale(value), "en");
    assert.equal(translate(parseLocale(value), "overview"), "Overview");
  }
  assert.equal(parseLocale("zh-CN"), "zh-CN");
  assert.equal(parseLocale("zh-TW"), "zh-TW");
});
