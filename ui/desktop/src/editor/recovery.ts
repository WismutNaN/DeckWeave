// SPDX-License-Identifier: GPL-3.0-or-later
import type { Draft, GraphDocument, MutationEnvelope } from "./types";
import { copy } from "./api";
export type Recovery = {
  version: 1;
  base: Draft;
  document: GraphDocument;
  pending: MutationEnvelope | null;
  attempted?: boolean;
  operationId: string | null;
};
export interface RecoveryJournal {
  read(): Recovery | null;
  write(value: Recovery): void;
}
export function browserJournal(storage: Storage): RecoveryJournal {
  const key = "deckweave.editor.recovery.v1";
  return {
    read() {
      const raw = storage.getItem(key);
      if (!raw) return null;
      const value = JSON.parse(raw) as Recovery;
      if (value.version !== 1 || !value.base?.draftId || !value.document?.graph)
        throw new Error("Unsupported editor recovery record");
      return value;
    },
    write(value) {
      storage.setItem(key, JSON.stringify(value));
    },
  };
}
// Objects merge by field; arrays are atomic so graph ordering is never guessed.
// A conflicting path leaves both original documents untouched.
export function mergeDocuments(
  base: GraphDocument,
  local: GraphDocument,
  remote: GraphDocument,
): GraphDocument {
  const same = (a: unknown, b: unknown) =>
    JSON.stringify(a) === JSON.stringify(b);
  const object = (v: unknown): v is Record<string, unknown> =>
    v !== null && typeof v === "object" && !Array.isArray(v);
  function merge(b: unknown, l: unknown, r: unknown, path: string): unknown {
    if (same(l, b)) return r;
    if (same(r, b) || same(l, r)) return l;
    if (object(b) && object(l) && object(r))
      return Object.fromEntries(
        [...new Set([...Object.keys(b), ...Object.keys(l), ...Object.keys(r)])]
          .map((k) => [k, merge(b[k], l[k], r[k], path + "/" + k)])
          .filter(([, v]) => v !== undefined),
      );
    throw new Error("Конфликт изменений / Edit conflict: " + (path || "/"));
  }
  return copy(merge(base, local, remote, "")) as GraphDocument;
}
