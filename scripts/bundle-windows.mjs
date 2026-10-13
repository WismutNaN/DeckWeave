// SPDX-License-Identifier: GPL-3.0-or-later
import fs from "node:fs";
import path from "node:path";
import { createHash } from "node:crypto";
import { packageTemplates } from "../plugins/platform-windows/package-set.mjs";

export function bundleWindows(root, target, architecture) {
  const hash = (bytes) => createHash("sha256").update(bytes).digest("hex");
  for (const { name, manifest } of packageTemplates()) {
    const destination = path.join(target, "bundled", `windows-${name}`);
    fs.mkdirSync(path.join(destination, "bin"), { recursive: true });
    fs.mkdirSync(path.join(destination, "schemas"), { recursive: true });
    const artifact = manifest.artifacts[0];
    artifact.architecture = architecture;
    artifact.id = `windows-${architecture}-${name}`;
    manifest.runtimeGroups[0].artifactId = artifact.id;
    const executable = path.join(target, path.basename(artifact.entrypoint));
    fs.copyFileSync(executable, path.join(destination, artifact.entrypoint));
    artifact.sha256 = hash(fs.readFileSync(executable));
    for (const entry of manifest["org.deckweave.automationCatalog"].endpoints)
      entry.runtimeRequirement.artifactSha256 = artifact.sha256;
    for (const schema of manifest.schemas) {
      const bytes = fs.readFileSync(
        path.join(root, "plugins/platform-windows", schema.path),
      );
      fs.writeFileSync(path.join(destination, schema.path), bytes);
      schema.sha256 = hash(bytes);
    }
    fs.writeFileSync(
      path.join(destination, "deckweave-plugin.json"),
      JSON.stringify(manifest, null, 2) + "\n",
    );
    fs.copyFileSync(
      path.join(root, "LICENSE"),
      path.join(destination, "LICENSE"),
    );
    console.log(`Bundled Windows ${name}: ${destination}`);
  }
}
