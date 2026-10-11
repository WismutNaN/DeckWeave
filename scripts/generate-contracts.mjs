// SPDX-License-Identifier: GPL-3.0-or-later
import { spawnSync } from "node:child_process";
import fs from "node:fs/promises";
import path from "node:path";
import { compileFromFile } from "json-schema-to-typescript";

const root = process.cwd();
const executable = path.join(
  root,
  "node_modules",
  "@bufbuild",
  "buf",
  "bin",
  "buf",
);
const generated = spawnSync(process.execPath, [executable, "generate"], {
  cwd: root,
  stdio: "inherit",
});
if (generated.status !== 0) process.exit(generated.status ?? 1);

const protobufOutput = path.join(
  root,
  "protocol",
  "typescript",
  "src",
  "generated",
  "deckweave",
  "v1",
  "control_pb.ts",
);
const protobufSource = (await fs.readFile(protobufOutput, "utf8"))
  .replaceAll("\r\n", "\n")
  .trimEnd();
const license = "// SPDX-License-Identifier: MIT OR Apache-2.0\n\n";
const licensedProtobuf = protobufSource.startsWith(
  "// SPDX-License-Identifier:",
)
  ? protobufSource
  : license + protobufSource;
await fs.writeFile(protobufOutput, `${licensedProtobuf}\n`, "utf8");

const schemaRoot = path.join(root, "protocol", "schema");
const outputRoot = path.join(
  root,
  "protocol",
  "typescript",
  "src",
  "generated",
  "documents",
);
await fs.mkdir(outputRoot, { recursive: true });
const files = (await fs.readdir(schemaRoot))
  .filter((file) => file.endsWith(".schema.json"))
  .sort();
for (const file of files) {
  const source = path.join(schemaRoot, file);
  const output = path.join(outputRoot, file.replace(".schema.json", ".ts"));
  const types = await compileFromFile(source, {
    bannerComment:
      "// SPDX-License-Identifier: MIT OR Apache-2.0\n// Generated from JSON Schema. Do not edit by hand.",
    cwd: schemaRoot,
    style: { singleQuote: false, semi: true, tabWidth: 2 },
  });
  await fs.writeFile(output, types.replaceAll("\r\n", "\n"), "utf8");
}
