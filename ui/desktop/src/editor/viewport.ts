// SPDX-License-Identifier: GPL-3.0-or-later
import { geometry } from "./appearance";
import { BINDING_KEY } from "./bindings";
import type { EditorDevice, GraphDocument } from "./types";
export type Viewport = {
  width: number;
  height: number;
  zoom: number;
  pan: { x: number; y: number };
};
export function deviceSize(
  device: EditorDevice,
  geo = geometry(device.descriptor),
) {
  const width = Math.min(740, Math.max(340, geo.width * 88 + 36));
  return {
    width,
    height: ((width - 36) * geo.height) / geo.width + 110,
    scale: (width - 36) / geo.width,
  };
}
export function visibleControls(
  device: EditorDevice,
  view: Viewport,
  selected: string,
  geo = geometry(device.descriptor),
) {
  const { scale } = deviceSize(device, geo),
    margin = 100;
  return device.descriptor.elements
    .map((element, index) => ({ element, index }))
    .filter(({ element }) => {
      if (element.elementId === selected) return true;
      const b = geo.controls[element.elementId];
      const x = (device.x + 18 + b.x * scale) * view.zoom + view.pan.x;
      const y = (device.y + 70 + b.y * scale) * view.zoom + view.pan.y;
      return (
        x + b.width * scale * view.zoom > -margin &&
        x < view.width + margin &&
        y + b.height * scale * view.zoom > -margin &&
        y < view.height + margin
      );
    });
}
export function assignmentIndex(document: GraphDocument | null) {
  const result = new Map<
    string,
    { count: number; first: GraphDocument["graph"]["nodes"][number] }
  >();
  for (const node of document?.graph.nodes ?? []) {
    const key = node.extensions[BINDING_KEY];
    if (node.kind.type !== "action" || typeof key !== "string") continue;
    const old = result.get(key);
    if (old) old.count++;
    else result.set(key, { count: 1, first: node });
  }
  return result;
}
export function canPan(
  button: number,
  space: boolean,
  interactive: boolean,
  device: boolean,
) {
  return button === 1 || (button === 0 && (space || (!interactive && !device)));
}
