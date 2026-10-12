// SPDX-License-Identifier: GPL-3.0-or-later
import { initialValue, schemaFor } from "./bindings";
import { valueShape } from "./variables";
import type { Catalog, GraphDocument, InstanceTemplate } from "./types";
export const INSTANCE_LABEL = "org.deckweave.editorLabel";
export function createInstance(
  document: GraphDocument,
  catalog: Catalog,
  template: InstanceTemplate,
  label: string,
  id = crypto.randomUUID(),
) {
  const schema = schemaFor(catalog, template.settingsSchema),
    shape = schema && valueShape(schema);
  if (!shape)
    throw new Error(
      "Схема настройки не поддерживается / Unsupported settings schema",
    );
  document.pluginInstances ??= {};
  if (
    Object.keys(document.pluginInstances).length >= 64 ||
    document.pluginInstances[id]
  )
    throw new Error("Instance limit or duplicate ID");
  document.pluginInstances[id] = {
    formatVersion: 1,
    instanceId: id,
    packageId: catalog.package.packageId,
    contributionId: template.contributionId,
    settingsRevision: 1,
    settingsSchema: template.settingsSchema,
    settings: initialValue(shape),
    credentials: {},
    requiredFeatures: { ...template.requiredFeatures },
    [INSTANCE_LABEL]: label.trim().slice(0, 80) || id,
  };
  return id;
}
export function deleteInstance(document: GraphDocument, id: string) {
  if (Object.values(document.graph.nodeInstances ?? {}).includes(id))
    throw new Error(
      "Подключение используется в действиях / This connection is used by actions",
    );
  delete document.pluginInstances?.[id];
}
export const instanceLabel = (
  instance: NonNullable<GraphDocument["pluginInstances"]>[string],
) =>
  typeof instance[INSTANCE_LABEL] === "string"
    ? (instance[INSTANCE_LABEL] as string)
    : instance.instanceId;
