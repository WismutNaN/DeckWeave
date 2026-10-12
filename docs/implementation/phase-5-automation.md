# Фаза 5: минимальный automation engine

Статус: срезы 5.0–5.3 завершены; следующий срез — production orchestration 5.4. Основание и исходные дефекты — [повторная проверка apply](../research/pre-apply-review.md#phase-5-readiness). Фаза 4 создала рабочую основу для apply и восстановления isolated fixture; общий command claim, superseded recovery и отдельный pending budget теперь закрывают найденные P1-переходы.

## Граница результата

M1 показывает путь `virtual source → condition → isolated action → typed result → state → virtual feedback` через public application service. Тот же план и scheduler исполняются в simulation с recorded outcomes. Apply во время pending action, restart provider, revoke, overflow и malformed result не приводят к stale effects или потерянному cleanup.

Фаза не включает Graph UI, полноценные sequence/parallel/join/layers/schedules, реальный Handy/Telegram/browser, renderer, firmware programming и media RT. Их contracts сохраняются как ограничения расширения. Внутренний pending invocation уже нужен: accepted job не считается результатом даже у одного action. Plugin-defined transform — async operation, а простая built-in condition остаётся чистой функцией.

## Владение кодом и состоянием

| Контекст | Что добавляется | Чего он не делает |
|---|---|---|
| Automation | `GraphDocument`, semantic validation, compiler IR, scheduler, run/invocation state, intents, replay | Не читает profile files, не управляет process и не вызывает OS |
| Workspace | Immutable candidate и структурная валидность сохранённого документа | Не реализует второй graph compiler |
| Registry | Immutable descriptor/availability snapshot, разрешение stable endpoints, session fencing | Не выбирает automation actions |
| Host | Activation epoch, ingress order, apply barrier, подключение scheduler и dispatcher | Не разбирает vocabulary графа и JSON fixture |
| Plugin Runtime | Package/group sessions, protocol translation, delivery и outcomes, lifecycle notifications | Не исполняет graph branches и не владеет run state |
| Gateway | Commands/queries/scopes и DTO для plan/run/trace | Не дублирует graph invariants |

Graph vocabulary принадлежит Automation; shared endpoint/IDs/schema references остаются в существующем Shared Kernel. `WorkspaceCandidate.document` — boundary, из которого compiler создаёт проверенный typed graph. Не вводить обратную зависимость Workspace → Automation и не складывать все graph nodes в `deckweave-domain`.

Ориентировочное разбиение внутри `deckweave-automation`: `graph`, `compile`, `plan`, `scheduler`, `invocation`, `trace`, `simulation`. Это не требование заранее создать пустые модули. Новый файл появляется вместе с самостоятельной ответственностью и проверкой.

## Срезы реализации

Каждый срез — отдельный reviewable commit или небольшая серия; перед началом оценить 1–5 дней и разделить, если acceptance не помещается. 5.1 и pure часть 5.2–5.3 не зависят от исправления persistence в 5.0. Production integration 5.4–5.5 зависит от закрытия 5.0–5.3.

### 5.0 — Закрыть обнаруженные переходы Фазы 4

- [x] Единый durable key scope для `PatchDraft` и `ApplyDraft`: Gateway резервирует opaque fingerprint в общем `CommandClaimRepository` до dispatch; command kind, target и preconditions входят в fingerprint. Оба порядка и restart дают conflict.
- [x] Recovery различает never-committed pending, текущую committed selection и ранее committed operation. Profile Store запечатывает status вытесняемого head до нового commit, а Host завершает неизвестный прежний activation как `Superseded/Inactive`, не как failed-before-commit и не реактивирует её.
- [x] Recovery потоково обходит terminal history с bounded памятью и расходует `maximum` только на pending work. Regression содержит 300 terminal operations при desktop budget 256. Полный B2 index/checkpoint/GC всё ещё нужен для ограничения времени полного directory scan и storage growth.
- [x] Commit прежней версии без durable compiled plan мигрируется через отдельный recovery-вход Automation compiler из immutable revision и сохранённых hashes. Host/runtime не разбирают fixture/graph; присутствующий, но повреждённый plan не маскируется recompilation и останавливает recovery.

Выход: три отрицательных сценария из [ревью](../research/pre-apply-review.md#phase-5-readiness) становятся постоянными regression tests. Сохраняются прежние crash/revoke/handoff acceptance. Полный B2 checkpoint/GC можно оставить до Фазы 6, но исправления startup limit и key scope относятся к 5.0.

### 5.1 — Graph contract и адресуемый invocation protocol

- [x] Зафиксировать versioned `GraphDocument`: стабильные GraphNodeId, source/action/state/condition/feedback, edges и typed value references. Отделить UI layout, unknown extensions и semantic data. Неизвестный executable node возвращает unsupported; он не пропускается.
- [x] Согласовать `InvocationRequest` с `RuntimeEndpointRef`: process session задаётся authenticated transport, request содержит полный `EndpointPath`, а operation ID определяет операцию, не экземпляр устройства. Rust/TS encode/decode fixtures фиксируют путь; Host объявляет `org.deckweave.invocation.endpoint_path` v1. Будущий dispatcher обязан отправлять invocation только session, согласовавшей feature.
- [x] Определить command delivery outcomes: rejected-before-send, enqueued/possibly-sent, accepted job, terminal result. Успех `ProcessRuntime::send` означает только помещение в локальную очередь; потеря ответа после возможной передачи не даёт права повторить внешний effect.
- [x] Зафиксировать limits для graph nodes/edges, payload bytes, queued events, pending runs/invocations, timers и trace; ошибки возвращаются структурированно, без обрезания данных. Расход runtime budgets начинается вместе с owner scheduler в 5.3.

Выход: два одинаковых device targets и service с одинаковым operation/local port ID получают разные wire requests; отсутствующий target отклоняется. Short terminal и accepted-job/result проходят один vocabulary. На этом этапе достаточно encode/decode и domain fixtures.

### 5.2 — Чистый compiler и исполняемое представление

- [x] `ExecutionPlanKind::{EmptyFixture, VirtualEchoFixture}` удалён: graph compiler выдаёт точные `RuntimeRequirement`, source subscriptions, canonical nodes/edges, topological order, endpoint contracts и state slots. Checksum-версия полного plan сохраняется без UI layout.
- [x] Compiler получает immutable schema/descriptor catalog и compile access snapshot, а не только номера `CompileFence`. Проверяются direction, точный `SchemaRef`, unit/range, availability policy, missing references, permissions, resource requirements и package lock. Неявных string/number conversions нет.
- [x] Semantic compile и runtime binding разделены. Validate/plan не запускает plugin; отсутствие metadata даёт unresolved, deferred availability — diagnostic. Чистый `RuntimeBinder` связывает stable references с session/generation/revision и повторно проверяет descriptor/operation contract; включение binder в production prepare относится к 5.4.
- [x] Initial graph subset — DAG: deterministic topological order входит в IR, cycles отклоняются с node IDs. Delay/schedule cycles появятся вместе с budgets и semantics в Фазе 11. Condition остаётся pure typed instruction; чтение snapshot реализует scheduler 5.3.

Выход: валидный graph компилируется независимо от JSON object order, некорректные edges имеют diagnostics с node/edge IDs. Cold-start apply нового package не требует уже активной session и не подменяет отсутствующую metadata догадкой. Ни Host, ни Plugin Runtime не знают названия graph nodes или `fixture.kind`.

### 5.3 — Детерминированный scheduler без I/O

- [x] Один `WorkspaceScheduler` обрабатывает `IngressItem`: signal, state update, timer, invocation outcome, topology/grant fault и barrier. У каждого есть host order и `(HostBootId, ActivationEpoch)`; source clock сохраняется отдельно.
- [x] Чистый transition принимает plan, state, item и time input, возвращает новый state плюс intents. Scheduler не делает filesystem/network/process calls и не ждёт plugin внутри transition.
- [x] Reliable edges/completions/cleanup и coalescible values имеют общую семантику порядка. Coalescing допускается только внутри сегмента до следующего edge/completion/timer/barrier; транспортная latest-value map не используется как модель scheduler.
- [x] Добавлены run-local InvocationId/outcomes, один terminal outcome, rejection malformed/oversized result, deadline/cancel и budgets. Для overflow есть отдельный fault/cleanup path, которому не требуется место в уже заполненной event queue. Исполняемый plan несёт минимальные schema contracts, поэтому runtime result проверяется по типу, а не только по размеру JSON.

Выход: fake clock проверяет `axis A → button → axis B`, encoder `+1,+1,-1`, duplicate terminal, два concurrent runs, stale epoch, timeout и overflow. Одинаковый ordered fixture даёт одинаковые state/intents. Для этого не нужны OS timers и настоящий child process.

### 5.4 — Runtime plan activation и безопасный dispatch

- [~] Prepare и recovery используют один сохранённый compiled runtime manifest; Plugin Runtime больше не разбирает `fixture.kind` и не выбирает первый group/contribution. Для pre-manifest commit действует ограниченный migration path через Automation compiler; новый или повреждённый manifest не перекомпилируется. Текущий coordinator пока принимает не более одного requirement; набор нескольких groups/instances остаётся следующим расширением.
- [ ] Связать реальный ingress/command/outcome adapters с scheduler. Текущий monitor только обновляет heartbeat и отбрасывает body; его нельзя считать работающим event dispatcher. Descriptor changes, process exit, revoke и restart/backoff становятся owner events.
- [ ] Сделать barrier частью живого dispatcher: закрыть новые effects, bounded drain/cancel, release held outputs, повторно проверить grants/topology, commit, назначить новую epoch, открыть новый plan. Булевый флаг без проверяющего его dispatch path не закрывает acceptance.
- [ ] Перед отправкой каждой команды проверять актуальные grants, endpoint generation/revision, epoch и owner lease. Cleanup хранит владельца прежнего output, не вычисляет release через новый mapping. Stop/recovery имеют общий budget для всех runtime groups.
- [ ] Разделить historical operation result и текущее runtime health. Fault monitor публикует degraded/unavailable; старый `Succeeded/Applied` receipt не становится единственным источником состояния работающей системы.

Выход: через Gateway применяются два разных graph, первый с двумя package/groups, затем изменённый. Revoke/crash/apply во время pending invocation не запускает continuation новой epoch. Сохранённая revision восстанавливается после restart без replay входного события. Production acceptance не зависит от Tauri window; desktop adapter следует в Фазе 6.

### 5.5 — Вертикальная M1 fixture

- [ ] Один virtual provider публикует button/axis/state; другой contribution/process исполняет action и отдаёт typed result; feedback получает отдельный адресуемый virtual sink. Обратная связь определяется result/state graph, а не особым `echo` в Host.
- [ ] Fixture содержит два одинаковых устройства и service. Настройки/instance IDs приходят из compiled requirements, а не из выбора первого package/contribution.
- [ ] Длинная fake operation возвращает accepted job, затем terminal result. Два нажатия не смешивают данные; duplicate/late/invalid completion не порождает второй feedback. Uncertain delivery не повторяется автоматически.
- [ ] Проверить API plan/apply/query/trace, cold start, смену graph и shutdown при закрытом UI. Runtime permissions остаются fail-closed; тестовая authorization fixture не становится production allow-all default.

Выход: hardware-independent демонстрация M1 с проверяемым trace. Реальный N4 Pro, клавиатурный hook и Handy не нужны для доказательства этого пути.

### 5.6 — Replay, diagnostics и latency baseline

- [ ] `SimulationRunner` использует тот же compiler/scheduler с отдельным состоянием и dispatcher, который записывает intents и не имеет live capabilities. Outcomes, order, timer firings, grants/availability и randomness берутся из fixture; недостающий outcome даёт inconclusive.
- [ ] Versioned bounded trace хранит plan/schema hashes, correlation/causation, ingress/coalescing decisions и redacted outcomes. Private text/audio/credentials не включаются по умолчанию.
- [ ] Измерить p50/p95/p99 virtual ingress → dispatch, отдельно queue wait, scheduler step и provider completion; задать load/soak fixture и growth bounds. Сначала измерить текущий путь, затем выбирать оптимизации по узкому месту.
- [ ] Проверить slow sink, bounded memory, fault/cleanup при перегрузке и отсутствие live calls в replay. UI Test Lab и длительный recording остаются Фазой 10.

Выход: live trace воспроизводит state/intents с fake clock; повторные runs не наращивают незавершённые state/queues. Полный набор acceptance 5.0–5.6 проходит на поддерживаемой core matrix. Фаза 5 не закрывается одной успешной `source → echo` демонстрацией.

## Порядок проверки и Definition of Done

Для каждой новой способности сначала пишется отрицательный domain/port test, затем реализация и один межконтекстный scenario. Не повторять весь существующий набор ради отчёта: локально выбирать затронутые suites, полный core gate оставлять CI и проверке интеграционного среза. Новые публичные/wire поля получают compatibility fixtures.

Gate Фазы 5 закрыт, когда compiler принимает versioned graph, scheduler исполняет IR, Plugin Runtime умеет адресуемый вызов нескольких instances, live apply/recovery сохраняет epochs/owners, а replay использует тот же transition path. B2 compaction остаётся явно открытой задачей до Фазы 6; её критерии включают и стоимость quota accounting, поскольку O(1) receipt lookup не делает всю mutation O(1).

Нормативные правила: [Automation](../contracts/automation.md), [Lifecycle](../contracts/lifecycle.md), [Capabilities](../contracts/capabilities.md), [DDD](../architecture/ddd.md). Этот файл задаёт порядок реализации и evidence; он не создаёт вторую версию контрактов.
