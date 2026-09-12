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
