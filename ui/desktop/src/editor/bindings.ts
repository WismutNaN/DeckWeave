// SPDX-License-Identifier: GPL-3.0-or-later
import { valueShape } from "./value-shape";
import { argumentBindings } from "./arguments";
import type {
  Action,
  Binding,
  Catalog,
  CatalogEndpoint,
  EditorDevice,
  GraphDocument,
  GraphNode,
  GraphPort,
  Json,
  Schema,
  SchemaRef,
  Shape,
} from "./types";
export const BINDING_KEY = "org.deckweave.editorBinding";
export const equal = (a: unknown, b: unknown): boolean => {
  if (a === b) return true;
  if (
    !a ||
    !b ||
    typeof a !== "object" ||
    typeof b !== "object" ||
    Array.isArray(a) !== Array.isArray(b)
  )
    return false;
  const left = Object.entries(a),
    right = Object.entries(b);
  return (
    left.length === right.length &&
    left.every(
      ([k, v]) =>
        Object.hasOwn(b, k) && equal(v, (b as Record<string, unknown>)[k]),
    )
  );
};
export const schemaFor = (
  catalog: Catalog,
  ref: SchemaRef,
): Schema | undefined => catalog.schemas.find((s) => equal(s.schema, ref));
export function initialValue(shape: Shape): Json {
  switch (shape.type) {
    case "null":
    case "nullable":
      return null;
    case "boolean":
      return false;
    case "number":
      return Math.max(shape.minimum, Math.min(0, shape.maximum));
    case "string":
      return shape.choices?.[0] ?? "";
    case "array":
      return [];
    case "record":
      return Object.fromEntries(
        Object.entries(shape.fields)
          .filter(([, f]) => !f.optional)
          .map(([key, f]) => [key, initialValue(f.schema)]),
      );
  }
}
export function actionShape(action: Action): Shape | null {
  const schema =
    action.operation && schemaFor(action.catalog, action.operation.inputSchema);
  return schema ? valueShape(schema, true) : null;
}
export function owner(deviceId: string, elementId: string): string {
  return JSON.stringify([deviceId, elementId]);
}
export function steps(
  document: GraphDocument,
  deviceId: string,
  elementId: string,
): GraphNode[] {
  return bindingNodes(document, deviceId, elementId).filter(
    (n) =>
      n.kind.type === "action" &&
      n.extensions[BINDING_KEY] === owner(deviceId, elementId),
  );
}
export function bindingNodes(
  document: GraphDocument,
  deviceId: string,
  elementId: string,
): GraphNode[] {
  const nodes = document.graph.nodes.filter(
    (n) => n.extensions[BINDING_KEY] === owner(deviceId, elementId),
  );
  if (!nodes.length) return [];
  const roots = nodes.filter((n) => n.kind.type === "source");
  if (roots.length !== 1) return nodes; // A complex graph is shown but cannot be extended blindly.
  const result: GraphNode[] = [];
  let current: GraphNode | undefined = roots[0];
  while (current && !result.some((n) => n.id === current?.id)) {
    result.push(current);
    const edges = document.graph.edges.filter(
      (e) =>
        e.source.nodeId === current?.id &&
        nodes.some((n) => n.id === e.target.nodeId),
    );
    if (edges.length > 1) return nodes;
    current = nodes.find((n) => n.id === edges[0]?.target.nodeId);
  }
  return result.length === nodes.length ? result : nodes;
}
export function foreignBinding(
  document: GraphDocument,
  device: EditorDevice,
  elementId: string,
): boolean {
  return document.graph.nodes.some((n) => {
    const e = n.kind.endpoint;
    return (
      e?.packageId === device.package.packageId &&
      e.nodeId === device.nodeId &&
      e.target.kind === "device" &&
      e.target.providerId === device.descriptor.providerId &&
      e.target.deviceIdentity === device.descriptor.identity &&
      e.port.elementId === elementId &&
      n.kind.type === "source" &&
      n.extensions[BINDING_KEY] !== owner(device.id, elementId)
    );
  });
}
export function sourceFor(
  catalogs: Catalog[],
  device: EditorDevice,
  elementId: string,
): CatalogEndpoint | undefined {
  const candidates = sourceCandidates(catalogs, device, elementId),
    selected = device.controls[elementId]?.trigger;
  return selected
    ? candidates.find((e) => equal(e.endpoint, selected))
    : candidates.length === 1
      ? candidates[0]
      : undefined;
}
export function sourceCandidates(
  catalogs: Catalog[],
  device: EditorDevice,
  elementId: string,
): CatalogEndpoint[] {
  return catalogs
    .filter((c) => equal(c.package, device.package))
    .flatMap((c) => c.endpoints)
    .filter(
      (e) =>
        e.endpoint.packageId === device.package.packageId &&
        e.endpoint.nodeId === device.nodeId &&
        e.endpoint.target.kind === "device" &&
        e.endpoint.target.deviceIdentity === device.descriptor.identity &&
        e.endpoint.target.providerId === device.descriptor.providerId &&
        e.endpoint.port.elementId === elementId &&
        ["EventOut", "ValueOut"].includes(e.port.kind) &&
        !e.port.tags?.includes("org.deckweave.virtual.display"),
    );
}
function port(
  id: string,
  schema: SchemaRef,
  role: string,
  kind: "flow" | "value" = "flow",
): GraphPort {
  return { id, schema, role, kind, unit: null, range: null };
}
export function appendAction(
  document: GraphDocument,
  catalogs: Catalog[],
  device: EditorDevice,
  elementId: string,
  action: Action,
  uid: string,
): string {
  const schemaPort = (
    id: string,
    ref: SchemaRef,
    role: string,
    kind: "flow" | "value" = "flow",
  ): GraphPort => {
    const schema = catalogs
      .flatMap((c) => c.schemas)
      .find((s) => equal(s.schema, ref));
    if (!schema) throw new Error("Missing static schema");
    return {
      ...port(id, ref, role, kind),
      unit: schema.unit,
      range: schema.range,
    };
  };
  const source = sourceFor(catalogs, device, elementId);
  if (!source || !action.operation)
    throw new Error("This control has no static input contract");
  const shape = actionShape(action);
  if (!shape)
    throw new Error("This action needs a supported bounded input schema");
  const identity = owner(device.id, elementId);
  if (foreignBinding(document, device, elementId))
    throw new Error(
      "Элемент уже используется в сложном графе / This control is used by an advanced graph",
    );
  const existing = bindingNodes(document, device.id, elementId);
  if (
    existing.length &&
    existing.some((n, i) => {
      const outgoing = document.graph.edges.filter(
        (e) => e.source.nodeId === n.id,
      );
      return (
        outgoing.length !== (i === existing.length - 1 ? 0 : 1) ||
        (outgoing[0] && outgoing[0].target.nodeId !== existing[i + 1]?.id)
      );
    })
  )
    throw new Error(
      "Сценарий требует редактора графа / This binding requires the graph editor",
    );
  const extensions = { [BINDING_KEY]: identity };
  let previous = existing.at(-1);
  if (!previous) {
    previous = {
      id: `source-${uid}`,
      kind: {
        type: "source",
        endpoint: source.endpoint,
        availability: "deferred",
      },
      inputs: [],
      outputs: [schemaPort("event", source.port.schema, "value", "value")],
      extensions,
    };
    document.graph.nodes.push(previous);
    const catalog = catalogs.find(
      (c) => c.package.packageId === source.endpoint.packageId,
    );
    if (
      catalog &&
      schemaFor(catalog, source.port.schema)?.shape === "boolean" &&
      device.controls[elementId]?.when !== "any"
    ) {
      previous.outputs[0].kind = "value";
      previous.outputs[0].role = "value";
      const condition: GraphNode = {
        id: `pressed-${uid}`,
        kind: { type: "condition", operator: "is_true", operand: null },
        inputs: [schemaPort("input", source.port.schema, "input", "value")],
        outputs: [
          schemaPort(
            "match",
            source.port.schema,
            device.controls[elementId]?.when === "false" ? "false" : "true",
          ),
          schemaPort(
            "other",
            source.port.schema,
            device.controls[elementId]?.when === "false" ? "true" : "false",
          ),
        ],
        extensions,
      };
      document.graph.edges.push({
        id: `edge-press-${uid}`,
        kind: "data",
        source: { nodeId: previous.id, portId: "event" },
        target: { nodeId: condition.id, portId: "input" },
        extensions: {},
      });
      document.graph.nodes.push(condition);
      previous = condition;
    }
  }
  const priorPort = previous.outputs[0];
  const node: GraphNode = {
    id: `action-${uid}`,
    kind: {
      type: "action",
      endpoint: action.endpoint,
      operation_id: action.operation.operationId,
      availability: "deferred",
    },
    inputs: [
      schemaPort(
        "trigger",
        priorPort.schema,
        priorPort.kind === "flow" ? "trigger" : "input",
        priorPort.kind,
      ),
    ],
    outputs: [
      schemaPort("result", action.operation.resultSchema, "result", "value"),
    ],
    extensions,
  };
  document.graph.nodes.push(node);
  document.graph.edges.push({
    id: `edge-action-${uid}`,
    kind: priorPort.kind === "flow" ? "flow" : "data",
    source: { nodeId: previous.id, portId: priorPort.id },
    target: { nodeId: node.id, portId: "trigger" },
    extensions: {},
  });
  if (shape.type === "record") {
    document.graph.actionArguments ??= {};
    document.graph.actionArguments[node.id] = {
      triggerPort: "trigger",
      fields: Object.fromEntries(
        Object.entries(shape.fields)
          .filter(([, f]) => !f.optional)
          .map(([key, f]) => [
            key,
            { type: "constant", value: initialValue(f.schema) } as Binding,
          ]),
      ),
    };
  }
  if (shape.type !== "record") {
    document.schemaVersion.minor = Math.max(document.schemaVersion.minor, 1);
    document.graph.actionArguments ??= {};
    document.graph.actionArguments[node.id] = {
      triggerPort: "trigger",
      fields: {},
      value: { type: "constant", value: initialValue(shape) },
    };
  }
  return node.id;
}
export function removeLastAction(
  document: GraphDocument,
  deviceId: string,
  elementId: string,
): void {
  const identity = owner(deviceId, elementId),
    chain = bindingNodes(document, deviceId, elementId);
  const action = chain.filter((n) => n.kind.type === "action").at(-1);
  if (!action) return;
  const remove = new Set(
    chain
      .filter(
        (n) =>
          n.id === action.id ||
          chain.filter((a) => a.kind.type === "action").length === 1,
      )
      .map((n) => n.id),
  );
  for (const node of document.graph.nodes.filter((n) => !remove.has(n.id))) {
    const args = argumentBindings(document.graph.actionArguments?.[node.id]);
    if (
      args.some(
        (b) =>
          (b.type === "run_result" || b.type === "state") &&
          remove.has(b.nodeId),
      )
    )
      throw new Error("The result is used by another action");
  }
  if (
    document.graph.edges.some(
      (e) => remove.has(e.source.nodeId) && !remove.has(e.target.nodeId),
    )
  )
    throw new Error("The result is connected to another graph node");
  document.graph.nodes = document.graph.nodes.filter((n) => !remove.has(n.id));
  document.graph.edges = document.graph.edges.filter(
    (e) => !remove.has(e.source.nodeId) && !remove.has(e.target.nodeId),
  );
  for (const id of remove) {
    delete document.graph.actionArguments?.[id];
    delete document.graph.nodeInstances?.[id];
    delete document.layout.nodes[id];
  }
}
