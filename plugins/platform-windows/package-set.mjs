// SPDX-License-Identifier: GPL-3.0-or-later
import fs from "node:fs";

export const windowsPackages = [
  {
    name: "audio",
    contribution: "audio-control",
    scopes: ["windows.audio.observe", "windows.audio.control"],
  },
  {
    name: "media",
    contribution: "media-control",
    scopes: ["windows.media.control"],
  },
  {
    name: "window",
    contribution: "windows-control",
    scopes: ["windows.window.observe", "windows.window.activate"],
  },
  {
    name: "applications",
    contribution: "applications-control",
    scopes: ["windows.process.launch", "windows.shell.open"],
  },
  {
    name: "keyboard",
    contribution: "keyboard-observe",
    scopes: ["input.observe"],
  },
];

export function packageForEndpoint(endpoint) {
  if (endpoint.target.kind === "device") return "keyboard";
  if (endpoint.port.elementId === "audio") return "audio";
  if (endpoint.port.elementId === "media") return "media";
  if (endpoint.port.elementId === "applications")
    return ["launch", "shell-open"].includes(endpoint.port.capabilityId)
      ? "applications"
      : "window";
  throw new Error("Unknown Windows endpoint family");
}

export function packageTemplates() {
  const legacy = JSON.parse(
    fs.readFileSync(
      new URL("./package/deckweave-plugin.template.json", import.meta.url),
      "utf8",
    ),
  );
  return windowsPackages.map(({ name, contribution, scopes }) => {
    const manifest = structuredClone(legacy);
    manifest.packageId = `org.deckweave.platform.windows.${name}`;
    manifest["org.deckweave.endpointMigrations"] = [
      { packageId: legacy.packageId, version: legacy.version },
    ];
    manifest.artifacts = [
      {
        id: `windows-x86_64-${name}`,
        os: "windows",
        architecture: "x86_64",
        entrypoint: `bin/deckweave-windows-${name}.exe`,
        sha256: "0".repeat(64),
      },
    ];
    manifest.runtimeGroups = [
      {
        id: name,
        runtimeClass: "isolated_control",
        artifactId: manifest.artifacts[0].id,
      },
    ];
    const source = legacy.contributions.find(
      (c) =>
        c.id === (name === "keyboard" ? "keyboard-observe" : "windows-control"),
    );
    manifest.contributions = [
      { ...structuredClone(source), id: contribution, runtimeGroupId: name },
    ];
    manifest.contributions[0].descriptor.mode = "native_adapter";
    manifest.permissions = manifest.permissions
      .filter((p) => scopes.includes(p.scope))
      .map((p) => ({ ...p, runtimeGroupIds: [name] }));
    const catalog = manifest["org.deckweave.automationCatalog"];
    catalog.endpoints = catalog.endpoints.filter(
      (e) => packageForEndpoint(e.endpoint) === name,
    );
    for (const e of catalog.endpoints) {
      e.endpoint.packageId = manifest.packageId;
      e.runtimeRequirement.packageId = manifest.packageId;
      e.runtimeRequirement.runtimeGroupId = name;
      e.runtimeRequirement.contributionId = contribution;
    }
    if (name !== "keyboard") delete manifest["org.deckweave.deviceTemplates"];
    // Schemas keep stable IDs shared with the legacy adapters. Only dependencies
    // between live services, not identical data formats, require another package.
    return { name, manifest };
  });
}
