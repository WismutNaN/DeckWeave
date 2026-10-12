// SPDX-License-Identifier: GPL-3.0-or-later
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
export const equal = (a: unknown, b: unknown) =>
  JSON.stringify(a) === JSON.stringify(b);
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
  const shape =
    action.operation &&
    schemaFor(action.catalog, action.operation.inputSchema)?.shape;
  return typeof shape === "object" && shape
    ? shape.structured
    : shape === "unit"
      ? { type: "null" }
      : null;
}
export function owner(deviceId: string, elementId: string): string {
  return JSON.stringify([deviceId, elementId]);
}
export function steps(
  document: GraphDocument,
  deviceId: string,
  elementId: string,
): GraphNode[] {
  return document.graph.nodes.filter(
    (n) =>
      n.kind.type === "action" &&
      n.extensions[BINDING_KEY] === owner(deviceId, elementId),
  );
}
export function sourceFor(
  catalogs: Catalog[],
  device: EditorDevice,
  elementId: string,
): CatalogEndpoint | undefined {
  return catalogs
    .flatMap((c) => c.endpoints)
    .find(
      (e) =>
        e.endpoint.packageId === device.package.packageId &&
        e.endpoint.nodeId === device.nodeId &&
        e.endpoint.target.kind === "device" &&
        e.endpoint.target.deviceIdentity === device.descriptor.identity &&
        e.endpoint.target.providerId === device.descriptor.providerId &&
        e.endpoint.port.elementId === elementId &&
        ["EventOut", "ValueOut"].includes(e.port.kind),
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
  const source = sourceFor(catalogs, device, elementId);
  if (!source || !action.operation)
    throw new Error("This control has no static input contract");
  const shape = actionShape(action);
  if (!shape || (shape.type !== "record" && shape.type !== "null"))
    throw new Error("This action needs a supported record or unit schema");
  const identity = owner(device.id, elementId);
  const existing = document.graph.nodes.filter(
    (n) => n.extensions[BINDING_KEY] === identity,
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
      outputs: [port("event", source.port.schema, "trigger")],
      extensions,
    };
    document.graph.nodes.push(previous);
    const catalog = catalogs.find(
      (c) => c.package.packageId === source.endpoint.packageId,
    );
    if (
      catalog &&
      schemaFor(catalog, source.port.schema)?.shape === "boolean"
    ) {
      previous.outputs[0].kind = "value";
      previous.outputs[0].role = "value";
      const condition: GraphNode = {
        id: `pressed-${uid}`,
        kind: { type: "condition", operator: "is_true", operand: null },
        inputs: [port("input", source.port.schema, "input", "value")],
        outputs: [
          port("yes", source.port.schema, "true"),
          port("no", source.port.schema, "false"),
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
      port(
        "trigger",
        priorPort.schema,
        priorPort.kind === "flow" ? "trigger" : "input",
        priorPort.kind,
      ),
    ],
    outputs: [port("result", action.operation.resultSchema, "result", "value")],
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
  return node.id;
}
export function removeLastAction(
  document: GraphDocument,
  deviceId: string,
  elementId: string,
): void {
  const identity = owner(deviceId, elementId),
    chain = document.graph.nodes.filter(
      (n) => n.extensions[BINDING_KEY] === identity,
    );
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
    const args = Object.values(
      document.graph.actionArguments?.[node.id]?.fields ?? {},
    );
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
