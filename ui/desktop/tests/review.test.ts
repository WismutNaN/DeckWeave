// SPDX-License-Identifier: GPL-3.0-or-later
import test from "node:test";
import assert from "node:assert/strict";
import { EditorSession } from "./session-loader";
import { previewClient } from "../src/editor/preview";
import type { EditorClient } from "../src/editor/api";
import type { MutationEnvelope } from "../src/editor/types";
test("disruption consent is explicit and included in the revalidated plan and apply", async () => {
  const base = previewClient(),
    commands: MutationEnvelope[] = [],
    plans: Record<string, unknown>[] = [];
  const client: EditorClient = {
    ...base,
    async command<T>(e) {
      commands.push(e);
      return base.command<T>(e);
    },
    async query<T>(q) {
      if (q.type === "plan_apply") {
        plans.push(q);
        if (!q.allow_disruption)
          throw {
            code: "plan_conflict",
            details: { requiresDisruption: true },
          };
        const plan = await base.query<Record<string, unknown>>(q);
        return {
          ...plan,
          requiresDisruption: true,
          disruptionAllowed: true,
        } as T;
      }
      return base.query<T>(q);
    },
  };
  const session = new EditorSession(client);
  await session.load();
  await session.apply();
  assert.equal(session.review?.requiresDisruption, true);
  assert.equal(session.review?.consent, false);
  assert.equal(session.plan, null);
  await session.confirmApply();
  assert.equal(commands.length, 0);
  session.review!.consent = true;
  await session.planReview();
  assert.equal(plans.at(-1)?.allow_disruption, true);
  await session.confirmApply();
  assert.equal(commands.at(-1)?.command.allow_disruption, true);
  assert.ok(commands.at(-1)?.command.plan_hash);
});
test("compiler diagnostics retain control/field ownership and block apply", async () => {
  const base = previewClient();
  const diagnostic = {
    code: "invalid_argument",
    nodeId: "action-1",
    field: "text",
  };
  const client: EditorClient = {
    ...base,
    async query<T>(q) {
      if (q.type === "plan_apply")
        throw { code: "plan_conflict", details: { diagnostics: [diagnostic] } };
      return base.query<T>(q);
    },
  };
  const session = new EditorSession(client);
  await session.load();
  await session.apply();
  assert.deepEqual(session.diagnostics, [diagnostic]);
  assert.equal(session.plan, null);
});
