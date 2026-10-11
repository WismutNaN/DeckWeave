// SPDX-License-Identifier: GPL-3.0-or-later
import { spawnSync } from "node:child_process";
import crypto from "node:crypto";
import fs from "node:fs";
import path from "node:path";

const root = process.cwd();
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
