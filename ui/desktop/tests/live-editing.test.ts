// SPDX-License-Identifier: GPL-3.0-or-later
import test from "node:test";
import assert from "node:assert/strict";
import { EditorSession } from "./session-loader";
import { previewClient } from "../src/editor/preview";
import { copy, type EditorClient } from "../src/editor/api";
import { layout } from "../src/editor/appearance";
import type { GraphDocument, MutationEnvelope } from "../src/editor/types";
import type { Recovery } from "../src/editor/recovery";

function latch() {
  let release!: () => void;
  const promise = new Promise<void>((resolve) => {
    release = resolve;
  });
  return { promise, release };
}
async function editor(client = previewClient()) {
  const s = new EditorSession(client, true);
  await s.load();
  s.liveEditing.start();
  return s;
}
function record(base = previewClient()) {
  const calls: MutationEnvelope[] = [];
  const client: EditorClient = {
    ...base,
    async command<T>(e) {
      calls.push(copy(e));
      return base.command<T>(e);
    },
  };
  return { client, calls };
}
async function activeX(s: InstanceType<typeof EditorSession>) {
  const active = await s.client.query<{ document: GraphDocument }>({
    type: "active_document",
  });
  return layout(active.document).devices[0].x;
}

test("startup has no effect; a burst and undo each publish only their latest document", async (t) => {
  const { client, calls } = record();
  const s = await editor(client);
  t.after(() => s.liveEditing.dispose());
  await s.liveEditing.flush();
  assert.equal(calls.length, 0);
  for (const x of [1, 2, 3]) s.updateDevice((d) => (d.x = x));
  await s.liveEditing.flush();
  assert.deepEqual(
    calls.map((c) => c.command.type),
    ["patch_draft", "apply_draft"],
  );
  assert.equal(await activeX(s), 3);
  assert.equal(s.review, null);
  s.history("undo");
  await s.liveEditing.flush();
  assert.equal(await activeX(s), 2);
  assert.equal(s.dirty, false);
});

for (const stage of ["save", "plan", "apply"] as const) {
  test(`editing while ${stage} waits preserves newer work and discards stale plans`, async (t) => {
    const base = previewClient(),
      entered = latch(),
      released = latch();
    let held = false;
    const gate = async () => {
      if (held) return;
      held = true;
      entered.release();
      await released.promise;
    };
    const { client, calls } = record({
      ...base,
      async command<T>(e) {
        const result = await base.command<T>(e);
        if (
          (stage === "save" && e.command.type === "patch_draft") ||
          (stage === "apply" && e.command.type === "apply_draft")
        )
          await gate();
        return result;
      },
      async query<T>(q) {
        const result = await base.query<T>(q);
        if (stage === "plan" && q.type === "plan_apply") await gate();
        return result;
      },
    });
    const s = await editor(client);
    t.after(() => s.liveEditing.dispose());
    s.updateDevice((d) => (d.x = 10));
    const applying = s.liveEditing.flush();
    await entered.promise;
    assert.equal(s.editLocked, false);
    s.updateDevice((d) => (d.x = 20));
    assert.equal(s.device?.x, 20);
    released.release();
    await applying;
    assert.equal(s.device?.x, 20);
    assert.equal(await activeX(s), 20);
    assert.equal(s.dirty, false);
    assert.deepEqual(
      calls
        .filter((c) => c.command.type === "apply_draft")
        .map((c) => c.command.expected_draft_version),
      stage === "apply" ? [2, 3] : [3],
    );
    assert.equal(s.liveStatus, "idle");
  });
}

test("an uncertain save keeps identity and later edits across recovery without automatic retry", async (t) => {
  const base = previewClient(),
    entered = latch(),
    released = latch();
  let lose = true,
    recovery: Recovery | null = null;
  const journal = {
    read: () => copy(recovery),
    write: (v: Recovery) => {
      recovery = copy(v);
    },
  };
  const { client, calls } = record({
    ...base,
    async command<T>(e) {
      const result = await base.command<T>(e);
      if (lose) {
        lose = false;
        entered.release();
        await released.promise;
        throw new Error("receipt lost");
      }
      return result;
    },
  });
  const s = await editor(client);
  s.journal = journal;
  t.after(() => s.liveEditing.dispose());
  s.updateDevice((d) => (d.x = 10));
  const applying = s.liveEditing.flush();
  await entered.promise;
  s.updateDevice((d) => (d.x = 20));
  released.release();
  await applying;
  await s.liveEditing.flush();
  assert.equal(calls.length, 1);
  assert.equal(s.editLocked, true);
  assert.equal(s.liveStatus, "error");
  const restored = new EditorSession(client, true);
  restored.journal = journal;
  await restored.load();
  restored.liveEditing.start();
  t.after(() => restored.liveEditing.dispose());
  await restored.liveEditing.flush();
  assert.equal(calls.length, 1);
  await restored.retry();
  assert.deepEqual(calls[0], calls[1]);
  assert.equal(restored.device?.x, 20);
  assert.equal(restored.dirty, true);
  assert.equal(restored.pending, null);
  assert.ok(calls.every((c) => c.command.type === "patch_draft"));
  restored.liveEditing.resume();
  await restored.liveEditing.flush();
  assert.equal(await activeX(restored), 20);
});

