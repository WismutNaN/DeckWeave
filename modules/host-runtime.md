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
| `ActivationEpoch`, `ApplyCommitRecord` | value/document | Runtime generation и durable точка выбора revision |

## Зависимости

| Модуль | Что использует |
|---|---|
| Domain & Protocol | IDs/envelopes/errors |
| Capability Registry | Active topology |
| Plugin Supervisor | Runtime resources |
| Automation Engine | Compiled plan |
| Profile Store | Revisions |
| Diagnostics | Health/trace |
| Application contracts | Реализуемые commands/queries/service ports |
| Host Platform Services | Injected locks, data roots, credentials и process cleanup |

## Инварианты

- V1 имеет один host на OS user session и один active workspace; Store защищён writer lock.
- Old revision работает до barrier; disruptive exclusive handoff требует явной downtime policy.
- Candidate/assets/package lock сохраняются до commit; после durable commit новая revision остаётся выбранной даже при degraded recovery.
- Effects/timers/frames/completions ограждены HostBootId/ActivationEpoch; старый outcome не запускает новую continuation.
- Частичный failure не выдаётся за successful apply.
- WebView lifecycle не управляет host lifecycle.
- Shutdown имеет bounded stages и safe-output attempt.

## Намеренно НЕ обрабатывает

- hardware protocol;
- rendering details;
- plugin implementation;
- UI state и navigation.

## Заметки для агента

> Использовать сериализованного owner для mutable host state. Не держать async lock во время внешнего plugin call. Обязательны [порядок apply/crash recovery](../docs/RUNTIME_CONTRACTS.md#сохранение-и-apply) и bounded cleanup held outputs. После commit rollback — новая operation, не обещание отмены внешнего эффекта.
