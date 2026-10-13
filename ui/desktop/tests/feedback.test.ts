// SPDX-License-Identifier: GPL-3.0-or-later
import test from "node:test";
import assert from "node:assert/strict";
import { editorFixture } from "./fixture";
import { layout, putLayout } from "../src/editor/appearance";
import { steps } from "../src/editor/bindings";
import {
  removeEditorFeedback,
  syncEditorFeedback,
} from "../src/editor/feedback";
import { copyControlBinding } from "../src/editor/binding-edits";
function fixture() {
  const { draft, catalog } = editorFixture(),
    doc = draft.document,
    view = layout(doc),
    device = view.devices[0];
  const source = catalog.endpoints.find(
    (e) => e.endpoint.target.kind === "device",
  )!;
  const schema = steps(doc, device.id, "key-1").at(-1)!.outputs[0].schema;
  for (const control of ["key-1", "key-2"])
    for (const [kind, id] of [
      ["ValueIn", "text"],
      ["ValueOut", "displayed"],
    ])
      catalog.endpoints.push({
        ...structuredClone(source),
        endpoint: {
          ...structuredClone(source.endpoint),
          port: { elementId: control, capabilityId: "display", portId: id },
        },
        port: {
          ...source.port,
          kind,
          portId: id,
          schema,
          tags: ["org.deckweave.virtual.display"],
        },
      });
  device.controls["key-1"] = { showResult: true };
  putLayout(doc, view);
  return { doc, catalog, device };
}
test("provider readback is an independent state route and survives copy without dangling references", () => {
  const { doc, catalog, device } = fixture();
  syncEditorFeedback(doc, [catalog]);
  assert.equal(
    doc.graph.nodes.filter((n) => n.kind.type === "feedback").length,
    1,
  );
  assert.equal(
    doc.graph.nodes.filter((n) => n.kind.type === "state").length,
    1,
  );
  removeEditorFeedback(doc);
  copyControlBinding(doc, [catalog], device, "key-1", device, "key-2", false);
  const view = layout(doc);
  view.devices[0].controls["key-2"] = { showResult: true };
  putLayout(doc, view);
  syncEditorFeedback(doc, [catalog]);
  assert.equal(
    doc.graph.nodes.filter((n) => n.kind.type === "feedback").length,
    2,
  );
  const ids = new Set(doc.graph.nodes.map((n) => n.id));
  assert.equal(ids.size, doc.graph.nodes.length);
  assert.ok(
    doc.graph.edges.every(
      (e) => ids.has(e.source.nodeId) && ids.has(e.target.nodeId),
    ),
  );
  assert.throws(() => syncEditorFeedback(doc, [catalog]));
});
test("editing never drops another workflow attached to the display", () => {
  const { doc, catalog } = fixture();
  syncEditorFeedback(doc, [catalog]);
  const display = doc.graph.nodes.find((n) => n.kind.type === "feedback")!;
  doc.graph.edges.push({
    id: "external",
    kind: "data",
    source: { nodeId: "foreign", portId: "value" },
    target: { nodeId: display.id, portId: "input" },
    extensions: {},
  });
  const before = structuredClone(doc);
  assert.throws(() => removeEditorFeedback(doc));
  assert.deepEqual(doc, before);
});
