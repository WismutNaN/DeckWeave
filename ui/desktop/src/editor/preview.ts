// SPDX-License-Identifier: GPL-3.0-or-later
//! Учебные данные загружаются только по явному выбору; нет native effects.
import { copy, type EditorClient } from "./api";
import { defaultBackground, putLayout } from "./appearance";
import type {
  Catalog,
  Device,
  Draft,
  GraphDocument,
  Locale,
  MutationEnvelope,
  SchemaRef,
  Snapshot,
} from "./types";
const ref = (id: string): SchemaRef => ({
  id: `org.deckweave.preview.${id}`,
  version: { major: 1, minor: 0 },
});
export function previewData() {
  const pkg = {
    packageId: "org.deckweave.preview.studio",
    version: "1.0.0",
    artifactSha256: "a".repeat(64),
  };
  const boolean = ref("boolean"),
    record = ref("message"),
    unit = ref("unit"),
    text = ref("text");
  const port = (portId: string, kind: string, schema: SchemaRef) => ({
    portId,
    kind,
    schema,
    availability: { status: "available" },
  });
  const descriptor: Device = {
    identity: "virtual-deck",
    providerId: "preview",
    kind: "virtual",
    label: "Studio Deck",
    elements: Array.from({ length: 15 }, (_, i) => ({
      elementId: `key-${i + 1}`,
      label: String(i + 1),
      capabilities: [
        {
          capabilityId: "button",
          kind: "org.deckweave.input.button",
          ports: [port("pressed", "EventOut", boolean)],
        },
      ],
    })),
    "org.deckweave.controlLayout": {
      version: 1,
      width: 5,
      height: 3,
      elements: Object.fromEntries(
        Array.from({ length: 15 }, (_, i) => [
          `key-${i + 1}`,
          { x: i % 5, y: Math.floor(i / 5), width: 1, height: 1 },
        ]),
      ),
    },
  };
  const catalog: Catalog = {
    package: pkg,
    manifestSha256: "b".repeat(64),
    translations: {
      defaultLocale: "en",
      messages: {
        ru: {
          "show-text": "Показать текст",
          "create-note": "Создать заметку",
          "copy-text": "Передать текст",
        },
        en: {
          "show-text": "Show text",
          "create-note": "Create a note",
          "copy-text": "Pass text",
        },
      },
    },
    schemas: [
      { schema: boolean, shape: "boolean", unit: null, range: null },
      { schema: unit, shape: "unit", unit: null, range: null },
      {
        schema: text,
        shape: { structured: { type: "string", maximumBytes: 2048 } },
        unit: null,
        range: null,
      },
      {
        schema: record,
        shape: {
          structured: {
            type: "record",
            fields: {
              text: { schema: { type: "string", maximumBytes: 2048 } },
              title: {
                schema: { type: "string", maximumBytes: 80 },
                optional: true,
              },
            },
          },
        },
        unit: null,
        range: null,
      },
    ],
    endpoints: [],
  };
  catalog.endpoints = descriptor.elements.map((e) => ({
    endpoint: {
      packageId: pkg.packageId,
      nodeId: "preview-node",
      target: {
        kind: "device",
        providerId: "preview",
        deviceIdentity: descriptor.identity,
      },
      port: {
        elementId: e.elementId,
        capabilityId: "button",
        portId: "pressed",
      },
    },
    contributionId: "device",
    runtimeGroupId: "preview",
    port: e.capabilities[0].ports[0],
    operation: null,
  }));
  for (const id of ["show-text", "create-note", "copy-text"])
    catalog.endpoints.push({
      endpoint: {
        packageId: pkg.packageId,
        nodeId: "preview-node",
        target: { kind: "service", providerId: "preview", serviceId: "text" },
        port: { elementId: id, capabilityId: "action", portId: "invoke" },
      },
      contributionId: "text",
      runtimeGroupId: "preview",
      port: port("invoke", "CommandIn", record),
      operation: {
        operationId: id,
        inputSchema: record,
        resultSchema: text,
        errorSchema: text,
      },
    });
  const document: GraphDocument = {
    schemaVersion: { major: 1, minor: 0 },
    graph: { nodes: [], edges: [], extensions: {} },
    layout: { nodes: {}, extensions: {} },
    extensions: {},
  };
  putLayout(document, {
    version: 1,
    devices: [
      {
        id: "preview-deck",
        package: pkg,
        nodeId: "preview-node",
        descriptor,
        x: 0,
        y: 0,
        appearance: defaultBackground(),
        controls: {},
      },
    ],
  });
  const draft: Draft = {
    draftId: "preview-draft",
    workspaceId: "preview",
    baseRevisionId: null,
    version: 1,
    packageLock: [],
    document,
  };
  const snapshot: Snapshot = {
    registryRevision: 1,
    installed: [
      {
        package: pkg,
        templates: [
          {
            nodeId: "preview-node",
            descriptorRevision: 1,
            devices: [descriptor],
          },
        ],
      },
    ],
    nodes: [],
    runtimes: [],
    active: null,
  };
  return { draft, snapshot, catalog };
}
export function previewClient(): EditorClient {
  const data = previewData();
  let draft = data.draft,
    locale: Locale = { revision: 1, requestedLocale: "ru", timeZone: "UTC" };
  let activeDocument: GraphDocument | null = null;
  const receipts = new Map<string, { payload: string; result: unknown }>();
  return {
    open: async () => copy(draft),
    window: async () => ({ dedupEpoch: "preview-window" }),
    locale: async () => copy(locale),
    reconcileLocale: async () => ({
      context: copy(locale),
      deliveryPending: false,
    }),
    changeLocale: async (expected, next) => {
      if (expected !== locale.revision) throw { code: "revision_conflict" };
      locale = next;
      return { context: copy(locale), deliveryPending: false };
    },
    async query<T>(query: Record<string, unknown>): Promise<T> {
      const values: Record<string, unknown> = {
        active_document: {
          selection: data.snapshot.active,
          document: activeDocument,
        },
        editor_snapshot: data.snapshot,
        plugin_catalog: data.catalog,
        draft,
        plan_apply: {
          planHash: `preview-${draft.version}`,
          draftVersion: draft.version,
          requiresDisruption: false,
          disruptionAllowed: false,
        },
        operation: {
          operationId: "preview-operation",
          state: "succeeded",
          runtimeHealth: "inactive",
        },
      };
      const value = values[String(query.type)];
      if (!value) throw new Error("Unsupported preview query");
      return copy(value) as T;
    },
    async command<T>(envelope: MutationEnvelope): Promise<T> {
      const payload = JSON.stringify(envelope.command),
        receipt = receipts.get(envelope.idempotencyKey);
      if (receipt) {
        if (receipt.payload !== payload) throw { code: "idempotency_conflict" };
        return copy(receipt.result) as T;
      }
      const command = envelope.command;
      let result: unknown;
      if (command.type === "patch_draft") {
        if (command.expected_version !== draft.version)
          throw { code: "revision_conflict" };
        const edit = command.edit as {
          document: GraphDocument;
          package_lock: Draft["packageLock"];
        };
        draft = {
          ...draft,
          version: draft.version + 1,
          document: copy(edit.document),
          packageLock: copy(edit.package_lock),
        };
        result = { draft: copy(draft) };
      } else if (command.type === "apply_draft") {
        result = { operationId: "preview-operation" };
        activeDocument = copy(draft.document);
        data.snapshot.active = {
          workspaceId: "preview",
          revisionId: `preview-${draft.version}`,
        };
      } else throw new Error("Unsupported preview command");
      receipts.set(envelope.idempotencyKey, { payload, result });
      return copy(result) as T;
    },
  };
}
