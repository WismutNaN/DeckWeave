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
| `SimulationRunner` | service | Тот же scheduler с isolated state и fixture outcomes вместо live dispatcher |

## Зависимости

| Модуль | Что использует |
|---|---|
| Domain & Protocol | Signals/commands/schemas |
| Registry snapshot port | Immutable resolved ports/leases; без управления mutable Registry |
| Diagnostics | Production trace sink; ReplayRunner находится в testkit |

## Инварианты

- Active plan immutable.
- Edge events не coalesce.
- Async effect не удерживает scheduler.
- Любой cycle пересекает положительный delay/новое внешнее событие и ограничен execution budget; один state/latch недостаточен.
- Sequence/parallel/join/error semantics явны.
- Replay с одинаковым fixture/clock даёт одинаковое state.
- Synthetic origin loop guard включён по умолчанию.
- Encoder delta и touch lifecycle не coalesce; value segments не пересекают edge/barrier.
- Late continuation другой epoch не меняет active state; cancel не означает rollback эффекта.
- Held outputs/layers принадлежат owner lease и освобождаются независимо от нового mapping.
- Unknown outcomes не повторяются без provider idempotency; simulation не вызывает live targets.

## Намеренно НЕ обрабатывает

- process lifecycle;
- profile file format;
- UI graph layout;
- raw media processing.

## Заметки для агента

> Фаза 5: source/action/state/condition/feedback; sequence/parallel/layer добавляются в Фазе 11. Следовать [execution lifecycle](../docs/RUNTIME_CONTRACTS.md#executions-cancellation-и-смена-плана) и [ordering/replay](../docs/RUNTIME_CONTRACTS.md#порядок-сигналов-и-время). Plugin conditions — async boundaries с deadline, не callbacks внутри scheduler.

Production simulate/replay использует `SimulationRunner` этого модуля: все effects превращаются в записанные intents, outcomes берутся из fixture. Если внешнего результата нет, report помечает ветвь `inconclusive`, не вызывает live provider. Testkit лишь собирает conformance/fault harness поверх этого публичного порта; UI/API не импортируют testkit.
