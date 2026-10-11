// SPDX-License-Identifier: GPL-3.0-or-later
import { test } from "node:test";
import assert from "node:assert/strict";
import { scanMarkdown } from "./check-docs.mjs";

test("anchors поддерживают кириллицу, повторные headings и explicit IDs", () => {
  const result = scanMarkdown('# Пример\n## Пример\n<a id="fixed"></a>');
  assert.deepEqual([...result.anchors], ["пример", "пример-1", "fixed"]);
});
test("fences исключают ложные links/headings, JSON проверяется отдельно", () => {
  const result = scanMarkdown(
    '```json\n{"id": 1}\n```\n```text\n# Не heading\n[bad](absent.md)\n```',
  );
  assert.equal(result.anchors.size, 0);
  assert.equal(result.prose, "");
  assert.deepEqual(JSON.parse(result.json[0]), { id: 1 });
  assert.equal(scanMarkdown("```text\nunfinished").unclosedFence, true);
});
