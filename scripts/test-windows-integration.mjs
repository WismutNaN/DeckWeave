// SPDX-License-Identifier: GPL-3.0-or-later
import { spawnSync } from "node:child_process";
import path from "node:path";
import { bundleWindows } from "./bundle-windows.mjs";
if (process.platform !== "win32")
  throw new Error("This integration fixture requires native Windows");
const root = process.cwd();
function cargo(args, env = process.env) {
  const result = spawnSync("cargo", args, { cwd: root, stdio: "inherit", env });
  if (result.error) throw result.error;
  if (result.status !== 0) process.exit(result.status ?? 1);
}
cargo(["build", "--locked", "-p", "deckweave-platform-windows", "--bins"]);
const metadata = spawnSync(
  "cargo",
  ["metadata", "--format-version", "1", "--no-deps"],
  { cwd: root, encoding: "utf8" },
);
if (metadata.status !== 0) throw new Error("Cargo metadata failed");
const target = path.join(JSON.parse(metadata.stdout).target_directory, "debug");
bundleWindows(
  root,
  target,
  process.arch === "x64"
    ? "x86_64"
    : process.arch === "arm64"
      ? "aarch64"
      : process.arch,
);
cargo(
  [
    "test",
    "--locked",
    "-p",
    "deckweave-plugin-supervisor",
    "--test",
    "production_apply",
    "windows_e2e::",
    "--",
    "--ignored",
    "--nocapture",
  ],
  {
    ...process.env,
    DECKWEAVE_WINDOWS_E2E_BUNDLES: path.join(target, "bundled"),
  },
);
