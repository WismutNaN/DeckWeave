// SPDX-License-Identifier: GPL-3.0-or-later
import type { GraphDocument } from "./types";
const sizes = new WeakMap<object, number>();
function size(value: GraphDocument) {
  let result = sizes.get(value);
  if (result === undefined) {
    result = JSON.stringify(value).length * 2;
    sizes.set(value, result);
  }
  return result;
}
export function pushHistory(
  history: GraphDocument[],
  value: GraphDocument,
  budget = 32 * 1024 * 1024,
) {
  const result = [...history, value];
  let used = 0,
    start = result.length - 1;
  for (let i = result.length - 1; i >= 0; i--) {
    const next = size(result[i]);
    if (
      i !== result.length - 1 &&
      (used + next > budget || result.length - i > 64)
    )
      break;
    used += next;
    start = i;
  }
  return result.slice(start);
}
