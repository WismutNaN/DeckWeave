// SPDX-License-Identifier: GPL-3.0-or-later
import { execFileSync } from "node:child_process";
import fs from "node:fs";
import path from "node:path";

import {
  normalizePath,
  validateLocalDependencies,
  validateSizeRatchet,
} from "./boundary-rules.mjs";

const architecture = JSON.parse(
  fs.readFileSync("scripts/architecture-baseline.json", "utf8"),
);

const metadata = JSON.parse(
  execFileSync(
    "cargo",
    ["metadata", "--format-version", "1", "--no-deps", "--locked"],
    { encoding: "utf8" },
  ),
);
const locals = metadata.packages.filter((p) =>
  metadata.workspace_members.includes(p.id),
);
const byName = new Map(locals.map((p) => [p.name, p]));
const errors = [];
function sourceFiles(dir) {
  return fs.readdirSync(dir, { withFileTypes: true }).flatMap((item) => {
    const file = path.join(dir, item.name);
    return item.isDirectory() ? sourceFiles(file) : [file];
  });
}
errors.push(...validateLocalDependencies(locals, architecture.packages));

const productionSources = new Map();
const crateRoots = new Set(
  locals.flatMap((pkg) =>
    pkg.targets
      .map((target) =>
        normalizePath(path.relative(process.cwd(), target.src_path)),
      )
      .filter((file) => file.includes("/src/")),
  ),
);

for (const pkg of locals) {
  const permissive = [
    "deckweave-protocol",
    "deckweave-virtual-plugin-example",
  ].includes(pkg.name);
  const license = permissive ? "MIT OR Apache-2.0" : "GPL-3.0-or-later";
  if (pkg.license !== license)
    errors.push(pkg.name + ": неправильная лицензия");
  for (const dep of pkg.dependencies) {
    if (dep.kind === "dev") continue;
    if (
      permissive &&
      byName.has(dep.name) &&
      byName.get(dep.name).license?.includes("GPL")
    )
      errors.push("Permissive protocol импортирует GPL");
    if (dep.name.includes("testkit"))
      errors.push("Production импортирует testkit: " + pkg.name);
  }
  const dir = path.dirname(pkg.manifest_path);
  for (const file of sourceFiles(path.join(dir, "src")).filter((f) =>
    f.endsWith(".rs"),
  )) {
    productionSources.set(
      normalizePath(path.relative(process.cwd(), file)),
      fs.readFileSync(file, "utf8"),
    );
    if (
      !fs
        .readFileSync(file, "utf8")
        .startsWith("// SPDX-License-Identifier: " + license)
    )
      errors.push(file + ": отсутствует SPDX");
  }
  const licenses = permissive ? ["LICENSE-MIT", "LICENSE-APACHE"] : ["LICENSE"];
  for (const file of licenses)
    if (!fs.existsSync(path.join(dir, file)))
      errors.push(pkg.name + ": отсутствует " + file);
}
errors.push(
  ...validateSizeRatchet({
    files: productionSources,
    crateRoots,
    limits: architecture.limits,
    exceptions: architecture.sizeExceptions,
  }),
);
for (const file of [
  "LICENSE",
  "THIRD_PARTY.md",
  "package-lock.json",
  "Cargo.lock",
]) {
  if (!fs.existsSync(file)) errors.push("Отсутствует " + file);
}
// Точный список expressions для текущего npm graph; новые требуют review.
const npmLicenses = new Set([
  "MIT",
  "Apache-2.0",
  "Apache-2.0 OR MIT",
  "ISC",
  "MPL-2.0",
  "BSD-3-Clause",
  "(Apache-2.0 AND BSD-3-Clause)",
  "Python-2.0",
]);
const npmLock = JSON.parse(fs.readFileSync("package-lock.json", "utf8"));
for (const [location, pkg] of Object.entries(npmLock.packages)) {
  if (!location.includes("node_modules/") || pkg.link) continue;
  if (!npmLicenses.has(pkg.license))
    errors.push(`${location}: npm license требует review: ${pkg.license}`);
  if (
    !pkg.resolved?.startsWith("https://registry.npmjs.org/") ||
    !pkg.integrity?.startsWith("sha512-")
  )
    errors.push(
      `${location}: неизвестный registry или отсутствует sha512 integrity`,
    );
}
if (errors.length) {
  console.error(errors.join("\n"));
  process.exitCode = 1;
} else
  console.log(
    `Границы: ${locals.length} packages с context ownership, ` +
      `${productionSources.size} production modules под size ratchet; ` +
      "protocol изолирован, production не зависит от testkit.",
  );
