// SPDX-License-Identifier: GPL-3.0-or-later
import fs from "node:fs";
import path from "node:path";
import Ajv2020 from "ajv/dist/2020.js";
import addFormats from "ajv-formats";

const root = process.cwd();
const schemas = {
  "plugin-manifest": "plugin-manifest.future.json",
  "settings-document": "settings.future.json",
  "workspace-profile": "workspace-profile.future.json",
};
const ajv = new Ajv2020({ allErrors: true, strict: true });
addFormats(ajv);
let validateManifest;

for (const [name, fixture] of Object.entries(schemas)) {
  const schemaPath = path.join(
    root,
    "protocol",
    "schema",
    `${name}.schema.json`,
  );
  const fixturePath = path.join(
    root,
    "protocol",
    "fixtures",
    "documents",
    fixture,
  );
  const schema = JSON.parse(fs.readFileSync(schemaPath, "utf8"));
  const document = JSON.parse(fs.readFileSync(fixturePath, "utf8"));
  const validate = ajv.compile(schema);
  if (name === "plugin-manifest") validateManifest = validate;
  if (!validate(document))
    throw new Error(`${fixture}: ${ajv.errorsText(validate.errors)}`);
  const roundTrip = JSON.parse(JSON.stringify(document));
  if (JSON.stringify(roundTrip) !== JSON.stringify(document)) {
    throw new Error(`${fixture}: optional fields were not preserved`);
  }
}

const windowsPackageRoot = path.join(root, "plugins", "platform-windows");
const windowsManifest = JSON.parse(
  fs.readFileSync(
    path.join(windowsPackageRoot, "package", "deckweave-plugin.template.json"),
    "utf8",
  ),
);
if (!validateManifest || !validateManifest(windowsManifest)) {
  throw new Error(
    `platform-windows manifest: ${ajv.errorsText(validateManifest?.errors)}`,
  );
}
for (const entry of windowsManifest.schemas) {
  const schema = JSON.parse(
    fs.readFileSync(path.join(windowsPackageRoot, entry.path), "utf8"),
  );
  ajv.compile(schema);
}

const invalidManifest = { manifestVersion: 1, packageId: "not-namespaced" };
if (!validateManifest || validateManifest(invalidManifest)) {
  throw new Error("invalid manifest unexpectedly passed validation");
}

const validManifest = JSON.parse(
  fs.readFileSync(
    path.join(
      root,
      "protocol",
      "fixtures",
      "documents",
      "plugin-manifest.future.json",
    ),
    "utf8",
  ),
);
for (const unsafePath of [
  "../plugin.exe",
  "/plugin.exe",
  "C:/plugin.exe",
  "bin/../plugin.exe",
]) {
  const unsafe = structuredClone(validManifest);
  unsafe.artifacts[0].entrypoint = unsafePath;
  if (validateManifest(unsafe)) {
    throw new Error(
      `unsafe package path unexpectedly passed validation: ${unsafePath}`,
    );
  }
}
console.log(
  "JSON Schema: shared documents and Windows package schemas pass validation.",
);
