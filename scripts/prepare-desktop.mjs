// SPDX-License-Identifier: GPL-3.0-or-later
import fs from "node:fs";
import path from "node:path";
import { createHash } from "node:crypto";
import { execFileSync } from "node:child_process";
import { fileURLToPath } from "node:url";
import { bundleWindows } from "./bundle-windows.mjs";
import {
  descriptors,
  manifest,
  ref,
  schemaDefinitions,
} from "../plugins/virtual-device/metadata.mjs";
const root = fileURLToPath(new URL("../", import.meta.url));
const release =
  process.argv.includes("--release") ||
  (process.argv.includes("--tauri") && process.env.TAURI_ENV_DEBUG !== "true");
const asset = path.join(root, "plugins/virtual-device/assets/descriptors.json");
fs.mkdirSync(path.dirname(asset), { recursive: true });
const json = (value) => JSON.stringify(value, null, 2) + "\n";
if (
  !fs.existsSync(asset) ||
  fs.readFileSync(asset, "utf8") !== json(descriptors)
)
  fs.writeFileSync(asset, json(descriptors));
execFileSync(
  "cargo",
  [
    "build",
    "-p",
    "deckweave-virtual-device",
    ...(process.platform === "win32"
      ? ["-p", "deckweave-platform-windows"]
      : []),
    ...(release ? ["--release"] : []),
  ],
  { cwd: root, stdio: "inherit" },
);
const target = path.resolve(
  root,
  process.env.CARGO_TARGET_DIR ?? "target",
  release ? "release" : "debug",
);
const destination = path.join(target, "bundled", "virtual-device");
const binary = `deckweave-virtual-device${process.platform === "win32" ? ".exe" : ""}`;
const digest = (bytes) => createHash("sha256").update(bytes).digest("hex");
fs.mkdirSync(path.join(destination, "bin"), { recursive: true });
fs.mkdirSync(path.join(destination, "schemas"), { recursive: true });
fs.copyFileSync(
  path.join(target, binary),
  path.join(destination, "bin", binary),
);
const schemas = Object.entries(schemaDefinitions).map(([id, definition]) => {
  const bytes = json({
    $schema: "https://json-schema.org/draft/2020-12/schema",
    ...definition,
  });
  const schemaPath = `schemas/${id}.json`;
  fs.writeFileSync(path.join(destination, schemaPath), bytes);
  return { ...ref(id), path: schemaPath, sha256: digest(bytes) };
});
const os = { win32: "windows", darwin: "macos", linux: "linux" }[
  process.platform
];
const architecture = { x64: "x86_64", arm64: "aarch64" }[process.arch];
if (!os || !architecture) throw new Error("Unsupported bundle platform");
fs.writeFileSync(
  path.join(destination, "deckweave-plugin.json"),
  json(
    manifest(
      digest(fs.readFileSync(path.join(target, binary))),
      os,
      architecture,
      binary,
      schemas,
    ),
  ),
);
fs.copyFileSync(path.join(root, "LICENSE"), path.join(destination, "LICENSE"));
console.log(`Bundled virtual device: ${destination}`);
if (process.platform === "win32") bundleWindows(root, target, architecture);
