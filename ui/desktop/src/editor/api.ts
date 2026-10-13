// SPDX-License-Identifier: GPL-3.0-or-later
import { invoke } from "@tauri-apps/api/core";
import type { Draft, Locale, MutationEnvelope } from "./types";
export interface EditorClient {
  query<T>(query: Record<string, unknown>): Promise<T>;
  command<T>(envelope: MutationEnvelope): Promise<T>;
  open(): Promise<Draft>;
  window(): Promise<{ dedupEpoch: string }>;
  locale(): Promise<Locale>;
  reconcileLocale(): Promise<{ context: Locale; deliveryPending: boolean }>;
  changeLocale(
    expected: number,
    next: Locale,
  ): Promise<{ context: Locale; deliveryPending: boolean }>;
}
export const desktopClient: EditorClient = {
  query: (query) => invoke("app_query", { query }),
  command: (envelope) => invoke("app_command", { envelope }),
  open: () => invoke("open_workspace"),
  window: () => invoke("mutation_window"),
  locale: () => invoke("locale_get"),
  reconcileLocale: () => invoke("locale_reconcile"),
  changeLocale: (expected, next) => invoke("locale_change", { expected, next }),
};
export const copy = <T>(value: T): T => JSON.parse(JSON.stringify(value)) as T;
export function message(cause: unknown, lang: string): string {
  const error = cause as {
    code?: string;
    details?: { reason?: string };
    message?: string;
    messageKey?: string;
  };
  const keys: Record<string, [string, string]> = {
    "api.error.document_too_large": [
      "Черновик превышает лимит размера хранилища. Уменьшите объём конфигурации; предыдущая версия сохранена.",
      "The draft exceeds the storage size limit. Reduce the configuration size; the previous version is preserved.",
    ],
    "api.error.virtual_stale": [
      "Устройство переподключилось или конфигурация изменилась. Обновите состояние и повторите нажатие.",
      "The device reconnected or the configuration changed. Refresh before clicking again.",
    ],
    "api.error.virtual_unsupported": [
      "Для этой кнопки нет применённого виртуального входа.",
      "This control has no applied virtual input.",
    ],
    "api.error.virtual_unavailable": [
      "Виртуальное устройство сейчас недоступно.",
      "The virtual device is currently unavailable.",
    ],
  };
  if (error?.messageKey && keys[error.messageKey])
    return keys[error.messageKey][lang === "ru" ? 0 : 1];
  const reasons: Record<string, [string, string]> = {
    revision_conflict: [
      "Черновик изменён в другом окне. Обновите его перед сохранением.",
      "The draft changed elsewhere. Reload it before saving.",
    ],
    permission_denied: [
      "Для этого действия требуется разрешение.",
      "This action requires permission.",
    ],
    plan_conflict: [
      "Конфигурация не готова к применению.",
      "The configuration is not ready to apply.",
    ],
    unavailable: [
      "Нет связи с ядром. Изменения остались в редакторе.",
      "The host is unavailable. Your edits remain in the editor.",
    ],
  };
  const translated =
    error?.code && reasons[error.code]?.[lang === "ru" ? 0 : 1];
  return [
    translated || error?.message || error?.messageKey || String(cause),
    error?.details?.reason,
  ]
    .filter(Boolean)
    .join(" ");
}
