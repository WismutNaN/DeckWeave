# Проверка готовности к apply после DDD migration

Исторические наблюдения и промежуточные статусы ниже относятся к указанным revisions. F5-R1–R3 и B2 впоследствии закрыты; актуальные доказательства и оставшиеся границы находятся в [pre-6 gate](../implementation/pre-phase-6.md). Старые формулировки «ещё не реализовано» сохраняют контекст исходного аудита.

Дата: 2026-10-12. Исходный аудит: `2bae564`; повторная проверка перехода к Фазе 5: `a3569bf` (см. [актуальный вывод](#phase-5-readiness)). Документ фиксирует аудит реализации, а не меняет нормативные контракты. Текущие задачи ведутся в [gate Фазы 4](../implementation/roadmap.md#pre-apply-gate) и [срезах Фазы 5](../implementation/phase-5-automation.md); повторный аудит обновляет этот файл, а не создаёт ещё один параллельный список задач.

## Вывод исходного аудита

DDD migration R1–R4 создала полезные границы: Workspace Configuration отделён от хранения, Gateway — от Host, OS adapters — от ports. Перепроектирование с нуля не требуется. Однако завершение переноса кода не означает готовность durable apply. Сейчас Фаза 4 уже идёт; её следующий безопасный результат — согласованные mutation/identity/ownership contracts и работающий Host coordinator на virtual fixture.

Критический разрыв находится между компонентами. Отдельные store, registry и supervisor проверены лучше, чем их совместное поведение. На исходной версии обнаружены ошибки даже при зелёных существующих tests. До реального cutover необходимо закрыть A–E ниже; это последовательные срезы Фазы 4, а не новая общая фаза «ещё раз всё переписать».

## Метод и доказательства

Проверены публичные Rust ports и их потребители, draft persistence/replay, registry/session/lease transitions, process I/O, desktop composition, новые Platform Input/Interaction contracts, CI и зависимость фаз. Сценарии расширяемости сопоставлены с [исследованием референсов](implementation-review.md): отдельные runtimes, стабильная identity, ограниченные очереди и явные outcomes важнее числа traits. Нового заимствования кода из upstream в этом аудите нет.

Доказательства разделены:

- **Воспроизведено:** новый тест нарушаемого инварианта падает на проверенной версии. Статус исправления указывается отдельно; исправленные случаи дополнительно проходят regression test.
- **Статический дефект:** конкретная последовательность публичных вызовов нарушает contract; отдельный executable probe не запускался.
- **Незавершённая способность:** scaffold ещё не обещает production behavior; перед подключением требуется acceptance gate.

Существующие проверки дали baseline; они не использованы как доказательство отсутствия ошибок. Linux, реальное hardware, отключение питания и опубликованный GitHub CI этим локальным аудитом не проверялись.

После исправлений исходного аудита прошли шесть новых regression tests и затронутые suites Host/Workspace/Profiles/Registry/Supervisor/Platform Host, Clippy по всему core workspace, format, architecture boundaries и проверка links/anchors/JSON во всех 69 Markdown-документах. Это историческое evidence, не повторный запуск на `a3569bf`. Точные команды package checks соответствуют [development](../implementation/development.md#проверки). Открытые пункты A–E ниже не выдаются за исправленные этими проверками.

## Исправлено в ходе аудита

| Проблема исходной версии | Изменение | Новая проверка |
|---|---|---|
| Undo/redo использовал текущую версию из store и заменял документ snapshot предыдущего редактора | Editor хранит собственную подтверждённую версию; внешняя запись вызывает conflict. Принятая новая версия начинает новую history | `undo_and_redo_reject_changes_from_another_editor`, `accepting_a_new_version_starts_a_new_undo_history` |
| Gateway после mutation отдельно читал latest draft: replay receipt v2 возвращал document v3 | Repository возвращает snapshot, сохранённый с receipt; replay читает ту же transaction | `replay_returns_the_document_committed_with_its_receipt`, в том числе после открытия store заново |
| Registry принимал другое содержимое при прежней descriptor revision и повышал registry revision при идентичном retry | Одинаковый snapshot — no-op, отличающийся требует новую revision; пустой initial snapshot учитывается явно | `snapshot_revision_cannot_be_reused_for_different_descriptors`, `empty_snapshot_still_consumes_its_revision` |
| Delta принималась без начального snapshot | Возвращается `ResyncRequired` | `delta_requires_an_initial_snapshot` |
| Core CI выбирал только `default-members`, пропуская integration tests profiles/registry/supervisor/OpenAction | Test и Clippy выбирают workspace с исключением desktop; desktop имеет отдельную job | Проверена конфигурация CI и соответствующие package targets |
| Windows adapter передавал неподдерживаемый `REPLACEFILE_WRITE_THROUGH` | Убран флаг; прежний `sync_all` данных сохранён | Сверка с [Microsoft ReplaceFileW](https://learn.microsoft.com/en-us/windows/win32/api/winbase/nf-winbase-replacefilew); это не доказательство power-loss durability |

Первые пять именованных regression tests были воспроизведены как failing до исправлений; проверка initial delta добавлена вместе с исправлением. Файлы с превышением size ratchet не расширены: editor и descriptor validation выделены в отдельные модули. Внутренние Rust `StoredDraftMutation`/`MutationOutcome` теперь содержат соответствующий draft snapshot и больше не реализуют `Eq`; persisted JSON и wire schema не изменены. API replay возвращает исторический snapshot операции; latest draft клиент получает отдельным query.

## A — Identity и ownership до сохранения bindings

**A1 и весь endpoint follow-up реализованы.** Общий domain `StableEndpointRef` содержит package/node/provider, discriminated device/service target и element/capability/port path, но не runtime placement. Workspace `EndpointBinding` сохраняет только этот тип. Registry разрешает его в единый `RuntimeEndpointRef` с runtime group, plugin session/generation и descriptor revision; тот же тип используют нормализованный signal ingress и command route.

Raw protocol `Signal` несёт полный `EndpointPath`; одиночный deprecated `source_port_id` сохранён на wire для декодирования старого сообщения, но ingress не угадывает по нему endpoint и возвращает `MissingSource`. Fixtures покрывают два одинаковых устройства, service с теми же локальными IDs, перенос descriptor между runtime groups, отсутствие transient полей в persisted binding и совпадение wire/registry/route identity. Отдельная migration старого persisted binding не нужна: до этого среза versioned binding document не публиковался; будущая миграция обязана создавать полный stable path либо оставить binding unavailable, а не подбирать первый локальный PortId.

**A2 реализован.** Resource arbiter выделен из registry aggregate. `ResourceLeaseRecord` хранит discriminated owner: полную runtime `SessionIdentity` либо prepare/probe operation с `OperationId` и monotonic deadline. Session replacement/invalidation, operation cancellation и expiry освобождают только точного owner; stale cleanup не может снять lease новой generation. Fixtures подтверждают, что падение группы A освобождает её ресурс, но не ресурс живой группы B. До commit ownership подготовленного ресурса по-прежнему не означает разрешение на production output.

**A3 реализован.** Registry больше не публикует session отдельно от первого descriptor snapshot: `register_initial_snapshot` сначала полностью валидирует candidate и резервирует следующую registry revision, затем одним изменением заменяет session/topology и освобождает leases прежней generation. `RuntimeController::activate_descriptors` использует только этот путь. Duplicate/invalid initial snapshot не оставляет пустую session, а неуспешная замена сохраняет прежние topology и leases; оба случая закреплены regression tests.

## B — Общий mutation ledger и ограниченное хранение

**B1 и F5-R1 реализованы.** `ApplicationService::open_mutation_window` после проверки scope выдаёт и сохраняет одно активное окно на principal; неизвестный, заменённый или истёкший epoch возвращает `idempotency_expired` даже для replay. Gateway публикует общий command claim по `(principal, epoch, key)` до draft/apply dispatch; command kind, target и preconditions входят в fingerprint. Draft receipt и apply operation остаются отдельными результатами, но не отдельными key namespaces. Оба порядка `PatchDraft`/`ApplyDraft` и restart покрыты regression tests.

Document commit использует recoverable последовательность immutable transaction → pending ledger record → draft head → immutable receipt. При restart bounded recovery допубликовывает candidate/receipt либо останавливается на диагностируемом divergence. Fault tests покрывают ошибку до pending, между pending/head и между head/receipt; отдельные tests — global collision, expiry, restart, authorization revoke и receipt quota. Pending и receipts ограничены конфигурируемыми quotas; прежний линейный поиск receipt по draft chain удалён.

**B2 остаётся до editor Фазы 6.** Append-only transaction snapshots больше не участвуют в обычном lookup ключа; receipt quota ограничивает историю. Однако весь write path не стал O(1): `begin_mutation` вызывает `directory_entry_count`, который перечисляет каталог receipts и собирает entries в `Vec`. Серия новых mutations сохраняет квадратичную суммарную стоимость quota accounting до достижения лимита. Checkpoint/compaction, эффективный счётчик/index и crash-safe GC ещё не реализованы. При compaction должны сохраняться pending operations, действующие dedup windows, receipts, revisions/snapshots/assets; crash в середине GC не делает опубликованное состояние нечитаемым. Benchmark достаточно провести на небольшой конфигурируемой границе и серии растущих историй, без дорогого hardware soak.

## C — Process I/O и единый runtime owner

**C1 реализован.** Блокирующие stdin/stdout/stderr изолированы в process transport workers. Caller использует bounded `try_send`: reliable control переполнение немедленно faults и завершает process tree, а keyed latest-value lane заменяет ещё не отправленное значение в bounded map. Writer failure также fences runtime. Wait/terminate имеют общий deadline; parent exit завершает process tree и не делает безусловный join потомков, удерживающих inherited pipes.

Acceptance включает child, который не читает stdin, coalescing под backpressure и parent, выходящий с живым наследником pipes; последний тест ограничен внешним watchdog.

**C2 реализован.** `ProductionRuntimeCoordinator` связывает `RuntimeController`, `ProcessRuntime`, monotonic clock, package index, grant revision и Capability Registry. Host по-прежнему владеет plan/barrier/commit, Plugin Runtime — session/process и bounded monitor. Desktop composition сканирует `packages`, восстанавливает active commit до публикации Gateway и явно завершает runtime при Exit; permission-free packages используют fail-closed authorization до появления persistent grant adapter.

Сквозной fixture идёт через `ApplicationGateway`: `PlanApply → ApplyDraft → Operation`, реальный child handshake и descriptor publication. Replay возвращает один OperationId, shutdown очищает registry, новый composition восстанавливает committed revision без входного event, а grant revoke после удаления Gateway сначала убирает routes и затем завершает child. C1 отдельно сохраняет overflow/unread-stdin/inherited-pipes watchdog evidence; out-of-process kill всей composition вынесен в E2.

Coordinator получает ports, а не imports чужой infrastructure. Plugin Runtime владеет process session; Host владеет порядком activation/barrier; Gateway владеет авторизацией и представлением operation. Нужен один определённый owner transitions, а не несколько независимых polling loops, меняющих одну session.

## D — Candidate и apply без циклической зависимости фаз

**D1 реализован.** Workspace выпускает immutable `WorkspaceCandidate`, хеш которого связывает полный draft, package lock и asset set. Duplicate PackageId/AssetId отклоняются structural validation, а Profile Store не выдаёт candidate, пока каждый referenced asset не прочитан и его checksum/media type/size не совпадают. Compiler больше не должен строить plan из урезанного `DraftView`.

**D2 реализован.** Новый Automation context публикует `PlanCompiler` и immutable `CompiledExecutionPlan`; временный `RestrictedFixtureCompiler` принимает только empty fixture либо точный `virtual_echo` с одним pinned virtual package. Неподдерживаемый executable graph возвращает `UnsupportedGraph`, а не успешный пустой plan. В Фазе 5 тот же acceptance выполняется с настоящим compiler и isolated virtual runtime.

Host `ApplyPlanningService` связывает plan hash с HostBootId, draft version, candidate/package/assets hashes, compiler output, registry/grant revisions, expected active selection и monotonic expiry. Prepare повторяет проверки до вызова `RuntimeActivation`; barrier проверяет их ещё раз и сверяет prepared owner/plan. Fixtures подтверждают conflict при topology/candidate/package change до prepare, grant change после prepare, expiry и несовпадающем active selection. Seam D используется durable coordinator и реальной C2 activation; произвольный graph остаётся задачей Фазы 5.

[Workspace validation](../../crates/deckweave-workspace/src/validation.rs) проверяет форму документа, hashes, строки и уникальность PackageId/AssetId; Profile adapter отдельно подтверждает существование и metadata asset bytes. Это всё ещё structural validation, а не разрешение activate. Automation отвечает за semantic graph/schema compatibility, Registry — за разрешение endpoints, security port — за grants. Gateway не должен дублировать эти инварианты.

## E — Commit/recovery и границы durability

**P1 перед durable apply, статический риск.** [NativeAtomicFileStore](../../crates/deckweave-platform-host/src/storage.rs) представляет все replace failures как обычную ошибку и удаляет temporary file. Но [ReplaceFileW](https://learn.microsoft.com/en-us/windows/win32/api/winbase/nf-winbase-replacefilew) описывает случаи, когда имена файлов уже изменились при возвращённой ошибке. Нельзя на основании одного `Err` гарантировать «публикации не было» или безусловно очищать единственную recovery copy.

Нужны классификация publication outcome, reconciliation с диском и fault fixtures. Удаление неподдерживаемого флага в этом аудите не решает весь recovery protocol. Уровень гарантии задаётся отдельно для process kill и power loss, с filesystem assumptions. Checksum обнаруживает повреждение, но не возвращает потерянные bytes.

**E0 реализован.** `AtomicFileStore` теперь возвращает классифицированный `PublicationError`: `NotPublished` допускает безопасный retry, `Unknown` требует восстановления по durable document. Native adapter после ошибки replace перечитывает target и сравнивает его с ожидаемыми bytes: совпадение превращает потерянный ответ в успех, старый/отсутствующий target — в `NotPublished`, ошибка чтения — в `Unknown` без удаления staged file. Fault fixtures воспроизводят ошибку до replace, ошибку после фактической смены target и невозможность reconciliation. Это доказывает process-level decision path, но не обещает power-loss durability изменения directory metadata.

**E1 реализован.** Workspace contract задаёт immutable `ApplyOperationIntent`, `ApplyCommitRecord`, operation status и `ApplyOperationRepository`; Profile adapter хранит intent/status раздельно и меняет выбранную revision только единым `active/commit.json`. Host `ApplyCoordinator` сериализует plan/prepare/barrier/commit/activate, после ошибки публикации перечитывает active commit и не открывает старый dispatcher наугад. Application Gateway предоставляет scoped `PlanApply`, `ApplyDraft` и `Operation`; повтор key возвращает тот же durable `OperationId`. Restart fixtures подтверждают старый выбор до commit, новый после commit, startup reactivation без replay входного события, reconciliation потерянного ответа, потерю status write после commit и grant change на barrier. Isolated virtual runtime через production composition входит в acceptance C2.

**E2 реализован на process-crash уровне.** Внешний test process использует тот же production coordinator, настоящий isolated child и публичный Gateway, затем аварийно завершается после candidate, intent, prepared status, barrier, durable commit и runtime activation. Новый process оставляет прежний выбор до commit, а после commit восстанавливает новую revision и runtime без replay входного события. Отдельные сквозные fixtures подтверждают disk-full при сохранении candidate, revoke grant до prepare и на barrier, а также отказ explicit disruptive handoff. `allow_disruption` входит в plan hash, application request, durable intent и commit record; план с downtime без явного consent отклоняется.

После commit новая revision остаётся выбранной даже при degraded activation; до commit выбирается прежняя. Receipt, operation status и selected revision согласуются после потери ответа. Эти fixtures доказывают поведение при завершении процесса и классифицированной ошибке записи, но не имитируют потерю питания, повреждение диска или неподтверждённые semantics directory flush.

## Последующие фазы: недостающие условия входа

| Фаза | Что проверить до следующего слоя |
|---|---|
| 5 | Заменить fake compiler без изменения semantics apply; source → isolated action → state/feedback идёт через Host. Dynamic schema change, два одинаковых устройства, old-epoch result и очередь release входят в M1 |
| 6 | Tauri adapter вызывает Gateway с host-derived principal; shared DTO parity, version conflict/replay и reconnect snapshot проверяются до canvas. UI bridge пока содержит только host commands, это ещё не интеграция draft API |
| 7 | Grants/discovery/availability проверяются при cold start. Observe не обещает interception и не создаёт физическую identity для aggregate input; возврат synthetic input не зацикливает trigger |
| 8–9 | One-owner arbitration одинаков для runtime, preview и probe; reconnect/format change не повторяет старый output. Renderer benchmark и реальные transport budgets предшествуют editor optimization |
| 11 | Minimal job correlation уже существует с Фазы 5; artifact broker, TTL/quota и cancel появляются до связывания transcription/browser/mail outputs. Fake Handy-like chain проверяет typed data, не реальные приватные данные |
| 12 | SDK freeze после conformance на device/service/dynamic ports и version-skew fixtures; публичные SDK не импортируют host infrastructure |
| 13 | Interaction scaffold не валидирует selected value по schema, не version-ирует search queries и хранит terminal sessions без eviction. Нужны query generation, epoch/provider fencing, monotonic deadlines, byte/count budgets и terminal retention до presenter/custom view |
| 13–14 | C0 Counter manifest не доказывает C1 lifecycle. Executable C1 spike должен предшествовать инвестициям в custom web/PI; production C1–C3 остаётся Фазой 14 |
| 16–18 | HTTP/WS adapter проходит parity с in-process Gateway; auth/recovery нельзя откладывать до этой фазы. Hardening проверяет уже реализованные гарантии, а не впервые добавляет ownership/cleanup |

## Оценка DDD и организации документации

Главные decisions — модульный монолит, один per-user Host, process isolation плагинов, типизированные capability families, отдельные data/control planes — сохраняются. Любое устройство не требует универсального callback в ядре: оно объявляет поддерживаемые capabilities, lifecycle и QoS; неизвестная family остаётся недоступной до явной поддержки host. Media RT, firmware programming и synchronous input interception не протаскиваются через обычный action queue.

Cargo dependency allowlist и size ratchet полезны, но не доказывают DDD автоматически: они не проверяют инварианты aggregate и не запрещают всякий `std::fs` внутри domain. При ревью нового use case нужны владелец инварианта, port contract и межконтекстный failure test. Перенос файлов без изменения ответственности не закрывает такой review.

Текущая структура `product → contracts → architecture/modules → implementation` уже пригодна для расширения. Повторное перемещение всех файлов не улучшит её. Внесены точечные изменения source-of-truth contracts, исправлены преувеличенные статусы, выделены проверяемые pre-apply slices. После закрытия gate этот отчёт остаётся evidence либо переносится в archive с обновлением ссылок; текущие задачи не дублируются в новых файлах.

<a id="phase-5-readiness"></a>
## Проверка перехода к Фазе 5 — `a3569bf`

**Срез 5.0 после этого аудита реализован.** Изменения закрыли три перехода между ledger, commit history и startup recovery постоянными regression tests. Дополнительно runtime/recovery переведены с собственного разбора fixture на checksum-версию compiled runtime requirements, а `InvocationRequest` получил полный `EndpointPath` и negotiated feature ID. Pure graph/compiler/scheduler остаются следующей работой; возвращаться к проектированию с нуля не нужно.

### Воспроизведённые дефекты — исправлены в срезе 5.0

| ID / приоритет | Входная последовательность и результат | Причина и требуемая граница исправления |
|---|---|---|
| F5-R1 / P1 | `ApplyDraft`/`PatchDraft` и обратный порядок с одним principal/epoch/key, включая restart | Общий durable `CommandClaimRepository` резервирует fingerprint команды до dispatch. Локальные draft receipt и apply operation остаются результатами своих use cases, но больше не определяют общий key scope |
| F5-R2 / P1 | Commit A без terminal status, затем завершённый B и startup recovery | Перед сменой head store запечатывает A как committed. Recovery завершает его отдельным `Superseded/Inactive`; прямой predecessor старого формата распознаётся по `previous_active` |
| F5-R3 / P1 | 300 terminal operations при `recoverable_operations(256)` | Directory iterator хранит в памяти только non-terminal snapshots; capacity применяется к pending set. B2 compaction/index остаётся для ограничения latency и размера истории, но terminal history больше не блокирует запуск |

Проверки выполнены отдельными временными integration targets, переиспользующими fixtures `durable_apply.rs` и `apply_operations.rs`. Запускались только новые `review_probe_*`: две Host-проверки и одна Profiles-проверка скомпилировались и упали на указанных assertions. Существующие suites повторно не запускались. Временные targets убраны из обычной test discovery; локальные исходники сохранены в ignored `test-results/phase5-readiness-a3569bf/`. Для постоянных regression tests в 5.0 добавить оба порядка R1 и restart, superseded committed history R2, более 256 terminal operations вместе с pending/corrupt records для R3. Повышение startup limit и безусловное игнорирование ошибок эти инварианты не исправляют.

### Что ещё ограничивает реализацию Фазы 5

Это статические наблюдения и отсутствующие способности, а не дополнительные выполненные crash tests:

- [InvocationRequest](../../protocol/deckweave/v1/control.proto) теперь содержит полный `EndpointPath`; Rust/TS fixtures и feature `org.deckweave.invocation.endpoint_path` фиксируют совместимость. Реальный dispatcher и отрицательная проверка session без negotiated feature появятся вместе с live invocation.
- [CompiledExecutionPlan](../../crates/deckweave-automation/src/plan.rs) больше не выбирает enum fixture: он содержит точные runtime requirements и сохраняется с checksum в apply intent/commit. Executable graph IR, descriptor/schema catalog и runtime binding ещё не реализованы.
- [Runtime coordinator](../../crates/deckweave-plugin-supervisor/src/host/coordinator.rs) одинаково использует сохранённые requirements при prepare/recovery и не разбирает JSON fixture. Ограничение одного runtime requirement пока остаётся явно диагностируемым; multi-group activation относится к 5.4.
- [Instance monitor](../../crates/deckweave-plugin-supervisor/src/host/instance.rs) потребляет envelope, но ещё не передаёт signal/outcome body в scheduler. Apply barrier пока не проверен живым effect dispatcher. Самих callback/boolean flags недостаточно: epoch, grants, route generation и cleanup owner должны проверяться на реальном dispatch.
- Исторический `Succeeded/Applied` operation receipt не заменяет текущее runtime health после process exit/revoke. Live health projection и события lifecycle нужны вместе с integration, а не после появления UI.

Эти границы покрывает [план 5.0–5.6](../implementation/phase-5-automation.md). B2 compaction остаётся обязательной до editor Фазы 6 и не блокирует pure engine. Полная поддержка Handy, браузера, Telegram, lighting/layers и media не переносится в Фазу 5: здесь доказываются общий typed result path, адресуемость и смена плана, на которых следующие фазы смогут строиться.
