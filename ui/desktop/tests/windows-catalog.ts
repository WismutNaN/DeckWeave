// SPDX-License-Identifier: GPL-3.0-or-later
import { readFileSync } from "node:fs";
import type { Catalog, Schema, Node } from "../src/editor/types";
export function windowsCatalog(name: string): {
  catalog: Catalog;
  templates: Node[];
} {
  const manifest = JSON.parse(
    readFileSync(
      new URL(
        `../../../plugins/platform-windows/package/${name}.template.json`,
        import.meta.url,
      ),
      "utf8",
    ),
  );
  const raw = manifest["org.deckweave.automationCatalog"];
  return {
    catalog: {
      package: {
        packageId: manifest.packageId,
        version: manifest.version,
        artifactSha256: "a".repeat(64),
      },
      manifestSha256: "b".repeat(64),
      translations: null,
      endpointMigrations: manifest["org.deckweave.endpointMigrations"],
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
    },
    templates: manifest["org.deckweave.deviceTemplates"] ?? [],
  };
}
