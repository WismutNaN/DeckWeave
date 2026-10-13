// SPDX-License-Identifier: GPL-3.0-or-later
// An explicit result-to-display binding, including provider readback through a state node.
import { argumentBindings } from "./arguments";
import { copy } from "./api";
import { layout } from "./appearance";
import { BINDING_KEY, equal, owner, steps } from "./bindings";
import type { Catalog, EditorDevice, GraphDocument } from "./types";
const KEY = "org.deckweave.editorDisplay";
export function displayEndpoints(
  catalogs: Catalog[],
  device: EditorDevice,
  control: string,
) {
  return catalogs
    .flatMap((c) => c.endpoints)
    .filter(
      (e) =>
        e.endpoint.packageId === device.package.packageId &&
        e.endpoint.nodeId === device.nodeId &&
        e.endpoint.target.kind === "device" &&
        e.endpoint.target.providerId === device.descriptor.providerId &&
        e.endpoint.target.deviceIdentity === device.descriptor.identity &&
        e.endpoint.port.elementId === control,
    );
}
export function removeEditorFeedback(doc: GraphDocument) {
  const ids = new Set(
    doc.graph.nodes
      .filter((n) => typeof n.extensions[KEY] === "string")
      .map((n) => n.id),
  );
  const nodes = new Map(doc.graph.nodes.map((n) => [n.id, n]));
  if (
    doc.graph.edges.some(
      (e) =>
        ids.has(e.target.nodeId) &&
        !ids.has(e.source.nodeId) &&
        (nodes.get(e.source.nodeId)?.kind.type !== "action" ||
          nodes.get(e.source.nodeId)?.extensions[BINDING_KEY] !==
            nodes.get(e.target.nodeId)?.extensions[KEY]),
    ) ||
    doc.graph.edges.some(
      (e) => ids.has(e.source.nodeId) && !ids.has(e.target.nodeId),
    ) ||
    Object.entries(doc.graph.actionArguments ?? {}).some(
      ([id, args]) =>
        !ids.has(id) &&
        argumentBindings(args).some(
          (b) =>
            (b.type === "state" || b.type === "run_result") &&
            ids.has(b.nodeId),
        ),
    )
  )
    throw new Error(
      "Вывод используется другим сценарием / The display is used by another workflow",
    );
  doc.graph.nodes = doc.graph.nodes.filter((n) => !ids.has(n.id));
  doc.graph.edges = doc.graph.edges.filter(
    (e) => !ids.has(e.source.nodeId) && !ids.has(e.target.nodeId),
  );
  for (const id of ids) {
    delete doc.layout.nodes[id];
    delete doc.graph.nodeInstances?.[id];
    delete doc.graph.actionArguments?.[id];
  }
}
export function syncEditorFeedback(doc: GraphDocument, catalogs: Catalog[]) {
  for (const device of layout(doc).devices)
    for (const [control, appearance] of Object.entries(device.controls)) {
      if (!appearance.showResult) continue;
      const actions = steps(doc, device.id, control),
        last = actions.at(-1);
      if (!last) continue;
      const endpoints = displayEndpoints(catalogs, device, control);
      const writes = endpoints.filter(
        (e) =>
          e.port.kind === "ValueIn" &&
          equal(e.port.schema, last.outputs[0].schema),
      );
      if (writes.length !== 1)
        throw new Error(
          "Для результата нет однозначного выхода. Отключите вывод результата или выберите совместимое действие. / No matching display output. Disable result display or choose a compatible action.",
        );
      const write = writes[0],
        id = `display-${actions[0].id}`,
        extensions = { [KEY]: owner(device.id, control) };
      if (
        [id, `${id}-observed`, `${id}-state`].some((key) =>
          doc.graph.nodes.some((n) => n.id === key),
        )
      )
        throw new Error("Display node ID conflict");
      const valuePort = {
        ...copy(last.outputs[0]),
        id: "value",
        kind: "value" as const,
        role: "value",
      };
      doc.graph.nodes.push({
        id,
        kind: {
          type: "feedback",
          endpoint: write.endpoint,
          availability: "deferred",
        },
        inputs: [{ ...valuePort, id: "input", role: "input" }],
        outputs: [],
        extensions,
      });
      doc.graph.edges.push({
        id: `edge-${id}`,
        kind: "data",
        source: { nodeId: last.id, portId: last.outputs[0].id },
        target: { nodeId: id, portId: "input" },
        extensions: {},
      });
      const read = endpoints.find(
        (e) =>
          e.port.kind === "ValueOut" &&
          e.port.tags?.includes("org.deckweave.virtual.display") &&
          equal(e.port.schema, write.port.schema),
      );
      if (read) {
        const observed = `${id}-observed`,
          state = `${id}-state`;
        doc.graph.nodes.push(
          {
            id: observed,
            kind: {
              type: "source",
              endpoint: read.endpoint,
              availability: "deferred",
            },
            inputs: [],
            outputs: [valuePort],
            extensions,
          },
          {
            id: state,
            kind: {
              type: "state",
              state_key: `org.deckweave.virtual.display.${actions[0].id}`,
              initial_value: "",
            },
            inputs: [{ ...valuePort, id: "input", role: "input" }],
            outputs: [valuePort],
            extensions,
          },
        );
        doc.graph.edges.push({
          id: `edge-${observed}`,
          kind: "data",
          source: { nodeId: observed, portId: "value" },
          target: { nodeId: state, portId: "input" },
          extensions: {},
        });
      }
    }
}
