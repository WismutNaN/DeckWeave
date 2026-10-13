// SPDX-License-Identifier: GPL-3.0-or-later
import type { Schema, Shape } from "./types";
// A conservative presentation filter. The core compiler remains authoritative.
export function valueShape(schema: Schema, allowUnit = false): Shape | null {
  if (schema.unit && !allowUnit) return null;
  if (typeof schema.shape === "object") {
    const shape = schema.shape.structured;
    if (!schema.range) return shape;
    if (shape.type !== "number") return null;
    const minimum = Math.max(shape.minimum, schema.range.minimum),
      maximum = Math.min(shape.maximum, schema.range.maximum);
    return minimum <= maximum ? { type: "number", minimum, maximum } : null;
  }
  if (schema.shape === "unit") return { type: "null" };
  if (schema.shape === "boolean") return { type: "boolean" };
  if (schema.shape === "string")
    return { type: "string", maximumBytes: 1048576 };
  if (schema.shape === "number")
    return {
      type: "number",
      minimum: schema.range?.minimum ?? -Number.MAX_VALUE,
      maximum: schema.range?.maximum ?? Number.MAX_VALUE,
    };
  return null;
}
