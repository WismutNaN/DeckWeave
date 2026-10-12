# Проверка готовности к apply после DDD migration

Дата: 2026-10-12. Проверенная исходная версия: `2bae564`. Документ фиксирует аудит реализации, а не меняет нормативные контракты. Текущие задачи ведутся в [gate Фазы 4](../implementation/roadmap.md#pre-apply-gate); повторный аудит обновляет этот файл, а не создаёт ещё один параллельный список задач.

## Вывод

DDD migration R1–R4 создала полезные границы: Workspace Configuration отделён от хранения, Gateway — от Host, OS adapters — от ports. Перепроектирование с нуля не требуется. Однако завершение переноса кода не означает готовность durable apply. Сейчас Фаза 4 уже идёт; её следующий безопасный результат — согласованные mutation/identity/ownership contracts и работающий Host coordinator на virtual fixture.

Критический разрыв находится между компонентами. Отдельные store, registry и supervisor проверены лучше, чем их совместное поведение. На исходной версии обнаружены ошибки даже при зелёных существующих tests. До реального cutover необходимо закрыть A–E ниже; это последовательные срезы Фазы 4, а не новая общая фаза «ещё раз всё переписать».

## Метод и доказательства

Проверены публичные Rust ports и их потребители, draft persistence/replay, registry/session/lease transitions, process I/O, desktop composition, новые Platform Input/Interaction contracts, CI и зависимость фаз. Сценарии расширяемости сопоставлены с [исследованием референсов](implementation-review.md): отдельные runtimes, стабильная identity, ограниченные очереди и явные outcomes важнее числа traits. Нового заимствования кода из upstream в этом аудите нет.

Доказательства разделены:

- **Воспроизведено:** новый тест сначала падает на исходном коде и проходит после исправления.
- **Статический дефект:** конкретная последовательность публичных вызовов нарушает contract; отдельный executable probe не запускался.
- **Незавершённая способность:** scaffold ещё не обещает production behavior; перед подключением требуется acceptance gate.

Существующие проверки дали baseline; они не использованы как доказательство отсутствия ошибок. Linux, реальное hardware, отключение питания и опубликованный GitHub CI этим локальным аудитом не проверялись.

После изменений прошли шесть новых regression tests и затронутые suites Host/Workspace/Profiles/Registry/Supervisor/Platform Host, Clippy по всему core workspace, format, architecture boundaries и проверка links/anchors/JSON во всех 69 Markdown-документах. Точные команды package checks соответствуют [development](../implementation/development.md#проверки). Открытые пункты A–E ниже не выдаются за исправленные этими проверками.

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

**A1 реализован.** Общий domain `StableEndpointRef` теперь содержит package/node/provider, discriminated device/service target и element/capability/port path, но не runtime group. Registry разрешает именно полный путь, допускает повтор локальных IDs в разных scopes и не перепривязывает соседний экземпляр. Raw protocol `Signal` получил полный `EndpointPath`; одиночный `source_port_id` помечен deprecated. Fixtures покрывают два одинаковых устройства, service с теми же локальными IDs и перенос descriptor между runtime groups.

Весь срез A этим не закрыт: нормализованный signal ingress, command routes и persisted bindings должны использовать тот же shared type, а затем пройти migration/compatibility fixture. Runtime resolution продолжает добавлять session/generation/revision; эти transient поля запрещено сохранять как identity.

**A2 реализован.** Resource arbiter выделен из registry aggregate. `ResourceLeaseRecord` хранит discriminated owner: полную runtime `SessionIdentity` либо prepare/probe operation с `OperationId` и monotonic deadline. Session replacement/invalidation, operation cancellation и expiry освобождают только точного owner; stale cleanup не может снять lease новой generation. Fixtures подтверждают, что падение группы A освобождает её ресурс, но не ресурс живой группы B. До commit ownership подготовленного ресурса по-прежнему не означает разрешение на production output.

**A3 реализован.** Registry больше не публикует session отдельно от первого descriptor snapshot: `register_initial_snapshot` сначала полностью валидирует candidate и резервирует следующую registry revision, затем одним изменением заменяет session/topology и освобождает leases прежней generation. `RuntimeController::activate_descriptors` использует только этот путь. Duplicate/invalid initial snapshot не оставляет пустую session, а неуспешная замена сохраняет прежние topology и leases; оба случая закреплены regression tests.

## B — Общий mutation ledger и ограниченное хранение

**B1 реализован.** `ApplicationService::open_mutation_window` после проверки scope выдаёт и сохраняет одно активное окно на principal; неизвестный, заменённый или истёкший epoch возвращает `idempotency_expired` даже для replay. Receipt адресуется O(1) по хешу глобального `(principal, epoch, key)`, а target/expected version/command входят в fingerprint. Поэтому один key на двух drafts даёт conflict до version check и после restart.

Document commit использует recoverable последовательность immutable transaction → pending ledger record → draft head → immutable receipt. При restart bounded recovery допубликовывает candidate/receipt либо останавливается на диагностируемом divergence. Fault tests покрывают ошибку до pending, между pending/head и между head/receipt; отдельные tests — global collision, expiry, restart, authorization revoke и receipt quota. Pending и receipts ограничены конфигурируемыми quotas; прежний линейный поиск receipt по draft chain удалён.

**B2 остаётся до editor Фазы 6.** Append-only transaction snapshots теперь не участвуют в обычном lookup ключа и ограничены общей receipt quota, поэтому больше не создают квадратичный поиск или скрытый `HistoryLimit` для каждого нового command. Однако checkpoint/compaction и crash-safe GC ещё не реализованы. При compaction должны сохраняться pending operations, действующие dedup windows, receipts, revisions/snapshots/assets; crash в середине GC не делает опубликованное состояние нечитаемым. Benchmark достаточно провести на небольшой конфигурируемой границе и серии растущих историй, без дорогого hardware soak.

## C — Process I/O и единый runtime owner

**C1 реализован.** Блокирующие stdin/stdout/stderr изолированы в process transport workers. Caller использует bounded `try_send`: reliable control переполнение немедленно faults и завершает process tree, а keyed latest-value lane заменяет ещё не отправленное значение в bounded map. Writer failure также fences runtime. Wait/terminate имеют общий deadline; parent exit завершает process tree и не делает безусловный join потомков, удерживающих inherited pipes.

Acceptance включает child, который не читает stdin, coalescing под backpressure и parent, выходящий с живым наследником pipes; последний тест ограничен внешним watchdog. Это закрывает transport defect, но не C2: production Host coordinator всё ещё должен владеть polling, grant/revoke и shutdown transitions.

**Незавершённая способность.** [HostRuntime](../../crates/deckweave-host/src/lib.rs) сейчас владеет locks/status/autostart. Desktop создаёт Host/Gateway/Store, но не production driver, связывающий `RuntimeController`, `ProcessRuntime`, clock, registry и grant events. Integration tests вручную соединяют эти части. Следующий срез должен запустить virtual provider через public application path и пройти revoke/crash/shutdown при закрытом UI.

Coordinator получает ports, а не imports чужой infrastructure. Plugin Runtime владеет process session; Host владеет порядком activation/barrier; Gateway владеет авторизацией и представлением operation. Нужен один определённый owner transitions, а не несколько независимых polling loops, меняющих одну session.

## D — Candidate и apply без циклической зависимости фаз

**D1 реализован.** Workspace выпускает immutable `WorkspaceCandidate`, хеш которого связывает полный draft, package lock и asset set. Duplicate PackageId/AssetId отклоняются structural validation, а Profile Store не выдаёт candidate, пока каждый referenced asset не прочитан и его checksum/media type/size не совпадают. Compiler больше не должен строить plan из урезанного `DraftView`.

**D2 реализован.** Новый Automation context публикует `PlanCompiler` и immutable `CompiledExecutionPlan`; временный `RestrictedFixtureCompiler` принимает только empty fixture либо точный `virtual_echo` с одним pinned virtual package. Неподдерживаемый executable graph возвращает `UnsupportedGraph`, а не успешный пустой plan. В Фазе 5 тот же acceptance выполняется с настоящим compiler и isolated virtual runtime.

Host `ApplyPlanningService` связывает plan hash с HostBootId, draft version, candidate/package/assets hashes, compiler output, registry/grant revisions, expected active selection и monotonic expiry. Prepare повторяет проверки до вызова `RuntimeActivation`; barrier проверяет их ещё раз и сверяет prepared owner/plan. Fixtures подтверждают conflict при topology/candidate/package change до prepare, grant change после prepare, expiry и несовпадающем active selection. Это закрывает seam D, но не durable commit: operation intent/status/recovery и реальная activation принадлежат C2/E1.

[Workspace validation](../../crates/deckweave-workspace/src/validation.rs) проверяет форму документа, hashes, строки и уникальность PackageId/AssetId; Profile adapter отдельно подтверждает существование и metadata asset bytes. Это всё ещё structural validation, а не разрешение activate. Automation отвечает за semantic graph/schema compatibility, Registry — за разрешение endpoints, security port — за grants. Gateway не должен дублировать эти инварианты.

## E — Commit/recovery и границы durability

**P1 перед durable apply, статический риск.** [NativeAtomicFileStore](../../crates/deckweave-platform-host/src/storage.rs) представляет все replace failures как обычную ошибку и удаляет temporary file. Но [ReplaceFileW](https://learn.microsoft.com/en-us/windows/win32/api/winbase/nf-winbase-replacefilew) описывает случаи, когда имена файлов уже изменились при возвращённой ошибке. Нельзя на основании одного `Err` гарантировать «публикации не было» или безусловно очищать единственную recovery copy.

Нужны классификация publication outcome, reconciliation с диском и fault fixtures. Удаление неподдерживаемого флага в этом аудите не решает весь recovery protocol. Уровень гарантии задаётся отдельно для process kill и power loss, с filesystem assumptions. Checksum обнаруживает повреждение, но не возвращает потерянные bytes.

После commit новая revision остаётся выбранной даже при degraded activation; до commit выбирается прежняя. Receipt, operation status и selected revision должны согласовываться после потери ответа. Disk full, grant revoke на prepare, exclusive handoff failure и настоящий kill процесса на каждой стадии проверяются через тот же composition, который используется приложением. Fake I/O error проверяет только свой участок, а не весь путь восстановления.

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
