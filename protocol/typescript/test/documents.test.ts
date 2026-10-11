// SPDX-License-Identifier: MIT OR Apache-2.0
import assert from "node:assert/strict";
import fs from "node:fs";
import { test } from "node:test";
import type { PluginManifest } from "../src/generated/documents/plugin-manifest.js";
import type { SettingsDocument } from "../src/generated/documents/settings-document.js";
import type { WorkspaceProfile } from "../src/generated/documents/workspace-profile.js";

function fixture<T>(name: string): T {
  return JSON.parse(
    fs.readFileSync(
      new URL(`../../fixtures/documents/${name}`, import.meta.url),
      "utf8",
    ),
  ) as T;
}

test("generated document types accept the shared extensible fixtures", () => {
  const manifest = fixture<PluginManifest>("plugin-manifest.future.json");
  const settings = fixture<SettingsDocument>("settings.future.json");
  const profile = fixture<WorkspaceProfile>("workspace-profile.future.json");
  assert.equal(manifest.packageId, "org.deckweave.fixture.keyboard");
  assert.equal(settings.schemaVersion.major, 1);
  assert.equal(profile.workspaceId, "workspace-fixture");
  assert.deepEqual(manifest.futureOptionalField, { mustSurvive: true });
});
