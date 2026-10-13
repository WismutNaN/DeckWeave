// SPDX-License-Identifier: GPL-3.0-or-later
import { equal, BINDING_KEY } from "./bindings";
import { layout } from "./appearance";
import type { GraphDocument, GraphNode } from "./types";
export function reviewDetails(
  before: GraphDocument | null,
  after: GraphDocument,
) {
  const previous = new Map((before?.graph.nodes ?? []).map((n) => [n.id, n]));
  const next = new Map(after.graph.nodes.map((n) => [n.id, n]));
  const changed = (n: GraphNode, p: GraphNode) =>
    !equal(n, p) ||
    !equal(
      before?.graph.actionArguments?.[n.id],
      after.graph.actionArguments?.[n.id],
    ) ||
    before?.graph.nodeInstances?.[n.id] !== after.graph.nodeInstances?.[n.id];
  const rows: {
    kind: "added" | "changed" | "removed";
    node: GraphNode;
    deviceId: string;
    controlId: string;
    location: string;
  }[] = [];
  for (const id of new Set([...next.keys(), ...previous.keys()])) {
    const n = next.get(id),
      p = previous.get(id),
      node = n ?? p!;
    if (node.kind.type !== "action" || (n && p && !changed(n, p))) continue;
    const identity = node.extensions[BINDING_KEY];
    let deviceId = "",
      controlId = "";
    if (typeof identity === "string")
      try {
        const pair = JSON.parse(identity);
        if (
          Array.isArray(pair) &&
          pair.length === 2 &&
          pair.every((v) => typeof v === "string")
        )
          [deviceId, controlId] = pair;
      } catch {}
    const device = layout(n ? after : before!).devices.find(
      (d) => d.id === deviceId,
    );
    const control =
      device?.controls[controlId]?.label ||
      device?.descriptor.elements.find((e) => e.elementId === controlId)
        ?.label ||
      controlId;
    rows.push({
      kind: !p ? "added" : !n ? "removed" : "changed",
      node,
      deviceId,
      controlId,
      location: device ? `${device.descriptor.label} · ${control}` : node.id,
    });
  }
  return rows;
}
