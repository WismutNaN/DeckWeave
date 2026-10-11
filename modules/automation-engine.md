# Модуль: Automation Engine

**Ответственность:** валидирует и компилирует workspace graph в immutable plan, затем детерминированно исполняет events, state, layers, schedules и effects.
**Расположение:** `crates/deckweave-automation/`.

## Публичный интерфейс

| Символ | Тип | Описание |
|---|---|---|
| `WorkspaceGraph` | document model | Editable semantic graph |
| `GraphCompiler` | service | Validation и compile |
| `ExecutionPlan` | immutable struct | Resolved runtime plan |
| `WorkspaceScheduler` | owner | Deterministic state transitions |
| `CommandIntent` | struct | Effect request |
| `ExecutionOutcome` | struct | Success/failure/trace |
| `Clock` | trait | Monotonic/wall/fake time |

## Зависимости

| Модуль | Что использует |
|---|---|
| Domain & Protocol | Signals/commands/schemas |
| Capability Registry | Resolved ports/leases |
| Diagnostics | Trace/replay |

## Инварианты

- Active plan immutable.
- Edge events не coalesce.
- Async effect не удерживает scheduler.
- Synchronous cycle запрещён.
- Sequence/parallel/join/error semantics явны.
- Replay с одинаковым fixture/clock даёт одинаковое state.
- Synthetic origin loop guard включён по умолчанию.

## Намеренно НЕ обрабатывает

- process lifecycle;
- profile file format;
- UI graph layout;
- raw media processing.

## Заметки для агента

> Сначала реализовать небольшой набор node kinds: source, action, state, condition, sequence, parallel, feedback, layer. Расширять через typed provider nodes, а не arbitrary callbacks внутри engine.

