// SPDX-License-Identifier: GPL-3.0-or-later
import test from "node:test";
import assert from "node:assert/strict";
import { EditorSession } from "./session-loader";
import { previewClient, previewData } from "../src/editor/preview";
import { pushHistory } from "../src/editor/history";
import type { Recovery } from "../src/editor/recovery";
test("asynchronous recovery is committed before a mutation can reach the host", async () => {
  const base = previewClient();
  let sent = 0,
    release: () => void = () => {};
  let block = false;
  const session = new EditorSession({
    ...base,
    async command<T>(e) {
      sent++;
      return base.command<T>(e);
    },
  });
  session.journal = {
    read: async () => null,
    write: async () => {
      if (block) await new Promise<void>((resolve) => (release = resolve));
    },
  };
  await session.load();
  session.updateDevice((d) => d.x++);
  block = true;
  const save = session.save();
  await new Promise((r) => setTimeout(r, 0));
  assert.equal(sent, 0);
  block = false;
  release();
  await save;
  assert.equal(sent, 1);
});
test("failed durable recovery blocks dispatch without losing the original request identity", async () => {
  const base = previewClient();
  let sent = 0,
    fail = false;
  let stored: Recovery | null = null;
  const session = new EditorSession({
    ...base,
    async command<T>(e) {
      sent++;
      return base.command<T>(e);
    },
  });
  session.journal = {
    read: async () => stored,
    write: async (value) => {
      if (fail) throw new Error("disk full");
      stored = value;
    },
  };
  await session.load();
  session.updateDevice((d) => d.x++);
  fail = true;
  await assert.rejects(() => session.save(), /disk full/);
  assert.equal(sent, 0);
  const key = session.pending!.idempotencyKey;
  fail = false;
  await session.save();
  assert.equal(sent, 1);
  assert.equal(stored!.pending, null);
  assert.ok(key);
});
test("undo snapshots obey a byte budget while retaining the latest edit", () => {
  const doc = previewData().draft.document,
    size = JSON.stringify(doc).length * 2;
  const history = Array.from({ length: 65 }, () => structuredClone(doc));
  assert.equal(pushHistory(history, doc, size * 2).length, 2);
  assert.equal(pushHistory(history, doc, 1).length, 1);
  assert.equal(pushHistory(history, doc, Infinity).length, 64);
});
