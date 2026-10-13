// SPDX-License-Identifier: GPL-3.0-or-later
// One metadata source for bundled descriptors, schema files and the indexed package.
export const packageId = "org.deckweave.virtual";
const version = { major: 1, minor: 0 };
export const ref = (id) => ({ id: `${packageId}.${id}`, version });
const port = (portId, kind, schema, tags = []) => ({
  portId,
  kind,
  schema: ref(schema),
  direction: kind.endsWith("Out") ? "output" : "input",
  semantics:
    kind === "EventOut" ? "event" : kind === "CommandIn" ? "command" : "value",
  compatibleVersions: { minimum: version, maximum: version },
  requiredPermissions: [],
  availability: { status: "available" },
  lifetime: "always",
  descriptorRevision: 1,
  tags,
});
const elements = Array.from({ length: 15 }, (_, i) => ({
  elementId: `key-${i + 1}`,
  label: String(i + 1),
  capabilities: [
    {
      capabilityId: "button",
      kind: "org.deckweave.input.button",
      ports: [
        port("pressed", "EventOut", "boolean", ["org.deckweave.virtual.press"]),
      ],
    },
    {
      capabilityId: "display",
      kind: "org.deckweave.display.text",
      ports: [
        port("text", "ValueIn", "text"),
        port("displayed", "ValueOut", "text", [
          "org.deckweave.virtual.display",
        ]),
      ],
    },
  ],
}));
export const descriptors = [
  {
    nodeId: "virtual-node",
    kind: "local",
    label: "Virtual Studio",
    descriptorRevision: 1,
    devices: ["a", "b"].map((id) => ({
      providerId: "virtual",
      identity: `deck-${id}`,
      kind: "virtual",
      label: `Virtual Deck ${id.toUpperCase()}`,
      elements,
      "org.deckweave.controlLayout": {
        version: 1,
        width: 5,
        height: 3,
        elements: Object.fromEntries(
          elements.map((e, i) => [
            e.elementId,
            { x: i % 5, y: Math.floor(i / 5), width: 1, height: 1 },
          ]),
        ),
      },
    })),
    services: [
      {
        serviceId: "text",
        providerId: "virtual",
        label: "Text",
        elements: [
          {
            elementId: "echo",
            label: "Echo",
            capabilities: [
              {
                capabilityId: "action",
                kind: "org.deckweave.action.text",
                ports: [port("invoke", "CommandIn", "message")],
              },
            ],
          },
        ],
      },
    ],
  },
];
export const schemaDefinitions = {
  boolean: { type: "boolean" },
  unit: { type: "null" },
  text: { type: "string", maxLength: 2048 },
  message: {
    type: "object",
    properties: { text: { type: "string", maxLength: 2048 } },
    required: ["text"],
    additionalProperties: false,
  },
  settings: {
    type: "object",
    properties: { prefix: { type: "string", maxLength: 80 } },
    required: ["prefix"],
    additionalProperties: false,
  },
};
const shapes = {
  boolean: "boolean",
  unit: "unit",
  text: { structured: { type: "string", maximumBytes: 2048 } },
  message: {
    structured: {
      type: "record",
      fields: { text: { schema: { type: "string", maximumBytes: 2048 } } },
    },
  },
  settings: {
    structured: {
      type: "record",
      fields: { prefix: { schema: { type: "string", maximumBytes: 80 } } },
    },
  },
};
export function manifest(sha256, os, architecture, binary, schemas) {
  const runtimeRequirement = (contributionId) => ({
    packageId,
    packageVersion: "0.1.0",
    artifactSha256: sha256,
    runtimeGroupId: "control",
    contributionId,
  });
  const endpoints = [];
  for (const device of descriptors[0].devices)
    for (const element of device.elements)
      for (const capability of element.capabilities)
        for (const p of capability.ports)
          endpoints.push({
            endpoint: {
              packageId,
              nodeId: "virtual-node",
              target: {
                kind: "device",
                providerId: "virtual",
                deviceIdentity: device.identity,
              },
              port: {
                elementId: element.elementId,
                capabilityId: capability.capabilityId,
                portId: p.portId,
              },
            },
            port: p,
            runtimeRequirement: runtimeRequirement("devices"),
            operation: null,
          });
  endpoints.push({
    endpoint: {
      packageId,
      nodeId: "virtual-node",
      target: { kind: "service", providerId: "virtual", serviceId: "text" },
      port: { elementId: "echo", capabilityId: "action", portId: "invoke" },
    },
    port: descriptors[0].services[0].elements[0].capabilities[0].ports[0],
    runtimeRequirement: runtimeRequirement("text"),
    operation: {
      operationId: "echo",
      version: 1,
      inputSchema: ref("message"),
      resultSchema: ref("text"),
      errorSchema: ref("text"),
      resources: [],
      execution: {
        kind: "short",
        progress: false,
        cancellation: false,
        reconciliation: false,
      },
      limits: {
        deadlineMs: 5000,
        maximumInputBytes: 8192,
        maximumOutputBytes: 16384,
        maximumConcurrentInvocations: 16,
        maximumQueueDepth: 64,
        maximumPerMinute: 600,
      },
      dataPolicy: {
        sensitiveFields: [],
        externalRecipients: [],
        traceAllowed: false,
        retentionSeconds: 0,
      },
    },
  });
  return {
    manifestVersion: 1,
    packageId,
    version: "0.1.0",
    license: "GPL-3.0-or-later",
    source: {
      repository: "https://github.com/WismutNaN/DeckWeave",
      revision: "source-tree",
      provenance:
        "First-party virtual device; no OS effects or external services.",
    },
    artifacts: [
      { id: "native", os, architecture, entrypoint: `bin/${binary}`, sha256 },
    ],
    runtimeGroups: [
      { id: "control", runtimeClass: "isolated_control", artifactId: "native" },
    ],
    contributions: ["devices", "text"].map((id) => ({
      id,
      type:
        id === "devices"
          ? "org.deckweave.device_provider"
          : "org.deckweave.action_provider",
      runtimeGroupId: "control",
      descriptor: {},
    })),
    protocolRange: { minimum: version, maximum: { major: 1, minor: 1 } },
    permissions: [],
    schemas,
    "org.deckweave.deviceTemplates": descriptors,
    "org.deckweave.automationCatalog": {
      formatVersion: 1,
      endpoints,
      schemas: Object.entries(shapes).map(([id, valueKind]) => ({
        schema: ref(id),
        valueKind,
        unit: null,
        range: null,
      })),
    },
    "org.deckweave.translations": {
      defaultLocale: "en",
      messages: {
        en: { echo: "Pass text", text: "Text settings", prefix: "Prefix" },
        ru: {
          echo: "Передать текст",
          text: "Настройки текста",
          prefix: "Префикс",
        },
      },
    },
    "org.deckweave.instanceTemplates": [
      {
        contributionId: "text",
        settingsSchema: ref("settings"),
        labelKey: "text",
        requiredFeatures: { "org.deckweave.configured_dispatch": 1 },
      },
    ],
  };
}
