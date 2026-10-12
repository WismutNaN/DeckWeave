# Модуль: Host Runtime

**Ответственность:** владеет lifecycle per-user host, active revision, apply coordination и согласованным snapshot системы. Application dispatch, principal authorization и API error translation принадлежат Application Gateway/Local API.
**Расположение:** `crates/deckweave-host/`, composition root в `apps/desktop/src-tauri/`.

## Публичный интерфейс

| Символ | Тип | Описание |
|---|---|---|
| `HostRuntime` | struct | Composition root application core |
| `HostCommand` | enum | Start/apply/shutdown и runtime operations |
| `HostSnapshot` | immutable struct | Согласованный query snapshot |
| `ApplyPlan`, `ApplyReport` | struct | Resource diff и результат cutover |
| `ApplyPlanningService`, `PlanToken` | service/value | Candidate compile, prepare и повторная проверка fenced preconditions перед barrier |
| `PlanningEvidenceSource`, `RuntimeActivation` | ports | Снимок boot/topology/grants/active selection и prepare без effects |
| `LifecycleState` | enum | Starting/Running/Degraded/Stopping |
| `ActivationEpoch`, `ApplyCommitRecord` | value/document | Runtime generation и durable точка выбора revision |
| `ArtifactBroker` | application service | Scoped bounded runtime artifacts, read leases, TTL/quota и cleanup |

## Зависимости

| Модуль | Что использует |
|---|---|
| Domain & Protocol | IDs/envelopes/errors |
| Capability Registry | Active topology |
| Plugin Supervisor | Runtime resources |
| Automation Engine | Compiled plan |
| Profile Store | Revisions |
| Diagnostics | Health/trace |
| Application contracts | Read-only `HostStatusSource`; Gateway подключается composition root |
| Host Platform Services | Injected locks, data roots, credentials и process cleanup |

## Инварианты

- V1 имеет один host на OS user session и один active workspace; Store защищён writer lock.
- Old revision работает до barrier; disruptive exclusive handoff требует явной downtime policy.
- Candidate/assets/package lock сохраняются до commit; после durable commit новая revision остаётся выбранной даже при degraded recovery.
- Effects/timers/frames/completions ограждены HostBootId/ActivationEpoch; старый outcome не запускает новую continuation.
- Частичный failure не выдаётся за successful apply.
- Plan hash связывает candidate/package/assets, compiler output, HostBootId, registry/grant revisions, expected active selection и monotonic expiry.
- Prepare и barrier заново читают candidate/evidence; stale или expired plan не вызывает activation/commit.
- WebView lifecycle не управляет host lifecycle.
- Shutdown имеет bounded stages и safe-output attempt.
- Runtime artifacts отделены от immutable profile assets; broker проверяет consumer grants и startup GC, journal не сохраняет private payload.

## Намеренно НЕ обрабатывает

- draft command dispatch и authorization policy;
- transport/application error mapping;
- hardware protocol;
- rendering details;
- plugin implementation;
- UI state и navigation.

## Заметки для агента

> Использовать сериализованного owner для mutable host state. Не держать async lock во время внешнего plugin call. Обязательны [порядок apply/crash recovery](../../contracts/lifecycle.md#сохранение-и-apply) и bounded cleanup held outputs. После commit rollback — новая operation, не обещание отмены внешнего эффекта.

`deckweave-host` не реализует `ApplicationService`: Gateway создаётся отдельно в composition root и получает read-only lifecycle projection и bounded-context repositories.

Текущая реализация содержит startup locks/status/autostart и D-срез planning seam. Profile adapter уже реализует port immutable apply intent/status/active commit, но production Host coordinator и публичный apply path ещё подключаются следующим срезом E1/C2. `ApplyPlanningService` использует явно ограниченный fixture compiler; Фаза 5 заменяет его настоящим без изменения fencing/acceptance. Путь command → virtual process → revoke/shutdown/recovery проверяется через composition приложения, включая работу при закрытом WebView.

ArtifactBroker реализует узкий application port с injected temporary-storage adapter; engine не зависит от Host implementation. Limits и lifecycle: [automation artifacts](../../contracts/automation.md#artifact-и-доступ-к-данным). Это небольшой сервис host, не новый обязательный процесс или database.
