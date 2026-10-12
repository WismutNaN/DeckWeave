// SPDX-License-Identifier: GPL-3.0-or-later
import type {
  Background,
  Bounds,
  Device,
  EditorDevice,
  EditorLayout,
  GraphDocument,
} from "./types";
export const EDITOR_KEY = "org.deckweave.editor";
export const defaultBackground = (): Background => ({
  mode: "gradient",
  start: "#3e6687",
  end: "#b47769",
  angle: 30,
  brightness: 100,
});
export function layout(document: GraphDocument): EditorLayout {
  const value = document.extensions[EDITOR_KEY] as unknown as
    EditorLayout | undefined;
  if (
    document.schemaVersion?.major !== 1 ||
    !document.graph?.nodes ||
    !document.layout?.nodes
  )
    throw new Error("Unsupported workspace document");
  if (
    value &&
    (value.version !== 1 ||
      !Array.isArray(value.devices) ||
      value.devices.length > 256)
  )
    throw new Error("Unsupported editor document");
  if (value) {
    const ids = new Set<string>();
    for (const d of value.devices) {
      if (
        !d ||
        typeof d.id !== "string" ||
        ids.has(d.id) ||
        !d.package?.packageId ||
        !d.nodeId ||
        !d.descriptor?.providerId ||
        !d.descriptor.identity ||
        !Array.isArray(d.descriptor.elements) ||
        d.descriptor.elements.length > 4096 ||
        ![d.x, d.y].every(Number.isFinite) ||
        !d.appearance ||
        !d.controls ||
        d.descriptor.elements.some(
          (e) => !e.elementId || !Array.isArray(e.capabilities),
        )
      )
        throw new Error("Invalid editor device layout");
      ids.add(d.id);
    }
  }
  return value ?? { version: 1, devices: [] };
}
export function putLayout(document: GraphDocument, value: EditorLayout): void {
  document.extensions[EDITOR_KEY] = JSON.parse(JSON.stringify(value));
}
export function geometry(device: Device): {
  width: number;
  height: number;
  controls: Record<string, Bounds>;
  measured: boolean;
} {
  const descriptor = device["org.deckweave.controlLayout"];
  const valid =
    descriptor?.version === 1 &&
    Boolean(descriptor.elements) &&
    descriptor.width > 0 &&
    descriptor.height > 0 &&
    descriptor.width <= 10000 &&
    descriptor.height <= 10000 &&
    device.elements.every((e) => {
      const b = descriptor.elements[e.elementId];
      return (
        b &&
        [b.x, b.y, b.width, b.height].every(Number.isFinite) &&
        b.x >= 0 &&
        b.y >= 0 &&
        b.width > 0 &&
        b.height > 0 &&
        b.x + b.width <= descriptor.width &&
        b.y + b.height <= descriptor.height
      );
    });
  if (valid && descriptor)
    return {
      width: descriptor.width,
      height: descriptor.height,
      controls: descriptor.elements,
      measured: true,
    };
  // Список без заявления о реальном расположении клавиш.
  return {
    width: 4,
    height: Math.max(1, Math.ceil(device.elements.length / 4)),
    measured: false,
    controls: Object.fromEntries(
      device.elements.map((element, i) => [
        element.elementId,
        { x: i % 4, y: Math.floor(i / 4), width: 1, height: 1 },
      ]),
    ),
  };
}
const safeColor = (color: string) =>
  /^#[0-9a-f]{6}$/i.test(color) ? color : "#334155";
export function backgroundCss(background: Background): string {
  return background.mode === "solid"
    ? safeColor(background.start)
    : `linear-gradient(${Number.isFinite(background.angle) ? background.angle : 0}deg, ${safeColor(background.start)}, ${safeColor(background.end)})`;
}
export function controlBackground(
  device: EditorDevice,
  elementId: string,
  geo = geometry(device.descriptor),
): string {
  const own = device.controls[elementId]?.background;
  if (own) return backgroundCss(own);
  const b = geo.controls[elementId];
  if (device.appearance.mode === "solid" || !b || !geo.measured)
    return safeColor(device.appearance.start);
  // Один фон на плоскости устройства, а не отдельный gradient на каждой кнопке.
  const width = (geo.width / b.width) * 100,
    height = (geo.height / b.height) * 100;
  const x = geo.width === b.width ? 0 : (b.x / (geo.width - b.width)) * 100;
  const y = geo.height === b.height ? 0 : (b.y / (geo.height - b.height)) * 100;
  return `${backgroundCss(device.appearance)} ${x}% ${y}% / ${width}% ${height}%`;
}

export function neighbor(
  device: Device,
  index: number,
  direction: string,
): number {
  const g = geometry(device),
    current = g.controls[device.elements[index].elementId];
  const dx =
      direction === "ArrowRight" ? 1 : direction === "ArrowLeft" ? -1 : 0,
    dy = direction === "ArrowDown" ? 1 : direction === "ArrowUp" ? -1 : 0;
  let next = index,
    score = Infinity;
  device.elements.forEach((element, i) => {
    const b = g.controls[element.elementId],
      x = b.x + b.width / 2 - current.x - current.width / 2,
      y = b.y + b.height / 2 - current.y - current.height / 2,
      along = x * dx + y * dy,
      cross = Math.abs(x * dy - y * dx),
      distance = along + cross * 3;
    if (along > 0.001 && distance < score) {
      score = distance;
      next = i;
    }
  });
  return next;
}
