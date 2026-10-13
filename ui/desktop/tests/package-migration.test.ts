// SPDX-License-Identifier: GPL-3.0-or-later
import test from "node:test";
import assert from "node:assert/strict";
import { windowsCatalog } from "./windows-catalog";
import { previewData } from "../src/editor/preview";
import { layout, defaultBackground } from "../src/editor/appearance";
import { appendAction } from "../src/editor/bindings";
import {
  migrationSources,
  planEndpointMigration,
} from "../src/editor/package-migration";
const names = ["audio", "media", "window", "applications", "keyboard"];
function fixture() {
  const { draft, catalog: virtual } = previewData();
  const { catalog: legacy, templates } = windowsCatalog("deckweave-plugin");
  const targets = names.map((n) => windowsCatalog(n).catalog),
    catalogs = [virtual, legacy, ...targets];
  const device = layout(draft.document).devices[0];
  for (const [index, e] of legacy.endpoints
    .filter((e) => e.operation)
    .entries()) {
    appendAction(
      draft.document,
      catalogs,
      device,
      "key-1",
      { ...e, catalog: legacy, title: "test" },
      `action-${index}`,
    );
  }
  const template = templates[0];
  const keyboard = {
    id: "keyboard",
    package: legacy.package,
    nodeId: template.nodeId,
    descriptor: template.devices[0],
    x: 0,
    y: 0,
    appearance: defaultBackground(),
    controls: {},
  };
  layout(draft.document).devices.push(keyboard);
  const key = keyboard.descriptor.elements[0].elementId;
  const action = legacy.endpoints.find((e) => e.operation)!;
  appendAction(
    draft.document,
    catalogs,
    keyboard,
    key,
    { ...action, catalog: legacy, title: "test" },
    "keyboard",
  );
  draft.packageLock = [{ ...legacy.package, source: null }];
  return { draft, catalogs, legacy, targets };
}

test("the legacy Windows package migrates all endpoint families atomically and preserves user configuration", () => {
  const { draft, catalogs, legacy } = fixture(),
    before = structuredClone(draft);
  const planned = planEndpointMigration(
    draft.document,
    catalogs,
    legacy.package.packageId,
  );
  assert.equal(planned.packages.length, 5);
  assert.deepEqual(migrationSources(draft.document, catalogs), [
    legacy.package.packageId,
  ]);
  assert.deepEqual(draft, before);
  assert.ok(
    planned.document.graph.nodes.every(
      (n) => n.kind.endpoint?.packageId !== legacy.package.packageId,
    ),
  );
  assert.equal(
    layout(planned.document).devices[1].package.packageId,
    legacy.package.packageId + ".keyboard",
  );
  assert.deepEqual(
    planned.document.graph.actionArguments,
    draft.document.graph.actionArguments,
  );
  assert.deepEqual(planned.document.graph.edges, draft.document.graph.edges);
  assert.deepEqual(
    layout(planned.document).devices[1].appearance,
    layout(draft.document).devices[1].appearance,
  );
  assert.deepEqual(migrationSources(planned.document, catalogs), []);
});

test("missing, ambiguous or changed contracts never produce a partial migration", () => {
  for (const problem of [
    "missing",
    "ambiguous",
    "schema",
    "operation",
    "version",
  ]) {
    const { draft, catalogs, legacy, targets } = fixture(),
      before = structuredClone(draft.document);
    const target = targets[0];
    if (problem === "missing") catalogs.splice(catalogs.indexOf(target), 1);
    if (problem === "ambiguous") catalogs.push(structuredClone(target));
    if (problem === "schema")
      target.schemas.find((s) => s.schema.id.endsWith("volume_scalar"))!.unit =
        "percent";
    if (problem === "operation")
      target.endpoints.find((e) => e.operation)!.operation!.operationId =
        "changed";
    if (problem === "version") target.endpointMigrations![0].version = "99.0.0";
    assert.throws(
      () =>
        planEndpointMigration(
          draft.document,
          catalogs,
          legacy.package.packageId,
        ),
      problem,
    );
    assert.deepEqual(draft.document, before);
  }
});

test("configured plugin instances require their own adapter and are never silently moved with credentials", () => {
  const { draft, catalogs, legacy } = fixture();
  draft.document.pluginInstances = {
    account: {
      instanceId: "account",
      packageId: legacy.package.packageId,
      contributionId: "windows-control",
      settingsRevision: 1,
      settingsSchema: legacy.schemas[0].schema,
      settings: {},
      credentialRefs: { token: "private" },
    },
  };
  const before = structuredClone(draft.document);
  assert.throws(() =>
    planEndpointMigration(draft.document, catalogs, legacy.package.packageId),
  );
  assert.deepEqual(draft.document, before);
});
