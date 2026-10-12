# Модуль: Automation Engine

**Ответственность:** валидирует и компилирует workspace graph в immutable plan, затем детерминированно исполняет events, state, layers, schedules и effects.
**Расположение:** `crates/deckweave-automation/`. Planning contract создан в Фазе 4; compiler/scheduler/runtime работают с Фазы 5, structural bindings расширены в pre-6.

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
| `WorkspaceScheduler` | pure owner | Bounded ordered ingress, deterministic state transitions и intents без process/network/filesystem I/O |
| `CommandIntent` | struct | Effect request |
| `Invocation`, `RunResult` | scoped types | Typed inputs/results, progress и terminal job outcome |
| `DataEdge`, `ResultBinding` | compiled references | Связь поля результата с входом шага конкретного run |
| `ExecutionOutcome` | struct | Success/failure/trace |
| `Clock` | trait | Monotonic/wall/fake time |
| `SimulationRunner`, `SimulationFixture` | service/document | Тот же compiled plan и scheduler с isolated state, fake ticks и fixture outcomes вместо live dispatcher |
| `SimulationTrace`, `SimulationLatency` | result | Bounded redacted trace и p50/p95/p99 baseline по четырём границам |

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
- Replay state не переживает отдельный run; fixture version/order/budget и safe trace IDs проверяются до исполнения.
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

M1 не доказывает готовность сложных plugin workflows. Перед editor нужны [structured bindings](../../contracts/automation.md#structured-bindings), live clock/cleanup и configured instances по [pre-6 gate](../../implementation/pre-phase-6.md). Полные layers/sequence/join остаются Фазой 11. Domain-specific HTTP/task/window logic не добавляется в graph enum.

> Фаза 5: source/action/state/condition/feedback; sequence/parallel/layer добавляются в Фазе 11. Следовать [execution lifecycle](../../contracts/lifecycle.md#executions-cancellation-и-смена-плана) и [ordering/replay](../../contracts/lifecycle.md#порядок-сигналов-и-время). Plugin conditions — async boundaries с deadline, не callbacks внутри scheduler.

`GraphDocument` v1 уже задаёт source/action/state/condition/feedback, typed flow/data ports, stable node/edge/port IDs и отдельный layout. Валидация проверяет version, shape, references, direction, exact schema и limits; cycles и descriptor catalog относятся к compiler среза 5.2. Неизвестный `GraphNodeKind` не сохраняется как extension и не пропускается. Runtime limits для payload/queues/runs/invocations/timers/trace определены общей policy и расходуются единым `WorkspaceScheduler`; plan сохраняет минимальные schema contracts для проверки результатов без обращения к live registry.

`GraphPlanCompiler` использует immutable `DescriptorCatalog` со schema value kind/unit/range, static port/operation descriptors и точными runtime requirements. Он строит одинаковый semantic hash независимо от порядка JSON nodes/edges и UI layout, отклоняет cycles, missing metadata, несовпадающие directions/schemas/package lock и отсутствующие compile permissions/resources. Deferred availability остаётся diagnostic; required availability — compile error. `RuntimeBinder` не компилирует повторно: он принимает IR и фактический snapshot, проверяет stable identity, точные runtime group/contribution, session generation, descriptor revision, permissions и operation contract. Production prepare строит binding из authenticated process session и полученных descriptors, а barrier повторно проверяет binding и что подготовленные процессы ещё живы.

Живой scheduler не переехал в Plugin Runtime: Host-owned owner хранит `WorkspaceScheduler`, назначает `(HostBootId, ActivationEpoch, IngressOrdinal)` и выполняет возвращённые intents через injected dispatcher. `WorkspaceScheduler::reopen_after_barrier` применяется только при pre-commit abort: pending effects уже отменены и никогда не replay; semantic plan и state slots остаются прежними.

`RestrictedFixtureCompiler` остаётся в совместимых test harness. Desktop composition использует `PackageGraphCompiler`: он читает manifest-hashed static catalog из пакетов выбранного lock и передаёт его `GraphPlanCompiler`. Metadata inspection не запускает plugin code. Production `GraphPlanCompiler` уже получает immutable descriptor/schema catalog, compile access, package lock и revision fence и выдаёт полный executable IR. Оба compiler проходят общий `PlanCompiler` boundary; Host и Plugin Runtime не интерпретируют JSON nodes. Checksum-версия `DurableExecutionPlan` входит в apply intent/commit и используется prepare/recovery. Для старого commit без extension `PlanCompiler::compile_recovery` принимает immutable revision и сохранённые hashes; присутствующий повреждённый plan всегда отклоняется, а не перекомпилируется.

Input/result/job schemas закладываются в Фазе 2; начальный invocation path — в Фазе 5, композиция и artifact/collection fixtures — в Фазе 11. [Automation contract](../../contracts/automation.md) является источником правил передачи результатов и работы слоёв; [сценарии](../../product/scenarios.md) задают сквозную приёмку.

Production simulate/replay использует `SimulationRunner` этого модуля: все effects попадают в bounded capability-free recorder, outcomes/timers/faults/order/random seed берутся из fixture. Если внешнего результата нет, report помечает ветвь `Inconclusive`; при переполнении ingress или recorder — `Overloaded` с cleanup evidence. Versioned trace содержит plan/schema hashes, fake ticks, correlation/causation, coalescing и machine outcomes, но не payload. Testkit лишь собирает conformance/fault harness поверх этого публичного порта; UI/API не импортируют testkit. Текущий baseline: [Фаза 5 replay/latency](../../implementation/phase-5-latency-baseline.md).
