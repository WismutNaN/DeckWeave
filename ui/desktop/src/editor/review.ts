// SPDX-License-Identifier: GPL-3.0-or-later
import { equal } from "./bindings";
import { layout } from "./appearance";
import type { GraphDocument, DraftDiagnostic } from "./types";
export function summarizeChanges(
  before: GraphDocument | null,
  after: GraphDocument,
) {
  const previous = before?.graph.nodes ?? [],
    next = after.graph.nodes;
  return {
    added: next.filter(
      (n) => !previous.some((p) => p.id === n.id) && n.kind.type === "action",
    ).length,
    removed: previous.filter(
      (n) => !next.some((p) => p.id === n.id) && n.kind.type === "action",
    ).length,
    changed: next.filter(
      (n) =>
        n.kind.type === "action" &&
        previous.some((p) => p.id === n.id) &&
        (!equal(
          n,
          previous.find((p) => p.id === n.id),
        ) ||
          !equal(
            before?.graph.actionArguments?.[n.id],
            after.graph.actionArguments?.[n.id],
          ) ||
          before?.graph.nodeInstances?.[n.id] !==
            after.graph.nodeInstances?.[n.id]),
    ).length,
    connections: !equal(
      before?.pluginInstances ?? {},
      after.pluginInstances ?? {},
    ),
    layout: !equal(before ? layout(before) : null, layout(after)),
    routing: !equal(before?.graph.edges ?? [], after.graph.edges),
  };
}
export function diagnosticMessage(
  diagnostic: DraftDiagnostic,
  lang: "ru" | "en",
) {
  const messages: Record<string, [string, string]> = {
    invalid_argument: [
      "Параметр отсутствует или несовместим со схемой. Проверьте значение и источник данных.",
      "Missing or incompatible argument. Check its value and data source.",
    ],
    invalid_value: [
      "Проверьте значение и конфигурацию подключения.",
      "Check the value and connection configuration.",
    ],
    missing_permission: [
      "Плагину не выдано необходимое разрешение.",
      "The plugin is missing a required permission.",
    ],
    missing_resource: [
      "Необходимый ресурс недоступен.",
      "A required resource is unavailable.",
    ],
    unresolved_endpoint: [
      "Плагин не объявил этот вход или действие.",
      "The plugin has not declared this input or action.",
    ],
    unsupported_endpoint: [
      "Это действие не поддерживается плагином.",
      "This action is unsupported by the plugin.",
    ],
    endpoint_unavailable: [
      "Устройство или действие сейчас недоступно.",
      "The device or action is currently unavailable.",
    ],
    schema_mismatch: [
      "Формат, единицы или диапазон данных не совпадают.",
      "Data format, units or range do not match.",
    ],
    graph_cycle: [
      "Сценарий содержит замкнутую зависимость.",
      "The workflow contains a dependency cycle.",
    ],
    package_lock_mismatch: [
      "Нужна установленная версия плагина, указанная в workspace.",
      "The workspace's pinned plugin version must be installed.",
    ],
  };
  return (
    messages[diagnostic.code]?.[lang === "ru" ? 0 : 1] ??
    (lang === "ru"
      ? "Не удалось проверить сценарий: "
      : "Workflow validation failed: ") + diagnostic.code
  );
}
