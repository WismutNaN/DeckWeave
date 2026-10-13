// SPDX-License-Identifier: GPL-3.0-or-later
import { spawnSync } from "node:child_process";
import crypto from "node:crypto";
import fs from "node:fs";
import path from "node:path";
import { packageTemplates } from "../plugins/platform-windows/package-set.mjs";

for (const { name, manifest } of packageTemplates()) {
  const actual = JSON.parse(
    fs.readFileSync(
      `plugins/platform-windows/package/${name}.template.json`,
      "utf8",
    ),
  );
  if (JSON.stringify(actual) !== JSON.stringify(manifest))
    throw new Error(`Windows ${name} package template is stale`);
}

import { descriptors } from "../plugins/virtual-device/metadata.mjs";
import {
  keyboardDescriptor,
  addKeyboardCatalog,
} from "../plugins/platform-windows/keyboard-metadata.mjs";
const keyboardRoot = "plugins/platform-windows/";
const windowsManifest = JSON.parse(
  fs.readFileSync(
    keyboardRoot + "package/deckweave-plugin.template.json",
    "utf8",
  ),
);
const updatedManifest = structuredClone(windowsManifest);
addKeyboardCatalog(updatedManifest);
if (
  JSON.stringify(updatedManifest) !== JSON.stringify(windowsManifest) ||
  JSON.stringify(
    JSON.parse(
      fs.readFileSync(
        keyboardRoot + "assets/input-observe-descriptors.json",
        "utf8",
      ),
    ),
  ) !== JSON.stringify(keyboardDescriptor)
)
  throw new Error("Windows keyboard metadata is stale");
const root = process.cwd();
if (
  JSON.stringify(
    JSON.parse(
      fs.readFileSync(
        path.join(root, "plugins/virtual-device/assets/descriptors.json"),
        "utf8",
      ),
    ),
  ) !== JSON.stringify(descriptors)
) {
  console.error(
    "Bundled virtual descriptors are stale. Run npm run prepare:desktop.",
  );
  process.exitCode = 1;
}
const generatedRoot = path.join(
  root,
  "protocol",
  "typescript",
  "src",
  "generated",
);

function files(directory) {
  return fs
    .readdirSync(directory, { withFileTypes: true })
    .flatMap((entry) => {
      const location = path.join(directory, entry.name);
      return entry.isDirectory() ? files(location) : [location];
    })
    .sort();
}

function snapshot() {
  return new Map(
    files(generatedRoot).map((file) => [
      path.relative(generatedRoot, file),
      crypto.createHash("sha256").update(fs.readFileSync(file)).digest("hex"),
    ]),
  );
}

const before = snapshot();
const result = spawnSync(
  process.execPath,
  [path.join(root, "scripts", "generate-contracts.mjs")],
  {
    cwd: root,
    stdio: "inherit",
  },
);
if (result.status !== 0) process.exit(result.status ?? 1);
const after = snapshot();
if (JSON.stringify([...before]) !== JSON.stringify([...after])) {
  console.error(
    "Generated contracts were stale. Regenerated files are left in the worktree.",
  );
  process.exitCode = 1;
} else {
  console.log(`Generated contracts are stable (${after.size} files).`);
}
