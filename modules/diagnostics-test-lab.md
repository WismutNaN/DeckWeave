# Модуль: Diagnostics & Test Lab

**Ответственность:** обеспечивает structured metrics/logs, trace correlation, event recording/replay, virtual devices и conformance fixtures.
**Расположение:** `crates/deckweave-testkit/`, `fixtures/`, `tests/`.

## Публичный интерфейс

| Символ | Тип | Описание |
|---|---|---|
| `TraceContext` | struct | Trace/correlation/causation |
| `MetricsSnapshot` | struct | Latency/queues/drops/memory |
| `EventRecorder` | service | Bounded opt-in capture |
| `ReplayRunner` | service | Fake-clock deterministic replay |
| `VirtualDeviceProvider` | test plugin | Hardware-independent source/target |
| `ConformanceSuite` | harness | Plugin/device/API contract tests |
| `DiagnosticBundle` | export | Redacted selected evidence |

## Зависимости

| Модуль | Что использует |
|---|---|
| Domain & Protocol | Envelopes/traces |
| Все runtime modules | Instrumentation interfaces |

## Инварианты

- Diagnostics path bounded и не блокирует hot path.
- Raw input/notification/window/media content opt-in.
- Secrets redacted before persistence/export.
- Replay preserves ordering/time semantics.
- Virtual fixtures do not require user hardware.
- Compatibility claims cite exact fixture/version.

## Намеренно НЕ обрабатывает

- user analytics/cloud telemetry;
- infinite event history;
- silent capture of sensitive content.

## Заметки для агента

> Testkit создаётся рано, а не после features. Каждый hardware provider должен иметь protocol fixtures, virtual fallback and manual acceptance checklist.

