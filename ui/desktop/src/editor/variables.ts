// SPDX-License-Identifier: GPL-3.0-or-later
import { equal } from "./bindings";
import type { Shape } from "./types";
export { valueShape } from "./value-shape";
export function assignable(source: Shape, target: Shape): boolean {
  if (source.type === "string" && target.type === "string")
    return (
      source.maximumBytes <= target.maximumBytes &&
      (!target.choices?.length ||
        Boolean(
          source.choices?.length &&
          source.choices.every((v) => target.choices!.includes(v)),
        ))
    );
  if (source.type === "number" && target.type === "number")
    return source.minimum >= target.minimum && source.maximum <= target.maximum;
  return equal(source, target);
}
export function valuePaths(
  shape: Shape,
  target: Shape,
  pointer = "",
  depth = 0,
): { pointer: string; label: string }[] {
  if (depth > 8) return [];
  const result = assignable(shape, target)
    ? [{ pointer, label: pointer || "value" }]
    : [];
  if (shape.type === "record")
    for (const [key, field] of Object.entries(shape.fields)) {
      if (!field.optional)
        result.push(
          ...valuePaths(
            field.schema,
            target,
            pointer + "/" + key.replaceAll("~", "~0").replaceAll("/", "~1"),
            depth + 1,
          ),
        );
    }
  return result;
}
