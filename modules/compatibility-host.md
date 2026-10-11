# Модуль: Compatibility Host

**Ответственность:** запускает legacy OpenAction/Elgato packages и переводит manifests, contexts, events, settings и feedback в модель DeckWeave.
**Расположение:** `runtimes/openaction-host/`.

## Публичный интерфейс

| Символ | Тип | Описание |
|---|---|---|
| `CompatibilityScanner` | service | C0 indexing/report |
| `LegacyPluginSession` | runtime handle | WebSocket lifecycle |
| `LegacyContextMap` | map/service | Context ↔ Binding |
| `ManifestAdapter` | service | Legacy actions → contributions |
| `CompatibilityReport` | document | Levels/features/issues |
| `PropertyInspectorBroker` | service | Scoped PI messaging |

## Зависимости

| Модуль | Что использует |
|---|---|
| Plugin Supervisor | Process lifecycle |
| Capability Registry | Target ports |
| Surface Compositor | Image/title/state feedback |
| Profile Store | Namespaced settings |

## Инварианты

- Legacy IDs не становятся core IDs.
- Compatibility level относится к exact package/OS/host versions.
- PI не получает DeckWeave privileged API.
- Unsupported event диагностируется, а не silently accepted.
- Host запускается только при active legacy lease.

## Намеренно НЕ обрабатывает

- автоматический source conversion;
- undocumented full Elgato emulation;
- Companion bridge;
- native DeckWeave plugins.

## Заметки для агента

> Реализация опирается на published protocols и versioned open fixtures. GPL reuse допускается только с provenance/license review и не должно затягивать Stream Deck domain в core.