test("invalid intermediate input leaves the previous active revision and resumes only on a new edit", async (t) => {
  const base = previewClient();
  let invalid = false;
  const { client, calls } = record({
    ...base,
    async query<T>(q) {
      if (q.type === "plan_apply" && invalid)
        throw {
          code: "plan_conflict",
          details: {
            diagnostics: [{ code: "invalid_argument", nodeId: "test" }],
          },
        };
      return base.query<T>(q);
    },
  });
  const s = await editor(client);
  t.after(() => s.liveEditing.dispose());
  s.updateDevice((d) => (d.x = 1));
  await s.liveEditing.flush();
  invalid = true;
  s.updateDevice((d) => (d.x = 2));
  await s.liveEditing.flush();
  assert.equal(await activeX(s), 1);
  assert.equal(s.diagnostics[0].code, "invalid_argument");
  const count = calls.length;
  invalid = false;
  await s.liveEditing.flush();
  assert.equal(calls.length, count);
  s.updateDevice((d) => (d.x = 3));
  await s.liveEditing.flush();
  assert.equal(await activeX(s), 3);
});

test("pause during planning does not submit the prepared revision", async (t) => {
  const base = previewClient(),
    entered = latch(),
    released = latch();
  const { client, calls } = record({
    ...base,
    async query<T>(q) {
      if (q.type === "plan_apply") {
        entered.release();
        await released.promise;
      }
      return base.query<T>(q);
    },
  });
  const s = await editor(client);
  t.after(() => s.liveEditing.dispose());
  s.updateDevice((d) => (d.x = 1));
  const applying = s.liveEditing.flush();
  await entered.promise;
  s.liveEditing.pause();
  released.release();
  await applying;
  assert.deepEqual(
    calls.map((c) => c.command.type),
    ["patch_draft"],
  );
  s.liveEditing.resume();
  await s.liveEditing.flush();
  assert.equal(await activeX(s), 1);
});

test("a stale validation error cannot discard the corrected form waiting behind it", async (t) => {
  const base = previewClient(),
    entered = latch(),
    released = latch();
  let invalid = true;
  const s = await editor({
    ...base,
    async query<T>(q) {
      if (q.type === "plan_apply" && invalid) {
        invalid = false;
        entered.release();
        await released.promise;
        throw {
          code: "plan_conflict",
          details: { diagnostics: [{ code: "invalid_argument" }] },
        };
      }
      return base.query<T>(q);
    },
  });
  t.after(() => s.liveEditing.dispose());
  s.updateDevice((d) => (d.x = 1));
  const applying = s.liveEditing.flush();
  await entered.promise;
  s.updateDevice((d) => (d.x = 2));
  released.release();
  await applying;
  assert.equal(await activeX(s), 2);
  assert.equal(s.error, "");
  assert.equal(s.liveStatus, "idle");
});

test("a failed activation is reported without automatic reapply", async (t) => {
  const base = previewClient();
  const { client, calls } = record({
    ...base,
    async query<T>(q) {
      if (q.type === "operation")
        return { state: "succeeded", runtimeHealth: "degraded" } as T;
      return base.query<T>(q);
    },
  });
  const s = await editor(client);
  s.preview = false;
  t.after(() => s.liveEditing.dispose());
  s.updateDevice((d) => (d.x = 1));
  await s.liveEditing.flush();
  assert.equal(s.liveStatus, "error");
  assert.equal(s.pending, null);
  await s.liveEditing.flush();
  assert.equal(calls.filter((c) => c.command.type === "apply_draft").length, 1);
});

test("failed recovery storage prevents dispatch and does not silently retry", async (t) => {
  const { client, calls } = record();
  const s = await editor(client);
  t.after(() => s.liveEditing.dispose());
  s.journal = {
    read: () => null,
    write: () => {
      throw new Error("disk full");
    },
  };
  s.updateDevice((d) => (d.x = 1));
  await s.liveEditing.flush();
  await s.liveEditing.flush();
  assert.equal(calls.length, 0);
  assert.match(s.error, /disk full/);
  assert.equal(s.liveStatus, "error");
});

test("permission changes replan an unchanged document in live mode but respect pause", async (t) => {
  const { client, calls } = record();
  const s = await editor(client);
  t.after(() => s.liveEditing.dispose());
  assert.equal(s.dirty, false);
  await s.liveEditing.permissionsChanged();
  await s.liveEditing.flush();
  assert.deepEqual(
    calls.map((c) => c.command.type),
    ["apply_draft"],
  );
  s.liveEditing.pause();
  await s.liveEditing.permissionsChanged();
  await s.liveEditing.flush();
  assert.equal(calls.length, 1);
  s.liveEditing.resume();
  await s.liveEditing.flush();
  assert.equal(calls.length, 2);
});

test("a permission change waits for an in-flight apply and never repeats an uncertain mutation", async (t) => {
  for (const uncertain of [false, true]) {
    const base = previewClient(),
      entered = latch(),
      released = latch();
    let first = true;
    const { client, calls } = record({
      ...base,
      async command<T>(e) {
        const result = await base.command<T>(e);
        if (first && e.command.type === "apply_draft") {
          first = false;
          entered.release();
          await released.promise;
          if (uncertain) throw new Error("reply lost");
        }
        return result;
      },
    });
    const s = await editor(client);
    t.after(() => s.liveEditing.dispose());
    s.updateDevice((d) => (d.x = 12));
    const applying = s.liveEditing.flush();
    await entered.promise;
    const changed = s.liveEditing.permissionsChanged();
    released.release();
    await Promise.all([applying, changed]);
    await s.liveEditing.flush();
    assert.equal(
      calls.filter((c) => c.command.type === "apply_draft").length,
      uncertain ? 1 : 2,
    );
    assert.equal(Boolean(s.pending), uncertain);
    if (uncertain) {
      assert.equal(s.liveStatus, "error");
      assert.match(s.error, /reply lost/);
    }
  }
});
