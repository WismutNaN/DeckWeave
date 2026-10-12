// SPDX-License-Identifier: GPL-3.0-or-later
import test from "node:test";
import assert from "node:assert/strict";
import { editorFixture } from "./fixture";
import { layout } from "../src/editor/appearance";
import { steps } from "../src/editor/bindings";
import {
  copyControlBinding,
  reorderAction,
  removeControlBinding,
} from "../src/editor/binding-edits";
import { createInstance, deleteInstance } from "../src/editor/instances";
import { summarizeChanges } from "../src/editor/review";

test("copying a control remaps results, preserves configured instances and cannot overwrite another binding", () => {
  const { draft, catalog } = editorFixture(),
    doc = draft.document,
    device = layout(doc).devices[0];
  const original = steps(doc, device.id, "key-1");
  doc.graph.nodeInstances = { [original[0].id]: "work" };
  copyControlBinding(doc, [catalog], device, "key-1", device, "key-2", false);
  const cloned = steps(doc, device.id, "key-2");
  assert.equal(cloned.length, 2);
  assert.notEqual(cloned[0].id, original[0].id);
  assert.deepEqual(doc.graph.actionArguments![cloned[1].id].fields.text, {
    type: "run_result",
    nodeId: cloned[0].id,
    pointer: "",
  });
  assert.equal(doc.graph.nodeInstances![cloned[0].id], "work");
  assert.throws(() =>
    copyControlBinding(doc, [catalog], device, "key-1", device, "key-2", false),
  );
  removeControlBinding(doc, device.id, "key-1");
  assert.equal(steps(doc, device.id, "key-2").length, 2);
});

test("reordering cannot put a consumer before its producer or detach an external graph edge", () => {
  const { draft, catalog } = editorFixture(),
    doc = draft.document,
    device = layout(doc).devices[0];
  const [first, second] = steps(doc, device.id, "key-1");
  assert.throws(() =>
    reorderAction(doc, [catalog], device.id, "key-1", second.id, first.id),
  );
  doc.graph.actionArguments![second.id].fields.text = {
    type: "constant",
    value: "independent",
  };
  reorderAction(doc, [catalog], device.id, "key-1", second.id, first.id);
  assert.equal(steps(doc, device.id, "key-1")[0].id, second.id);
  doc.graph.edges.push({
    ...doc.graph.edges[0],
    id: "foreign",
    source: { nodeId: "foreign", portId: "out" },
    target: { nodeId: first.id, portId: "trigger" },
  });
  assert.throws(() => removeControlBinding(doc, device.id, "key-1"));
});

test("instance settings are independent and referenced instances cannot be deleted", () => {
  const { draft, catalog } = editorFixture(),
    doc = draft.document;
  const template = {
    contributionId: catalog.endpoints.find((e) => e.operation)!.contributionId,
    settingsSchema: catalog.endpoints.find((e) => e.operation)!.operation!
      .inputSchema,
    labelKey: "account",
    requiredFeatures: {},
  };
  createInstance(doc, catalog, template, "Work", "work");
  createInstance(doc, catalog, template, "Home", "home");
  doc.pluginInstances!.work.settings = { text: "private" };
  assert.notDeepEqual(
    doc.pluginInstances!.home.settings,
    doc.pluginInstances!.work.settings,
  );
  doc.graph.nodeInstances = { action: "work" };
  assert.throws(() => deleteInstance(doc, "work"));
  deleteInstance(doc, "home");
  assert.equal(doc.pluginInstances!.home, undefined);
});

test("review counts configuration changes without exposing field values", () => {
  const { draft } = editorFixture(),
    before = structuredClone(draft.document),
    after = structuredClone(before);
  const id = Object.keys(after.graph.actionArguments!)[0];
  after.graph.actionArguments![id].fields.text = {
    type: "constant",
    value: "private-token",
  };
  const summary = summarizeChanges(before, after);
  assert.equal(summary.changed, 1);
  assert.ok(!JSON.stringify(summary).includes("private-token"));
});
