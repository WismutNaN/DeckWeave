// SPDX-License-Identifier: GPL-3.0-or-later
import fs from "node:fs";
const keys = JSON.parse(
  fs.readFileSync(new URL("./src/input/keys.json", import.meta.url), "utf8"),
);
const version = { major: 1, minor: 0 };
export const keyboardBoolean = { id: "org.deckweave.windows.boolean", version };
export const keyboardDescriptor = [
  {
    nodeId: "windows-input-node",
    kind: "local",
    label: "Windows input",
    descriptorRevision: 1,
    devices: [
      {
        identity: "system-keyboard-aggregate",
        providerId: "org.deckweave.platform.windows.input",
        kind: "virtual",
        label: "System keyboard",
        elements: [
          {
            elementId: "keyboard",
            label: "Keyboard observations",
            capabilities: [
              {
                capabilityId: "observe",
                kind: "org.deckweave.input.keyboard",
                ports: [
                  port("edge", {
                    id: "org.deckweave.windows.keyboard_observation",
                    version,
                  }),
                ],
              },
            ],
          },
          ...keys.map((key) => ({
            elementId: key.key,
            label: key.label,
            capabilities: [
              {
                capabilityId: "button",
                kind: "org.deckweave.input.button",
                ports: [port("pressed", keyboardBoolean)],
              },
            ],
          })),
        ],
        "org.deckweave.inputSource": {
          granularity: "system_aggregate",
          physicalIdentity: false,
          canObserve: true,
          canIntercept: false,
          passThrough: true,
          failOpen: true,
        },
        "org.deckweave.controlLayout": {
          version: 1,
          width: 24,
          height: 6,
          elements: Object.fromEntries(
            keys.map((k) => [
              k.key,
              { x: k.x, y: k.y, width: k.width, height: 1 },
            ]),
          ),
        },
      },
    ],
  },
];
function port(portId, schema) {
  return {
    portId,
    kind: "EventOut",
    direction: "output",
    semantics: "event",
    schema,
    compatibleVersions: { minimum: version, maximum: version },
    requiredPermissions: ["input.observe"],
    availability: { status: "available", reason: null },
    lifetime: "always",
    descriptorRevision: 1,
    tags: ["windows", "keyboard", "observe", "pass-through"],
  };
}
export function addKeyboardCatalog(manifest) {
  const catalog = manifest["org.deckweave.automationCatalog"];
  catalog.endpoints = catalog.endpoints.filter(
    (e) => e.runtimeRequirement.runtimeGroupId !== "input-observe",
  );
  catalog.schemas = catalog.schemas.filter(
    (s) =>
      ![
        keyboardBoolean.id,
        "org.deckweave.windows.keyboard_observation",
      ].includes(s.schema.id),
  );
  catalog.schemas.push(
    { schema: keyboardBoolean, valueKind: "boolean", unit: null, range: null },
    {
      schema: { id: "org.deckweave.windows.keyboard_observation", version },
      valueKind: "json",
      unit: null,
      range: null,
    },
  );
  const node = keyboardDescriptor[0],
    device = node.devices[0];
  for (const element of device.elements)
    for (const capability of element.capabilities)
      for (const port of capability.ports) {
        catalog.endpoints.push({
          endpoint: {
            packageId: manifest.packageId,
            nodeId: node.nodeId,
            target: {
              kind: "device",
              providerId: device.providerId,
              deviceIdentity: device.identity,
            },
            port: {
              elementId: element.elementId,
              capabilityId: capability.capabilityId,
              portId: port.portId,
            },
          },
          runtimeRequirement: {
            packageId: manifest.packageId,
            packageVersion: manifest.version,
            artifactSha256: manifest.artifacts[1].sha256,
            runtimeGroupId: "input-observe",
            contributionId: "keyboard-observe",
          },
          port,
          operation: null,
        });
      }
  manifest["org.deckweave.deviceTemplates"] = keyboardDescriptor;
}
