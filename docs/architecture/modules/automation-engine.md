# Модуль: Automation Engine

**Ответственность:** валидирует и компилирует workspace graph в immutable plan, затем детерминированно исполняет events, state, layers, schedules и effects.
**Расположение:** `crates/deckweave-automation/`. Planning contract и ограниченный fixture compiler созданы в Фазе 4; scheduler/runtime появляются в Фазе 5.

## Публичный интерфейс

| Символ | Тип | Описание |
|---|---|---|
| `GraphDocument`, `GraphDefinition` | document model | Versioned editable graph; semantic nodes/edges отделены от layout и extensions |
| `GraphLimits`, `GraphValidationError` | policy/value | Bounded document/runtime budgets и адресные diagnostics без усечения |
| `DescriptorCatalog`, `GraphPlanCompiler` | immutable input/service | Static schema/endpoint/operation evidence, semantic validation и canonical DAG compile без запуска plugin |
| `ExecutableGraph` | immutable IR | Sorted nodes/edges, topological order, source index, state slots и endpoint contracts; layout исключён |
| `RuntimeBinder` | pure service | Связывает stable endpoints с session/generation/revision и повторно проверяет runtime descriptors |
| `PlanCompiler` | port | Компилирует immutable `WorkspaceCandidate` при зафиксированных registry/grant revisions |
| `ExecutionPlan` | immutable struct | Resolved runtime plan |
| `RuntimeRequirement`, `DurableExecutionPlan` | immutable structs | Точная package/version/hash/group/contribution потребность и checksum envelope для recovery |
| `WorkspaceScheduler` | owner | Deterministic state transitions |
| `CommandIntent` | struct | Effect request |
| `Invocation`, `RunResult` | scoped types | Typed inputs/results, progress и terminal job outcome |
| `DataEdge`, `ResultBinding` | compiled references | Связь поля результата с входом шага конкретного run |
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
- Неизвестный executable node отклоняется; extension data не становится скрытой executable semantics.
- UI layout не входит в semantic graph и не влияет на compiled plan.
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
- Results run-local; acceptance не разблокирует sequence. Join/skip/null/error/reentrancy policies явны.
- Большой результат — RuntimeArtifactRef через injected broker port; engine не читает произвольные paths.
- Input layer tokens, held output owners и reactive effect intents очищаются по epoch/session; frame ticks исполняет compositor.

## Намеренно НЕ обрабатывает

- process lifecycle;
- profile file format;
- интерпретацию UI graph layout при исполнении;
- raw media processing.

## Заметки для агента

> Фаза 5: source/action/state/condition/feedback; sequence/parallel/layer добавляются в Фазе 11. Следовать [execution lifecycle](../../contracts/lifecycle.md#executions-cancellation-и-смена-плана) и [ordering/replay](../../contracts/lifecycle.md#порядок-сигналов-и-время). Plugin conditions — async boundaries с deadline, не callbacks внутри scheduler.

`GraphDocument` v1 уже задаёт source/action/state/condition/feedback, typed flow/data ports, stable node/edge/port IDs и отдельный layout. Валидация проверяет version, shape, references, direction, exact schema и limits; cycles и descriptor catalog относятся к compiler среза 5.2. Неизвестный `GraphNodeKind` не сохраняется как extension и не пропускается. Runtime limits для queues/runs/invocations/timers/trace определены той же policy, но расходуются scheduler в 5.3.

`GraphPlanCompiler` использует immutable `DescriptorCatalog` со schema value kind/unit/range, static port/operation descriptors и точными runtime requirements. Он строит одинаковый semantic hash независимо от порядка JSON nodes/edges и UI layout, отклоняет cycles, missing metadata, несовпадающие directions/schemas/package lock и отсутствующие compile permissions/resources. Deferred availability остаётся diagnostic; required availability — compile error. `RuntimeBinder` не компилирует повторно: он принимает IR и фактический snapshot, проверяет stable identity, session generation, descriptor revision, permissions и operation contract. Production-вызов binder на prepare/barrier подключается в 5.4.

Временный `RestrictedFixtureCompiler` не является урезанной реализацией произвольного graph: он принимает пустой workspace без packages/assets либо точную virtual fixture с kind `virtual_echo`/`virtual_disruptive_echo` и одним pinned `org.deckweave.example.virtual`. Любой другой document/lock возвращает `UnsupportedGraph`. Результат уже не содержит fixture enum: compiler выдаёт точный `RuntimeRequirement`, а checksum-версия `DurableExecutionPlan` входит в apply intent/commit и используется prepare/recovery. Для commit, созданного до появления этого extension, `PlanCompiler::compile_recovery` принимает immutable revision и сохранённые candidate/package hashes; это migration seam владельца graph vocabulary, а не разрешение Host/runtime разбирать graph. Присутствующий повреждённый extension всегда отклоняется. Текущий `PlanCompiler` получает revision fence; настоящий compiler также потребует immutable descriptor/schema catalog и полный executable IR. Graph vocabulary остаётся внутри Automation; Host и Plugin Runtime не интерпретируют JSON nodes.

Input/result/job schemas закладываются в Фазе 2; начальный invocation path — в Фазе 5, композиция и artifact/collection fixtures — в Фазе 11. [Automation contract](../../contracts/automation.md) является источником правил передачи результатов и работы слоёв; [сценарии](../../product/scenarios.md) задают сквозную приёмку.

Production simulate/replay использует `SimulationRunner` этого модуля: все effects превращаются в записанные intents, outcomes берутся из fixture. Если внешнего результата нет, report помечает ветвь `inconclusive`, не вызывает live provider. Testkit лишь собирает conformance/fault harness поверх этого публичного порта; UI/API не импортируют testkit.
