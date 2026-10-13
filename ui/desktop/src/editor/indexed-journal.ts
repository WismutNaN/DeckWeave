// SPDX-License-Identifier: GPL-3.0-or-later
import type { Recovery, RecoveryJournal } from "./recovery";
// IndexedDB keeps large drafts outside localStorage's small synchronous quota.
// A mutation awaits the transaction's complete event before crossing IPC.
export function browserJournal(legacy: Storage): RecoveryJournal {
  const key = "deckweave.editor.recovery.v1";
  const opened = new Promise<IDBDatabase>((resolve, reject) => {
    const request = indexedDB.open("deckweave-editor", 1);
    request.onupgradeneeded = () =>
      request.result.createObjectStore("recovery");
    request.onsuccess = () => resolve(request.result);
    request.onerror = () => reject(request.error);
    request.onblocked = () => reject(new Error("Recovery database is blocked"));
  });
  // Handlers are installed before load() consumes the result.
  void opened.catch(() => {});
  const validate = (value: Recovery | null): Recovery | null => {
    if (
      value &&
      (value.version !== 1 || !value.base?.draftId || !value.document?.graph)
    )
      throw new Error("Unsupported editor recovery record");
    return value;
  };
  return {
    async read() {
      const database = await opened;
      const value = await new Promise<Recovery | null>((resolve, reject) => {
        const request = database
          .transaction("recovery", "readonly")
          .objectStore("recovery")
          .get(key);
        request.onsuccess = () => resolve(request.result ?? null);
        request.onerror = () => reject(request.error);
      });
      return validate(value ?? JSON.parse(legacy.getItem(key) ?? "null"));
    },
    async write(value) {
      const database = await opened;
      await new Promise<void>((resolve, reject) => {
        const transaction = database.transaction("recovery", "readwrite", {
          durability: "strict",
        });
        transaction.oncomplete = () => resolve();
        transaction.onabort = () =>
          reject(
            transaction.error ?? new Error("Recovery transaction aborted"),
          );
        transaction.onerror = () => reject(transaction.error);
        transaction.objectStore("recovery").put(value, key);
      });
      try {
        legacy.removeItem(key);
      } catch {
        /* The committed IndexedDB record has priority. */
      }
    },
  };
}
