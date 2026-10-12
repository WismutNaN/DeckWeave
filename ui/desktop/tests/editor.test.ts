// SPDX-License-Identifier: GPL-3.0-or-later
import test from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { editorFixture } from "./fixture";
import { previewData, previewClient } from "../src/editor/preview";
import { copy, type EditorClient } from "../src/editor/api";
import { controlBackground, geometry, layout } from "../src/editor/appearance";
import {
  appendAction,
  removeLastAction,
  sourceFor,
  equal,
} from "../src/editor/bindings";
import { assignable, valuePaths } from "../src/editor/variables";
import {
  ACTION_DRAG_TYPE,
  hasActionDrag,
  readActionDrag,
  writeActionDrag,
} from "../src/editor/drag";
import {
  mergeDocuments,
  type Recovery,
  type RecoveryJournal,
} from "../src/editor/recovery";
import type { MutationEnvelope } from "../src/editor/types";
import { EditorSession } from "./session-loader";

class TestDataTransfer {
  private readonly values = new Map<string, string>();
  effectAllowed = "uninitialized";
  get types() {
    return [...this.values.keys()];
  }
  setData(type: string, value: string) {
    this.values.set(type, value);
  }
  getData(type: string) {
    return this.values.get(type) ?? "";
  }
  delete(type: string) {
    this.values.delete(type);
  }
}

test("action drag survives the WebView text fallback and rejects foreign data", () => {
  const endpoint = previewData().catalog.endpoints.find(
    (item) => item.operation,
  )!.endpoint;
  const transfer = new TestDataTransfer();
  writeActionDrag(transfer as DataTransfer, endpoint);
  assert.equal(transfer.effectAllowed, "copy");
  assert.ok(hasActionDrag(transfer as DataTransfer));
  assert.deepEqual(readActionDrag(transfer as DataTransfer), endpoint);

  transfer.delete(ACTION_DRAG_TYPE);
  assert.deepEqual(readActionDrag(transfer as DataTransfer), endpoint);

  const foreign = new TestDataTransfer();
  foreign.setData("text/plain", JSON.stringify(endpoint));
  assert.equal(readActionDrag(foreign as DataTransfer), null);
});

