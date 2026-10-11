# Модуль: Diagnostics & Test Lab

**Ответственность:** обеспечивает structured metrics/logs, trace correlation, event recording/replay, virtual devices и conformance fixtures.
**Расположение:** production instrumentation/recorder в `crates/deckweave-diagnostics/`; replay fixtures/fakes/conformance harness в `crates/deckweave-testkit/`, `fixtures/`, `tests/`. Production simulation/replay исполняет `SimulationRunner` из Automation Engine.

## Публичный интерфейс

| Символ | Тип | Описание |
|---|---|---|
| `TraceContext` | struct | Trace/correlation/causation |
| `MetricsSnapshot` | struct | Latency/queues/drops/memory |
| `EventRecorder` | service | Bounded opt-in capture |
| `ReplayRunner` | test harness | Fixture runner поверх production `SimulationRunner` из Automation Engine |
| `VirtualDeviceProvider` | test plugin | Hardware-independent source/target |
| `ConformanceSuite` | harness | Plugin/device/API contract tests |
| `DiagnosticBundle` | export | Redacted selected evidence |

## Зависимости

| Модуль | Что использует |
|---|---|
| Domain & Protocol | Envelopes/traces |
| Runtime modules, только из testkit | Replay/fault/conformance adapters; production diagnostics не импортирует runtime implementations |

## Инварианты

- Diagnostics path bounded и не блокирует hot path.
- Raw input/notification/window/media content opt-in.
- Secrets redacted before persistence/export.
- Replay preserves ordering/time semantics.
- Virtual fixtures do not require user hardware.
- Compatibility claims cite exact fixture/version.
- Production crates не импортируют testkit; runtime пишет в узкий diagnostics sink.
- Fixture записывает ingress order, coalescing, timer firings, completions и initial state; перестановка внешних outcomes не обязана давать тот же state.
- Default replay использует fake targets без live effects, diagnostics export не исполняет записанные commands.

## Намеренно НЕ обрабатывает

- user analytics/cloud telemetry;
- infinite event history;
- silent capture of sensitive content.

## Заметки для агента

> Testkit создаётся рано, а не после features. Каждый hardware provider должен иметь protocol fixtures, virtual fallback and manual acceptance checklist.

Разделение dependencies: [ADR-0013](../../decisions/0013-dependency-and-host-service-boundaries.md). Semantics replay: [контракт детерминизма](../../contracts/lifecycle.md#детерминизм-и-циклы).

Ранний conformance набор включает несколько независимых providers: observe keyboard, analog travel, irregular LEDs, screen/pointer, async result service и media template. Workflow fixtures проверяют concurrent run scope, unknown effect, artifact quota/revoke/TTL и notification gaps; lighting — fake-clock wave/cooldown/fade и конфликт output owners. Источники и gaps hardware claims фиксируются по [исследованию](../../research/implementation-review.md); success fake fixture не выдаётся за hardware или release-binary conformance.
