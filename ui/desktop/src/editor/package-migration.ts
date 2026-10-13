// SPDX-License-Identifier: GPL-3.0-or-later
import { copy } from "./api";
import { layout } from "./appearance";
import { equal, schemaFor } from "./bindings";
import type {
  Catalog,
  CatalogEndpoint,
  Endpoint,
  GraphDocument,
} from "./types";

export function migrationSources(
  document: GraphDocument,
  catalogs: Catalog[],
): string[] {
  const used = new Set([
    ...document.graph.nodes.flatMap((n) =>
      n.kind.endpoint ? [n.kind.endpoint.packageId] : [],
    ),
    ...layout(document).devices.map((d) => d.package.packageId),
  ]);
  return [
    ...new Set(
      catalogs.flatMap(
        (c) => c.endpointMigrations?.map((m) => m.packageId) ?? [],
      ),
    ),
  ].filter((id) => used.has(id));
}

// This changes a draft only. Save/apply, CAS, undo, grants and package locks keep
// their ordinary owners; metadata cannot grant access or execute migration code.
export function planEndpointMigration(
  document: GraphDocument,
  catalogs: Catalog[],
  from: string,
) {
  const sources = catalogs.filter((c) => c.package.packageId === from);
  if (sources.length !== 1)
    throw new Error(
      "Нужна установленная закреплённая версия исходного плагина / The pinned source plugin must be installed",
    );
  const source = sources[0];
  const candidates = catalogs.filter((c) =>
    c.endpointMigrations?.some(
      (m) => m.packageId === from && m.version === source.package.version,
    ),
  );
  if (!candidates.length)
    throw new Error(
      "Нет совместимой версии для переноса / No compatible migration target",
    );
  if (
    Object.values(document.pluginInstances ?? {}).some(
      (i) => i.packageId === from,
    )
  )
    throw new Error(
      "Перенос подключений с настройками требует отдельного адаптера / Configured connections require a migration adapter",
    );
  const selected = new Map<string, Catalog>();
  function target(endpoint: Endpoint): {
    catalog: Catalog;
    entry: CatalogEndpoint;
  } {
    const previous = source.endpoints.find((e) => equal(e.endpoint, endpoint));
    const options = candidates.flatMap((catalog) =>
      catalog.endpoints
        .filter((e) => equal({ ...e.endpoint, packageId: from }, endpoint))
        .map((entry) => ({ catalog, entry })),
    );
    if (!previous || options.length !== 1)
      throw new Error(
        "Назначение нельзя однозначно перенести / Assignment has no unambiguous migration target",
      );
    const option = options[0],
      next = option.entry;
    const schemas = [
      previous.port.schema,
      ...(previous.operation
        ? [
            previous.operation.inputSchema,
            previous.operation.resultSchema,
            previous.operation.errorSchema,
          ]
        : []),
    ];
    if (
      !equal(previous.port, next.port) ||
      !equal(previous.operation, next.operation) ||
      schemas.some(
        (ref) =>
          !schemaFor(source, ref) ||
          !equal(schemaFor(source, ref), schemaFor(option.catalog, ref)),
      )
    )
      throw new Error(
        "Изменился контракт назначения / Assignment contract changed",
      );
    selected.set(option.catalog.package.packageId, option.catalog);
    return option;
  }
  const result = copy(document);
  let assignments = 0;
  for (const node of result.graph.nodes) {
    if (node.kind.endpoint?.packageId === from) {
      node.kind.endpoint = copy(target(node.kind.endpoint).entry.endpoint);
      assignments++;
    }
  }
  for (const device of layout(result).devices) {
    if (device.package.packageId !== from) continue;
    if (!equal(device.package, source.package))
      throw new Error(
        "Версия устройства не совпадает с закреплённым плагином / Device package does not match the pinned version",
      );
    const endpoints = source.endpoints.filter(
      (e) =>
        e.endpoint.nodeId === device.nodeId &&
        e.endpoint.target.kind === "device" &&
        e.endpoint.target.providerId === device.descriptor.providerId &&
        e.endpoint.target.deviceIdentity === device.descriptor.identity,
    );
    const destinations = endpoints.map((e) => target(e.endpoint).catalog);
    if (!destinations.length || destinations.some((c) => c !== destinations[0]))
      throw new Error(
        "Разделение устройства требует отдельного адаптера / Splitting a device requires a migration adapter",
      );
    device.package = copy(destinations[0].package);
    for (const control of Object.values(device.controls)) {
      if (control.trigger?.packageId === from)
        control.trigger = copy(target(control.trigger).entry.endpoint);
    }
  }
  return { document: result, assignments, packages: [...selected.keys()] };
}
