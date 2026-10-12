// SPDX-License-Identifier: GPL-3.0-or-later
import type { Endpoint } from "./types";

export const ACTION_DRAG_TYPE = "application/x-deckweave-action";
const ACTION_DRAG_PREFIX = "deckweave-action:";

type DragWriter = Pick<DataTransfer, "setData" | "effectAllowed">;
type DragReader = Pick<DataTransfer, "getData" | "types">;
export type SelectionDrag = {
  kind: "control" | "step" | "device";
  deviceId: string;
  controlId: string;
  stepId?: string;
};
export const SELECTION_DRAG_TYPE = "application/x-deckweave-selection";
export function writeSelectionDrag(transfer: DragWriter, value: SelectionDrag) {
  const text = "deckweave-selection:" + JSON.stringify(value);
  transfer.setData(SELECTION_DRAG_TYPE, text);
  transfer.setData("text/plain", text);
  transfer.effectAllowed = value.kind === "control" ? "copyMove" : "move";
}
export function readSelectionDrag(
  transfer: DragReader | null,
): SelectionDrag | null {
  if (!transfer) return null;
  const text =
    transfer.getData(SELECTION_DRAG_TYPE) || transfer.getData("text/plain");
  if (!text.startsWith("deckweave-selection:") || text.length > 8192)
    return null;
  try {
    const value = JSON.parse(text.slice(20));
    if (
      ["control", "step", "device"].includes(value?.kind) &&
      typeof value.deviceId === "string" &&
      typeof value.controlId === "string" &&
      (value.kind !== "step" || typeof value.stepId === "string")
    )
      return value;
  } catch {
    /* чужой drag */
  }
  return null;
}

function payload(endpoint: Endpoint): string {
  return ACTION_DRAG_PREFIX + JSON.stringify(endpoint);
}

function endpoint(value: unknown): value is Endpoint {
  if (!value || typeof value !== "object") return false;
  const item = value as Partial<Endpoint>;
  return (
    typeof item.packageId === "string" &&
    typeof item.nodeId === "string" &&
    Boolean(item.target) &&
    typeof item.target === "object" &&
    (item.target.kind === "device" || item.target.kind === "service") &&
    Boolean(item.port) &&
    typeof item.port === "object" &&
    typeof item.port.elementId === "string" &&
    typeof item.port.capabilityId === "string" &&
    typeof item.port.portId === "string"
  );
}

export function writeActionDrag(
  transfer: DragWriter,
  actionEndpoint: Endpoint,
): void {
  const value = payload(actionEndpoint);
  transfer.setData(ACTION_DRAG_TYPE, value);
  // WebView implementations are more consistent with a standard fallback.
  transfer.setData("text/plain", value);
  transfer.effectAllowed = "copy";
}

export function hasActionDrag(transfer: DragReader | null): boolean {
  if (!transfer) return false;
  return Array.from(transfer.types).some(
    (type) => type === ACTION_DRAG_TYPE || type === "text/plain",
  );
}

export function readActionDrag(transfer: DragReader | null): Endpoint | null {
  if (!transfer) return null;
  for (const type of [ACTION_DRAG_TYPE, "text/plain"]) {
    const value = transfer.getData(type);
    if (!value.startsWith(ACTION_DRAG_PREFIX)) continue;
    try {
      const decoded: unknown = JSON.parse(
        value.slice(ACTION_DRAG_PREFIX.length),
      );
      if (endpoint(decoded)) return decoded;
    } catch {
      // Foreign or truncated drag data is not a DeckWeave assignment.
    }
  }
  return null;
}
