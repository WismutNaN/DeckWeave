// SPDX-License-Identifier: GPL-3.0-or-later
import { appendAction, bindingNodes, equal, schemaFor } from "./bindings";
import { argumentBindings } from "./arguments";
import { copy } from "./api";
import type { Catalog, EditorDevice, GraphDocument, GraphNode } from "./types";

function chain(document: GraphDocument, deviceId: string, controlId: string) {
  const nodes = bindingNodes(document, deviceId, controlId);
  if (
    nodes.some((n, i) => {
      const incoming = document.graph.edges.filter(
        (e) => e.target.nodeId === n.id,
      );
      if (
        incoming.length !== (i === 0 ? 0 : 1) ||
        (incoming[0] && incoming[0].source.nodeId !== nodes[i - 1]?.id)
      )
        return true;
      const edges = document.graph.edges.filter(
        (e) => e.source.nodeId === n.id,
      );
      return (
        edges.length !== (i === nodes.length - 1 ? 0 : 1) ||
        (edges[0] && edges[0].target.nodeId !== nodes[i + 1]?.id)
      );
    })
  )
    throw new Error(
      "Для ветвящегося сценария нужен редактор графа / A branching workflow requires the graph editor",
    );
  return nodes;
}
function checkOrder(
  document: GraphDocument,
  actions: GraphNode[],
  sources: GraphNode[],
) {
  actions.forEach((node, i) => {
    for (const binding of argumentBindings(
      document.graph.actionArguments?.[node.id],
    )) {
      if (
        binding.type === "run_result" &&
        ![...sources, ...actions.slice(0, i)].some(
          (n) => n.id === binding.nodeId,
        )
      )
        throw new Error(
          "Сначала должен выполняться шаг, чей результат используется / The result producer must run first",
        );
    }
  });
}
function wire(
  document: GraphDocument,
  nodes: GraphNode[],
  catalogs: Catalog[],
) {
  nodes.slice(1).forEach((node, i) => {
    const previous = nodes[i];
    if (node.kind.type !== "action") return;
    const input = previous.outputs[0];
    const action = catalogs
      .flatMap((c) => c.endpoints)
      .find((e) => equal(e.endpoint, node.kind.endpoint));
    const unit =
      action?.operation &&
      catalogs.some(
        (c) => schemaFor(c, action.operation!.inputSchema)?.shape === "unit",
      );
    if (
      unit &&
      input.kind === "value" &&
      !equal(input.schema, action!.operation!.inputSchema)
    ) {
      document.graph.actionArguments ??= {};
      document.graph.actionArguments[node.id] ??= {
        triggerPort: "trigger",
        fields: {},
        value: { type: "constant", value: null },
      };
      document.schemaVersion.minor = Math.max(document.schemaVersion.minor, 1);
    }
    node.inputs[0] = {
      ...copy(input),
      id: "trigger",
      role: input.kind === "flow" ? "trigger" : "input",
    };
  });
  const ids = new Set(nodes.map((n) => n.id));
  document.graph.edges = document.graph.edges.filter(
    (e) => !ids.has(e.source.nodeId),
  );
  nodes.slice(1).forEach((node, i) =>
    document.graph.edges.push({
      id: `sequence-${nodes[i].id}-${node.id}`,
      kind: nodes[i].outputs[0].kind === "flow" ? "flow" : "data",
      source: { nodeId: nodes[i].id, portId: nodes[i].outputs[0].id },
      target: { nodeId: node.id, portId: node.inputs[0].id },
      extensions: {},
    }),
  );
}
export function reorderAction(
  document: GraphDocument,
  catalogs: Catalog[],
  deviceId: string,
  controlId: string,
  from: string,
  before: string,
) {
  const nodes = chain(document, deviceId, controlId),
    actions = nodes.filter((n) => n.kind.type === "action");
  const moving = actions.find((n) => n.id === from);
  if (!moving || from === before || !actions.some((n) => n.id === before))
    return;
  const reordered = actions.filter((n) => n !== moving);
  reordered.splice(
    reordered.findIndex((n) => n.id === before),
    0,
    moving,
  );
  checkOrder(
    document,
    reordered,
    nodes.filter((n) => n.kind.type === "source"),
  );
  wire(
    document,
    [...nodes.filter((n) => n.kind.type !== "action"), ...reordered],
    catalogs,
  );
}
export function removeControlBinding(
  document: GraphDocument,
  deviceId: string,
  controlId: string,
) {
  const nodes = chain(document, deviceId, controlId),
    ids = new Set(nodes.map((n) => n.id));
  for (const node of document.graph.nodes.filter((n) => !ids.has(n.id))) {
    if (
      argumentBindings(document.graph.actionArguments?.[node.id]).some(
        (b) =>
          (b.type === "run_result" || b.type === "state") && ids.has(b.nodeId),
      )
    )
      throw new Error(
        "Результат используется другой кнопкой / Another control uses this result",
      );
  }
  document.graph.nodes = document.graph.nodes.filter((n) => !ids.has(n.id));
  document.graph.edges = document.graph.edges.filter(
    (e) => !ids.has(e.source.nodeId) && !ids.has(e.target.nodeId),
  );
  for (const id of ids) {
    delete document.graph.actionArguments?.[id];
    delete document.graph.nodeInstances?.[id];
    delete document.layout.nodes[id];
  }
}
export function copyControlBinding(
  document: GraphDocument,
  catalogs: Catalog[],
  source: EditorDevice,
  control: string,
  target: EditorDevice,
  targetControl: string,
  move: boolean,
) {
  if (source.id === target.id && control === targetControl) return;
  if (bindingNodes(document, target.id, targetControl).length)
    throw new Error(
      "Целевая кнопка уже настроена / Target control already has actions",
    );
  const actions = chain(document, source.id, control).filter(
    (n) => n.kind.type === "action",
  );
  checkOrder(
    document,
    actions,
    bindingNodes(document, source.id, control).filter(
      (n) => n.kind.type === "source",
    ),
  );
  const remap = new Map<string, string>();
  for (const node of actions) {
    const catalog = catalogs.find((c) =>
      c.endpoints.some((e) => equal(e.endpoint, node.kind.endpoint)),
    );
    const entry = catalog?.endpoints.find((e) =>
      equal(e.endpoint, node.kind.endpoint),
    );
    if (!catalog || !entry)
      throw new Error(
        "Недоступен контракт действия / Action contract unavailable",
      );
    const id = appendAction(
      document,
      catalogs,
      target,
      targetControl,
      { ...entry, catalog, title: "" },
      crypto.randomUUID(),
    );
    remap.set(node.id, id);
    const sourceNode = bindingNodes(document, source.id, control).find(
        (n) => n.kind.type === "source",
      ),
      targetNode = bindingNodes(document, target.id, targetControl).find(
        (n) => n.kind.type === "source",
      );
    if (sourceNode && targetNode) remap.set(sourceNode.id, targetNode.id);
    const arguments_ = document.graph.actionArguments?.[node.id];
    if (arguments_) {
      const cloned = copy(arguments_);
      for (const binding of argumentBindings(cloned))
        if (binding.type === "run_result")
          binding.nodeId = remap.get(binding.nodeId)!;
      document.graph.actionArguments![id] = cloned;
    }
    const instance = document.graph.nodeInstances?.[node.id];
    if (instance) {
      document.graph.nodeInstances ??= {};
      document.graph.nodeInstances[id] = instance;
    }
  }
  if (move) removeControlBinding(document, source.id, control);
}
