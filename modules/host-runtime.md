# Модуль: Host Runtime

**Ответственность:** владеет lifecycle per-user host, active revision, apply coordination и согласованным snapshot системы.
**Расположение:** `crates/deckweave-host/`, composition root в `apps/desktop/src-tauri/`.

## Публичный интерфейс

| Символ | Тип | Описание |
|---|---|---|
| `HostRuntime` | struct | Composition root application core |
| `HostCommand` | enum | Start/apply/shutdown и runtime operations |
| `HostSnapshot` | immutable struct | Согласованный query snapshot |
| `ApplyPlan`, `ApplyReport` | struct | Resource diff и результат cutover |
| `LifecycleState` | enum | Starting/Running/Degraded/Stopping |

## Зависимости

| Модуль | Что использует |
|---|---|
| Domain & Protocol | IDs/envelopes/errors |
| Capability Registry | Active topology |
| Plugin Supervisor | Runtime resources |
| Automation Engine | Compiled plan |
| Profile Store | Revisions |
| Diagnostics | Health/trace |

## Инварианты

- В каждый момент есть не более одной active revision workspace.
- Old revision работает до логического cutover.
- Частичный failure не выдаётся за successful apply.
- WebView lifecycle не управляет host lifecycle.
- Shutdown имеет bounded stages и safe-output attempt.

## Намеренно НЕ обрабатывает

- hardware protocol;
- rendering details;
- plugin implementation;
- UI state и navigation.

## Заметки для агента

> Использовать сериализованного owner для mutable host state. Не держать async lock во время внешнего plugin call. Apply — orchestrated saga с prepare/cutover/cleanup, а не ACID для hardware.

