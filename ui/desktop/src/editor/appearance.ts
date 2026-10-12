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
  if (value && (value.version !== 1 || !Array.isArray(value.devices)))
    throw new Error("Unsupported editor document");
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
): string {
  const own = device.controls[elementId]?.background;
  if (own) return backgroundCss(own);
  const geo = geometry(device.descriptor),
    b = geo.controls[elementId];
  if (device.appearance.mode === "solid" || !b || !geo.measured)
    return safeColor(device.appearance.start);
  // Один фон на плоскости устройства, а не отдельный gradient на каждой кнопке.
  const width = (geo.width / b.width) * 100,
    height = (geo.height / b.height) * 100;
  const x = geo.width === b.width ? 0 : (b.x / (geo.width - b.width)) * 100;
  const y = geo.height === b.height ? 0 : (b.y / (geo.height - b.height)) * 100;
  return `${backgroundCss(device.appearance)} ${x}% ${y}% / ${width}% ${height}%`;
}
