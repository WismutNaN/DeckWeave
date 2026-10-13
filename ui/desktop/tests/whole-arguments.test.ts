// SPDX-License-Identifier: GPL-3.0-or-later
import test from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { previewData } from "../src/editor/preview";
import { editorFixture } from "./fixture";
import { layout } from "../src/editor/appearance";
import {
  actionShape,
  appendAction,
  bindingNodes,
  steps,
  removeLastAction,
} from "../src/editor/bindings";
import {
  copyControlBinding,
  reorderAction,
  removeControlBinding,
} from "../src/editor/binding-edits";
import { valueShape } from "../src/editor/value-shape";
import type { Catalog, Schema } from "../src/editor/types";

test("all bundled Windows actions have an assignable form, including numeric, boolean, choice and unit inputs", () => {
  const { draft, catalog: source } = previewData(),
    device = layout(draft.document).devices[0];
  const seen = new Set<string>();
  for (const group of ["audio", "media", "window", "applications"]) {
    const manifest = JSON.parse(
      readFileSync(
        new URL(
          `../../../plugins/platform-windows/package/${group}.template.json`,
          import.meta.url,
        ),
        "utf8",
      ),
    );
    const raw = manifest["org.deckweave.automationCatalog"];
    const catalog: Catalog = {
      package: {
        packageId: manifest.packageId,
        version: manifest.version,
        artifactSha256: "a".repeat(64),
      },
      manifestSha256: "b".repeat(64),
      translations: null,
      schemas: raw.schemas.map((s: { valueKind: Schema["shape"] }) => ({
        ...s,
        shape: s.valueKind,
      })),
      endpoints: raw.endpoints.map(
        (e: {
          runtimeRequirement: {
            contributionId: string;
            runtimeGroupId: string;
          };
        }) => ({ ...e, ...e.runtimeRequirement }),
      ),
    };
    for (const endpoint of catalog.endpoints.filter((e) => e.operation)) {
      const action = {
        ...endpoint,
        catalog,
        title: endpoint.operation!.operationId,
      };
      const shape = actionShape(action);
      assert.ok(shape, action.title);
      seen.add(shape.type);
      const document = structuredClone(draft.document);
      const id = appendAction(
        document,
        [source, catalog],
        device,
        "key-1",
        action,
        action.title,
      );
      if (shape.type !== "record") {
        assert.equal(document.schemaVersion.minor, 1);
        assert.ok(document.graph.actionArguments![id].value);
        assert.deepEqual(document.graph.actionArguments![id].fields, {});
      }
    }
  }
  for (const type of ["number", "boolean", "string", "null", "record"])
    assert.ok(seen.has(type), type);
});

test("copy and reorder preserve whole-input dependencies, including the original control event", () => {
  const { draft, catalog } = editorFixture(),
    doc = draft.document,
    device = layout(doc).devices[0];
  const [first, second] = steps(doc, device.id, "key-1");
  const source = bindingNodes(doc, device.id, "key-1").find(
    (n) => n.kind.type === "source",
  )!;
  doc.schemaVersion.minor = 1;
  doc.graph.actionArguments![first.id] = {
    triggerPort: "trigger",
    fields: {},
    value: { type: "run_result", nodeId: source.id, pointer: "" },
  };
  doc.graph.actionArguments![second.id] = {
    triggerPort: "trigger",
    fields: {},
    value: { type: "run_result", nodeId: first.id, pointer: "" },
  };
  assert.throws(() =>
    reorderAction(doc, [catalog], device.id, "key-1", second.id, first.id),
  );
  copyControlBinding(doc, [catalog], device, "key-1", device, "key-2", false);
  const cloned = steps(doc, device.id, "key-2"),
    clonedSource = bindingNodes(doc, device.id, "key-2").find(
      (n) => n.kind.type === "source",
    )!;
  assert.deepEqual(doc.graph.actionArguments![cloned[0].id].value, {
    type: "run_result",
    nodeId: clonedSource.id,
    pointer: "",
  });
  assert.deepEqual(doc.graph.actionArguments![cloned[1].id].value, {
    type: "run_result",
    nodeId: cloned[0].id,
    pointer: "",
  });
  doc.graph.actionArguments![cloned[0].id].value = {
    type: "run_result",
    nodeId: second.id,
    pointer: "",
  };
  assert.throws(() => removeLastAction(doc, device.id, "key-1"));
  assert.throws(() => removeControlBinding(doc, device.id, "key-1"));
});

test("a parameterless action can follow a result-bearing step without inventing a conversion", () => {
  const { draft, catalog } = editorFixture(),
    doc = draft.document,
    device = layout(doc).devices[0];
  const unit = structuredClone(catalog.endpoints.find((e) => e.operation)!);
  const input = catalog.schemas.find((s) => s.shape === "unit")!.schema;
  unit.operation!.inputSchema = input;
  unit.operation!.operationId = "parameterless";
  unit.endpoint.port.portId = "parameterless";
  unit.port.portId = "parameterless";
  unit.port.schema = input;
  catalog.endpoints.push(unit);
  const id = appendAction(
    doc,
    [catalog],
    device,
    "key-1",
    { ...unit, catalog, title: "unit" },
    "unit",
  );
  assert.deepEqual(doc.graph.actionArguments![id].value, {
    type: "constant",
    value: null,
  });
  const first = steps(doc, device.id, "key-1")[0];
  reorderAction(doc, [catalog], device.id, "key-1", id, first.id);
  assert.equal(steps(doc, device.id, "key-1")[0].id, id);
});

test("numeric forms honor catalog ranges without dropping physical units from variables", () => {
  const schema: Schema = {
    schema: { id: "test", version: { major: 1, minor: 0 } },
    shape: { structured: { type: "number", minimum: -10, maximum: 10 } },
    unit: "ratio",
    range: { minimum: 0, maximum: 1 },
  };
  assert.equal(valueShape(schema), null);
  assert.deepEqual(valueShape(schema, true), {
    type: "number",
    minimum: 0,
    maximum: 1,
  });
});