test("action metadata follows the pinned version and never guesses between installed versions", async () => {
  const session = new EditorSession(previewClient());
  await session.load();
  const first = copy(session.catalogs[0]),
    second = copy(first);
  second.package.version = "2.0.0";
  second.package.artifactSha256 = "c".repeat(64);
  session.catalogs = [second, first];
  assert.equal(session.actions.length, 0);
  assert.equal(session.ambiguousPackages.length, 1);
  session.draft!.packageLock = [{ ...first.package, source: null }];
  assert.equal(session.actions.length, 3);
  assert.ok(
    session.actions.every((a) => a.catalog.package.version === "1.0.0"),
  );
});
test("revoking access after a lost response cannot erase the unknown mutation", async () => {
  const underlying = previewClient();
  let attempts = 0;
  const client: EditorClient = {
    ...underlying,
    async command<T>(e: MutationEnvelope) {
      attempts++;
      if (attempts === 1) {
        await underlying.command<T>(e);
        throw new Error("lost");
      }
      throw { code: "permission_denied" };
    },
  };
  const session = new EditorSession(client);
  await session.load();
  session.updateDevice((d) => (d.y = 99));
  await session.run(() => session.save());
  const key = session.pending?.idempotencyKey;
  await session.retry();
  assert.equal(session.pending?.idempotencyKey, key);
  assert.ok(session.pending);
  assert.equal(attempts, 2);
});
test("Rust acceptance fixture matches the actual desktop graph builder", () => {
  assert.deepEqual(
    editorFixture(),
    JSON.parse(
      readFileSync(
        new URL("./fixtures/assignment.json", import.meta.url),
        "utf8",
      ),
    ),
  );
});
test("device gradient is shared spatially and overrides survive palette changes", () => {
  const d = layout(previewData().draft.document).devices[0];
  assert.notEqual(
    controlBackground(d, "key-1"),
    controlBackground(d, "key-15"),
  );
  d.controls["key-1"] = {
    background: { ...d.appearance, mode: "solid", start: "#123456" },
  };
  const own = controlBackground(d, "key-1");
  d.appearance.start = "#ffffff";
  assert.equal(controlBackground(d, "key-1"), own);
  delete d.controls["key-1"].background;
  assert.match(controlBackground(d, "key-1"), /#ffffff/);
});
test("unknown or broken geometry never claims physical positions", () => {
  const d = layout(previewData().draft.document).devices[0];
  d.descriptor["org.deckweave.controlLayout"]!.elements["key-1"].x = -1;
  assert.equal(geometry(d.descriptor).measured, false);
  assert.equal(controlBackground(d, "key-1"), d.appearance.start);
});
test("input lookup respects physical identity and requires an explicit ambiguous port", () => {
  const { catalog, draft } = previewData(),
    d = layout(draft.document).devices[0];
  assert.ok(sourceFor([catalog], d, "key-1"));
  const twin = copy(d);
  twin.descriptor.identity = "other";
  assert.equal(sourceFor([catalog], twin, "key-1"), undefined);
  const second = copy(catalog.endpoints[0]);
  second.endpoint.port.portId = "touch";
  catalog.endpoints.push(second);
  assert.equal(sourceFor([catalog], d, "key-1"), undefined);
  d.controls["key-1"] = { trigger: second.endpoint };
  assert.equal(sourceFor([catalog], d, "key-1")?.endpoint.port.portId, "touch");
});
test("removing one control chain preserves unrelated graph and extensions", () => {
  const { draft, catalog } = editorFixture(),
    d = layout(draft.document).devices[0];
  const action = {
    ...catalog.endpoints.find((e) => e.operation)!,
    catalog,
    title: "action",
  };
  appendAction(draft.document, [catalog], d, "key-2", action, "other");
  draft.document.extensions["example.unknown"] = { keep: true };
  removeLastAction(draft.document, d.id, "key-1");
  removeLastAction(draft.document, d.id, "key-1");
  assert.ok(draft.document.graph.nodes.some((n) => n.id === "action-other"));
  assert.ok(!draft.document.graph.nodes.some((n) => n.id === "action-first"));
  assert.deepEqual(draft.document.extensions["example.unknown"], {
    keep: true,
  });
});
test("variables filter type, bounds, optional values and escaped field paths", () => {
  const target = { type: "string" as const, maximumBytes: 80 };
  assert.equal(
    assignable({ type: "string", maximumBytes: 2048 }, target),
    false,
  );
  assert.equal(assignable({ type: "boolean" }, target), false);
  assert.deepEqual(
    valuePaths(
      {
        type: "record",
        fields: {
          "a/b~c": { schema: target },
          maybe: { schema: target, optional: true },
        },
      },
      target,
    ),
    [{ pointer: "/a~1b~0c", label: "/a~1b~0c" }],
  );
  assert.ok(equal({ b: 2, a: 1 }, { a: 1, b: 2 }));
});
test("three-way merge preserves disjoint updates and fails closed on conflicting arrays", () => {
  const base = previewData().draft.document,
    l = copy(base),
    r = copy(base);
  l.extensions["local.field"] = 1;
  r.extensions["remote.field"] = 2;
  const merged = mergeDocuments(base, l, r);
  assert.equal(merged.extensions["local.field"], 1);
  assert.equal(merged.extensions["remote.field"], 2);
  layout(l).devices[0].x = 1;
  layout(r).devices[0].x = 2;
  assert.throws(() => mergeDocuments(base, l, r), /Edit conflict/);
});
function journal(): RecoveryJournal {
  let value: Recovery | null = null;
  return {
    read: () => copy(value),
    write: (v) => {
      value = copy(v);
    },
  };
}
test("lost save receipt survives editor restart and retries exactly once without applying", async () => {
  const underlying = previewClient(),
    calls: MutationEnvelope[] = [],
    store = journal();
  let fail = true;
  const client: EditorClient = {
    ...underlying,
    async command<T>(envelope: MutationEnvelope) {
      calls.push(copy(envelope));
      const result = await underlying.command<T>(envelope);
      if (fail) {
        fail = false;
        throw new Error("transport lost");
      }
      return result;
    },
  };
  const first = new EditorSession(client);
  first.journal = store;
  await first.load();
  first.updateDevice((d) => (d.x = 33));
  await first.run(() => first.save());
  assert.ok(first.pending);
  assert.match(first.error, /transport lost/);
  const next = new EditorSession(client);
  next.journal = store;
  await next.load();
  assert.ok(next.pending);
  await next.retry();
  assert.equal(next.draft?.version, 2);
  assert.equal(next.pending, null);
  assert.equal(next.device?.x, 33);
  assert.deepEqual(calls[0], calls[1]);
  assert.ok(calls.every((c) => c.command.type === "patch_draft"));
});
test("query failure after apply receipt retains the mutation identity", async () => {
  const underlying = previewClient(),
    calls: MutationEnvelope[] = [];
  let fail = true;
  const client: EditorClient = {
    ...underlying,
    async command<T>(e: MutationEnvelope) {
      calls.push(copy(e));
      return underlying.command<T>(e);
    },
    async query<T>(q: Record<string, unknown>) {
      if (q.type === "operation" && fail) {
        fail = false;
        throw { code: "permission_denied" };
      }
      return underlying.query<T>(q);
    },
  };
  const session = new EditorSession(client);
  await session.load();
  await session.apply();
  await session.confirmApply();
  assert.ok(session.pending);
  await session.retry();
  assert.equal(session.pending, null);
  assert.deepEqual(calls[0], calls[1]);
});
test("CAS rejection retains local work, and merge never overwrites remote changes", async () => {
  const underlying = previewClient();
  let fail = true;
  const client: EditorClient = {
    ...underlying,
    async command<T>(e: MutationEnvelope) {
      if (fail) {
        fail = false;
        throw { code: "revision_conflict" };
      }
      return underlying.command<T>(e);
    },
  };
  const session = new EditorSession(client);
  await session.load();
  session.updateDevice((d) => (d.x = 11));
  await session.run(() => session.save());
  assert.equal(session.pending, null);
  assert.equal(session.conflict, true);
  assert.equal(session.device?.x, 11);
  await session.mergeLatest();
  await session.run(() => session.save());
  assert.equal(session.conflict, false);
  assert.equal(session.draft?.version, 2);
});
test("locale fallback is per key and notification failure reconciles the persisted revision", async () => {
  const client = previewClient();
  let reconciles = 0;
  const session = new EditorSession({
    ...client,
    changeLocale: async (_, next) => ({ context: next, deliveryPending: true }),
    reconcileLocale: async () => {
      reconciles++;
      return {
        context: { revision: 2, requestedLocale: "en", timeZone: "UTC" },
        deliveryPending: false,
      };
    },
  });
  await session.load();
  await session.language("en");
  assert.equal(reconciles, 1);
  assert.equal(session.localePending, false);
  assert.equal(session.actions[0].title, "Show text");
  session.locale.requestedLocale = "ru-RU";
  delete session.catalogs[0].translations!.messages.ru["show-text"];
  assert.equal(session.actions[0].title, "Show text");
});
