# План реализации DeckWeave

План рассчитан на одного разработчика или одного coding agent. Фазы ниже — последовательные результаты, а не обещание завершить весь L-объём за 1–5 дней. Перед началом L-фаза разбивается на рабочие срезы до 1–5 дней с самостоятельной демонстрацией; размер уточняется по результатам spike и предыдущего среза. Незавершённый backend/контракт нельзя скрывать в оценке следующей фазы.

## Обязательные зависимости и срезы

Нормативные failure/ordering contracts: [Lifecycle](../contracts/lifecycle.md). При расхождении module summary с контрактом сначала исправляется документация. Реализованные foundations имеют tests, но общий apply/recovery protocol ещё не подтверждён; [аудит после DDD migration](../research/pre-apply-review.md) отделяет дефекты от незавершённых способностей.

Расширяемость проверяется до SDK freeze на [сценариях](../product/scenarios.md): независимые keyboard/analog/RGB/surface/job/media descriptors, typed results, geometry и dynamic ports. [Исследование реализаций](../research/implementation-review.md) определяет reuse-кандидатов и ограничения evidence; существующий upstream метод не закрывает hardware acceptance.

| Фаза | Последовательные срезы и gate |
|---|---|
| 3 | Static index → handshake/virtual provider → leases/discovery/restart; отдельно ранний C0/C1 compatibility spike на pinned open fixture |
| 4 | После draft/store: A identity/owners → B ledger/retention → C runtime coordinator → D candidate/apply ports → E commit/recovery; [gate](#pre-apply-gate) до hardware |
| 5 | Compiler → scheduler/ordering → async outcomes/epochs/replay; performance baseline |
| 6 | Canvas/select → schema-form binding → draft/apply/health; готовый preset укладывается в click budget |
| 8 | Protocol/identity fixtures → inputs → frames/light/haptic; неподтверждённый hardware не считается реализованным |
| 9 | Backend benchmark/ADR → display renderer/frame protocol → basic LightFrame/ownership → hit testing/preview → editor; diff loss fixture обязателен |
| 11 | Отдельные срезы по 1–5 дней: layers/cleanup → typed sequence/parallel/results → jobs/artifact broker → collections/notification fixture → lighting envelopes → schedules/Graph UI; один срез не обещает все nodes |
| 12 | Rust/TS SDK release → Python runtime/SDK → conformance/package-lock update → catalogue metadata |
| 13 | Executable C1 spike → interaction lifecycle/limits → browser isolation/offscreen rendering spike → settings/surface widget → schema presenter → custom view; неудачный spike требует пересмотра backend/сроков |
| 14 | C0/C1 production host → C2 PI/settings → C3 dial/feedback → replacement migration; каждый уровень имеет отдельный report |
| 15 | Probe/inspection → calibration/mapping → capture/conformance; production service не зависит от testkit |
| 17 | Portable packaging → Linux virtual/CLI → Linux UI/platform smoke; core Linux CI настроен с Фазы 2, результат remote run фиксируется отдельно |
| 18 | Recovery/security matrix → performance/soak → release/docs; новые features не добавляются |

Каждый срез получает конкретную fixture, rollback/disable path и рабочую промежуточную сборку. Это не параллельные задачи с неявными зависимостями.

## Легенда статусов

- [ ] Не начата
- [~] В процессе
- [x] Готово

## Обзор фаз

| Фаза | Название | Результат | Трудоёмкость | Статус |
|---:|---|---|:---:|:---:|
| 1 | Скелет репозитория | Workspace компилируется, UI shell и CI запускаются | S | [~] |
| 2 | Domain и protocol contracts | Versioned types/schemas/generated code | M | [x] |
| 3 | Plugin supervisor и virtual package | Foundations готовы; Host integration ещё не закрыта | L | [~] |
| 4 | Profile revisions и application API | Draft/validate/apply/rollback через API | L | [~] |
| 5 | Минимальный automation engine | Source → action → feedback deterministic path | L | [ ] |
| 6 | Desktop workspace canvas | Устройства и bindings редактируются в UI | L | [ ] |
| 7 | Windows base provider и Platform Input | Volume/media/app/active-window и обычная клавиатура работают | M | [ ] |
| 8 | N4 Pro hardware provider | Keys/encoders/touch/light/haptic доступны | L | [ ] |
| 9 | Surface compositor | Полный экран N4 Pro без grid gaps | L | [ ] |
| 10 | Gamepad и test lab | Второй класс hardware + replay/metrics | M | [ ] |
| 11 | Layers и сложные automation | Conditions, sequence/parallel, errors, timers | L | [ ] |
| 12 | Plugin SDK и packaging | Rust/TypeScript/Python SDK, conformance, install/update | L | [ ] |
| 13 | Widget и Interaction runtime | Custom settings/widgets и host-owned interactions изолированы | L | [ ] |
| 14 | OpenAction compatibility | C0–C3 reference packages работают | L | [ ] |
| 15 | Device Studio | Standard discovery/calibration/capture | L | [ ] |
| 16 | Codex, CLI и optional MCP | Полный управляемый API workflow | M | [ ] |
| 17 | Portable build и Linux foundation | Separate data root и первый Linux adapter | L | [ ] |
| 18 | Hardening v1 | Ошибки, recovery, performance, docs, release | L | [ ] |

## Milestones

- **M1 — Architecture proof:** после Фазы 5 virtual input вызывает isolated action и feedback.
- **M2a — N4 Pro preview:** после Фазы 9 panel управляет Windows и выводит full-screen scene; сложные P0 automation ещё не закрыты.
- **M2 — Daily N4 Pro:** после Фазы 11 закрыты все P0, включая layers, sequence/parallel/error policies, recording, export и portable data-root policy.
- **M3 — Extensible beta:** после Фазы 14 доступны SDK и существующие OpenAction plugins.
- **M4 — v1 foundation:** после Фазы 18 готов публичный Windows release и Linux foundation.

<a id="ddd-migration"></a>
## DDD migration перед продолжением Фазы 4

**Исходное состояние до миграции:** profile storage, draft ApplicationService и host lifecycle работали, но крупные модули смешивали aggregate/application/infrastructure, а `deckweave-host` содержал dispatch, authorization и error translation. R1–R4 ниже фиксируют выполненное разделение.

**Целевое состояние:** bounded contexts и dependency rules из [DDD](../architecture/ddd.md); Workspace Configuration отделён от Profile Store adapter, Host Lifecycle — от Application Gateway, Platform Input и Interaction UI имеют собственные ports/contracts.

**Стратегия:** strangler migration без big-bang rewrite. Каждый срез — отдельный commit; публичные re-export/adapters временно сохраняют существующих потребителей. Новый use case пишется уже в целевой границе. Существующий превышающий size ratchet файл не растёт.

### R1 — Зафиксировать baseline и architecture gates

- [x] Добавить/актуализировать characterization tests текущих draft, receipts, authorization и host snapshot paths до переноса кода.
- [x] Расширить `check-boundaries` правилами context dependencies, thin crate roots и size ratchet; исключения содержат owner/reason/review milestone (→ DW-NF-015).
- [x] Сохранить зелёными `npm run check`, full Rust workspace tests и fault tests; baseline не меняет runtime behavior.

### R2 — Выделить Workspace Configuration

- [x] Создать `deckweave-workspace`, перенести `WorkspaceDraft`/`WorkspaceRevision`, commands, validation, pure JSON mutation и `DraftRepository` port (→ [Workspace Configuration](../architecture/modules/workspace-configuration.md)).
- [x] Перенести оставшуюся revision/snapshot/export policy; оставить append-only/filesystem/recovery в Profile Store infrastructure adapter (→ [Profile Store](../architecture/modules/profile-store.md)).
- [x] Разделить tests первого среза: pure domain mutation в Workspace, application characterization через port, storage/round-trip/fault tests в Profiles.
- [x] Сохранить временные re-export для прежних imports; удалить их отдельным commit после перевода всех consumers (→ [DDD migration](#ddd-migration)).

### R3 — Разделить Host Lifecycle и Application Gateway

- [x] Вынести command/query dispatch, principal scopes и stable error mapping из `deckweave-host`; operation tracking добавляется уже в эту границу первым срезом Фазы 4.
- [x] Оставить Host владельцем startup/apply/active revision/epochs/recovery; platform-host разбить на узкие launch/storage/process adapters без смены публичных ports.
- [x] Проверить запрет прямых imports чужой infrastructure и parity in-process client.

### R4 — Подготовить новые расширения

- [x] Добавить protocol/domain fixtures Platform Input: bundled aggregate keyboard, observe/pass-through, честная granularity, separate intercept/emit.
- [x] Добавить InteractionSession ports и fake presenter/provider: search pages, selection/cancel/timeout, concurrent isolation.
- [x] Не подключать реальный OS hook, Zotero или custom web runtime до соответствующих Фаз 7/13.

**Rollback:** до удаления compatibility re-export каждый срез откатывается одним commit. Изменение persisted document/wire schema в R1–R4 запрещено без отдельной migration/ADR. Если gate выявляет незафиксированную семантику, перенос останавливается на текущей зелёной версии, добавляется characterization test, затем срез повторяется.

**Gate структурной миграции:** [x] R1–R4 завершены. Это разрешает работу в новых границах, но не подтверждает готовность apply. Operation tracking/apply реализуются через Application Gateway и Host Lifecycle ports после условий ниже.

<a id="pre-apply-gate"></a>
## Gate перед durable apply

Основание: [аудит реализации после DDD migration](../research/pre-apply-review.md). Фаза 4 уже начата. Следующие срезы закрывают недостающие контракты до irreversible cutover; каждый планируется отдельно на 1–5 дней, с дополнительным делением при превышении оценки.

- [x] Добавить regression tests и исправить concurrent editor undo/redo, согласованность receipt/document при replay, snapshot revision/initial delta; включить integration packages в core CI.

| Срез | Владелец и результат | Обязательное доказательство выхода |
|---|---|---|
| A — Identity/ownership | Registry + Plugin Runtime: полный device/service endpoint path, owner-scoped resource leases и атомарная initial registration | Два одинаковых устройства с одинаковыми локальными PortId; crash/revoke группы A освобождает только её leases; invalid initial snapshot не оставляет session и не разрушает прежнюю |
| B — Mutation ledger | Gateway + Workspace ports + Store adapter: общий scope ключей, persisted issued epochs и atomic receipt snapshot; bounded index/quota входят в apply gate, полная compaction — до editor Фазы 6 | Один key на двух draft даёт conflict; restart/expired epoch/revoke; quota не превращается в неограниченную историю; будущий checkpoint не теряет receipts |
| C — Runtime coordinator | Plugin Runtime сначала получает bounded/nonblocking transport, затем Host связывает clocks/grants/process/registry одним owner | Unread stdin и parent exit с унаследованными pipes ограничены watchdog; public application path запускает virtual package и проходит overflow/revoke/shutdown при закрытом UI |
| D — Candidate/apply seam | Workspace проверяет lock/assets, Automation port компилирует, Host сериализует preconditions/barrier | Plan token связан с draft/package/registry/grants/active revision; unsupported graph отклоняется; topology/grant change между plan и prepare конфликтует |
| E — Commit/recovery | Host + Store ports: intent/commit/status и reconciliation неоднозначных OS errors | Kill до/после каждого шага, disk full, lost reply, failed exclusive handoff; старый выбор до commit, новый после commit, без replay effects |

- [ ] A завершён; endpoint contract согласован для descriptors, signal ingress, routes и persisted bindings.
- [ ] B завершён; API dedup contract реализован целиком, а не только в одном draft.
- [ ] C завершён; интеграция использует production composition и внешний watchdog для hang tests.
- [ ] D завершён; Фаза 4 использует ограниченный fake compiler для empty/virtual fixtures. Он не принимает произвольный graph как пустой plan; Фаза 5 заменяет adapter настоящим compiler на том же acceptance.
- [ ] E завершён; process-crash evidence отделено от power-loss/filesystem assumptions.

Порядок малых обратимых срезов:

- [x] A1 — stable endpoint identity и raw signal path: полный device/service path, повторяемые локальные IDs, независимость persisted binding от runtime group.
- [x] A2 — owner-scoped leases и точечный cleanup session/prepare/probe owner.
- [x] A3 — атомарная initial registration без пустой session и без разрушения прежней topology.
- [ ] C1 — bounded/nonblocking process transport и независимый kill path.
- [ ] B1 — global mutation ledger, issued epochs, recovery и bounded quota/index.
- [ ] D — candidate/compiler/activation ports и fenced plan token.
- [ ] C2 — production runtime coordinator через public application path.
- [ ] E0 — publication outcomes и disk reconciliation для неоднозначного replace.
- [ ] E1 — durable apply/operation tracking и recovery acceptance.
- [ ] B2 — checkpoint/compaction до начала editor-heavy Фазы 6; не блокирует первый bounded virtual apply после B1.

Выход gate — demonstration `application command → prepare → commit → virtual activation → restart/recovery`. Перенос реализации в отдельные crates и зелёные unit tests сами по себе этот gate не закрывают. Реальные actions/hardware не подключаются, пока owner/cleanup/recovery не доказаны.

---

<a id="фаза-1--скелет-репозитория"></a>
## Фаза 1 — Скелет репозитория

**Цель:** проект компилируется и запускается без бизнес-логики.
**Результат:** Rust workspace, Tauri/Svelte shell, empty tray host и CI проходят smoke tests.
**Трудоёмкость:** S
**Статус:** [~] Реализация и локальные автопроверки готовы; ожидаются ручная приёмка трея и первый GitHub CI.

### Контекст для агента
> Каркас реализован без hardware, plugin IPC и graph logic. [Разработка и evidence](development.md) описывают команды, проверенный lifecycle и оставшиеся проверки. Не считать будущие contracts runtime-гарантиями.

### Задачи

- [x] Создать только необходимые каталоги из целевой структуры (→ [Архитектура — Целевая структура репозитория](../architecture/overview.md#целевая-структура-репозитория)).
- [x] Создать Rust crates/apps с пустыми публичными boundaries (→ [Модуль: Domain & Protocol](../architecture/modules/domain-protocol.md), [Модуль: Host Runtime](../architecture/modules/host-runtime.md)).
- [x] Создать Tauri/Svelte shell: окно, tray, close-to-tray, явный Exit (→ [Модуль: Desktop Shell](../architecture/modules/desktop-shell.md)).
- [x] Реализовать single-instance/data-root locks, installed/portable root selection и optional autostart; portable ничего не регистрирует сам (→ [Host Platform Services](../architecture/modules/host-platform-services.md)).
- [x] Зафиксировать GPL и permissive package license files/headers (→ [ADR-0007](../decisions/0007-licensing.md)).
- [x] Настроить format/lint/test/license/provenance checks в CI (→ [Модуль: Diagnostics & Test Lab](../architecture/modules/diagnostics-test-lab.md)).
- [x] Включить docs-check: paths/anchors, requirement IDs, PLAN → modules и JSON examples (→ [Обзор документации](../README.md)).

### Тесты

- [x] Юнит: workspace компилируется, Rust unit/integration tests проходят.
- [x] Интеграционный: desktop стартует, окно закрывается, tray host остаётся жив.
- [ ] Ручная проверка: Exit из tray завершает process без orphan children.
- [x] Lifecycle: второй запуск с тем же root открывает UI; другой root не запускает второй host; crash освобождает lock. Hardware в этой фазе отсутствует.

Автоматический smoke дополнительно вызывает общий Exit handler и проверяет завершение host/WebView2. Он не подменяет ручную проверку клика по пункту tray menu. CI настроен для Windows и Linux core, но результаты удалённого GitHub run ещё не получены.

### Ссылки

- [ADR-0001](../decisions/0001-stack-and-product-shape.md)
- [ADR-0011](../decisions/0011-deployment-modes.md)
- [Требования DW-OP-001](../product/requirements.md#api-профили-и-эксплуатация)

---

## Фаза 2 — Domain и protocol contracts

**Цель:** зафиксировать минимальную capability/revision/message model до реализации runtime.
**Результат:** schemas генерируют Rust/TypeScript types; compatibility fixtures подтверждают round-trip.
**Трудоёмкость:** M
**Статус:** [x] Готово

### Контекст для агента
> Реализован минимальный vocabulary плюс extension mechanism; remote/media имеют только descriptors, без transport/runtime и buffer ABI. Следующая фаза может опираться на зафиксированные fixtures и generated types.

### Задачи

- [x] Реализовать IDs, descriptors, port kinds, envelopes, outcomes и errors (→ [Модуль: Domain & Protocol](../architecture/modules/domain-protocol.md)).
- [x] Создать Protobuf control envelope и JSON Schemas manifest/settings/profile (→ [Модель плагинов — Native control protocol](../contracts/plugins.md#native-control-protocol)).
- [x] Реализовать unknown-field/schema-version round-trip tests (→ [Архитектура — Правила эволюции](../architecture/overview.md#правила-эволюции)).
- [x] Зарезервировать namespaced stream/remote extension boundary без обещания стабильного buffer ABI до будущего spike (→ [Модуль: Remote Agent](../architecture/modules/remote-agent.md), [Модуль: Media Runtime](../architecture/modules/media-runtime.md)).
- [x] Создать generator task для Rust/TypeScript types (→ [Модель плагинов — SDK](../contracts/plugins.md#sdk)).
- [x] Зафиксировать HostBootId/ActivationEpoch, ingress order/clock domains и permissive protocol → SDK boundary; добавить domain/protocol Linux CI (→ [ADR-0013](../decisions/0013-dependency-and-host-service-boundaries.md), [Контракты сигналов](../contracts/lifecycle.md#порядок-сигналов-и-время)).
- [x] Задать input/result/error, InvocationId/JobId, artifact reference и dynamic descriptor schemas; согласование required features/limits (DW-WS-007, DW-PL-007) проверить до SDK freeze (→ [Automation](../contracts/automation.md), [Plugins](../contracts/plugins.md)).
- [x] Добавить независимые descriptor fixtures observe keyboard, analog travel, irregular RGB topology, screen/pointer, async service и media template; не создавать production media (→ [Lighting](../contracts/lighting.md), [Media](../contracts/media.md), [Сценарии](../product/scenarios.md)).

### Тесты

- [x] Юнит: ID/descriptor/envelope validation и stable serialization.
- [x] Property-based: unknown optional fields и invalid ranges.
- [x] Contract: Rust и TypeScript читают один fixture одинаково.
- [x] Contract: unknown fields/enums/presence проверяются на выбранных codegen libraries; JSON round-trip тестируется отдельно от wire.

Контрактная приёмка использует общий hex wire fixture, три расширяемых document fixtures и независимые domain fixtures.
Prost typed decode намеренно не считается прозрачным relay для неизвестных wire fields; opaque bytes сохраняются отдельно.
Protobuf-ES retention, неизвестные enum numbers и optional presence проверяются тестами. Linux/Windows core CI выполняет
generation drift check, TypeScript contracts и Rust domain/protocol tests.

### Ссылки

- [Capabilities](../contracts/capabilities.md)
- [ADR-0002](../decisions/0002-plugin-execution.md)

---

## Фаза 3 — Plugin supervisor и virtual package

**Цель:** доказать lazy, isolated, restartable plugin lifecycle.
**Результат:** virtual test package индексируется без запуска, активируется lease, публикует device и восстанавливается после crash.
**Трудоёмкость:** L
**Статус:** [~] Foundations готовы; аудит выявил незакрытые registry/lease/I/O и Host integration paths. Они закрываются срезами A/C [pre-apply gate](#pre-apply-gate).

### Контекст для агента
> Domain/protocol types существуют. Hardware и UI ещё не нужны; virtual package служит единственным provider и reference fixture.

### Задачи

- [x] Реализовать manifest index/staging/hash validation (→ [Модуль: Plugin Supervisor](../architecture/modules/plugin-supervisor.md)).
- [x] Реализовать Protobuf stdio handshake, limits, heartbeat и stop deadline (→ [Модель плагинов — Lifecycle](../contracts/plugins.md#lifecycle)).
- [x] Реализовать Capability Registry и session invalidation (→ [Модуль: Capability Registry](../architecture/modules/capability-registry.md)).
- [x] Создать permissive virtual DeviceProvider/ActionProvider example (→ [Модуль: Diagnostics & Test Lab](../architecture/modules/diagnostics-test-lab.md)).
- [x] Добавить restart/backoff/circuit-breaker tests (→ [ADR-0005](../decisions/0005-failure-isolation.md)).
- [x] Добавить scoped discovery lease до первого binding, canonical resource arbiter и host authorization/credential ports (→ [Runtime contracts](../contracts/lifecycle.md#resources-discovery-и-версии-packages), [Host Platform Services](../architecture/modules/host-platform-services.md)).
- [x] Подтвердить минимальные Rust/TS SDK helpers одним wire fixture и C0 manifest scanner на pinned open plugin (→ [SDK](../contracts/plugins.md#sdk), [Compatibility Host](../architecture/modules/compatibility-host.md)).
- [ ] Закрыть registry/owner/I/O gaps и production Host coordination по [срезам A/C](#pre-apply-gate); ручное связывание компонентов в integration test не заменяет composition приложения.

### Тесты

- [x] Юнит: manifest/permission/session state machines.
- [x] Интеграционный: index без process; lease starts; release stops after grace.
- [x] Fault injection: crash, hang, oversized message, late old-session event.
- [x] Ownership: discovery на пустом workspace; два provider aliases/probe не получают один exclusive interface.
- [x] Contract: atomic descriptor delta/base resync, removed port, stale revision и unsupported required feature; independent contribution остаётся доступной.

Нативная fixture проходит `staging → SHA-256 recheck → permission → process-tree guard → Hello/Ack →
descriptor registration → heartbeat → idle grace → deactivation`. Windows использует Job Object с
`KILL_ON_JOB_CLOSE`, Unix — отдельную process group; orphan-grandchild проверяется integration test.
Bounded stderr и Protobuf framing не смешиваются. Отзыв grant сначала инвалидирует registry routes,
затем останавливает runtime без автоматического restart.

Ранний [OpenAction C0/C1 spike](../research/openaction-c0-c1-spike.md) индексирует pinned MIT Counter
manifest без исполнения. C1 честно оставлен будущему отдельному WebSocket compatibility host: наличие
manifest или PI не выдаётся за runtime compatibility.
Executable C1 spike остаётся обязательным условием до custom web/PI Фазы 13; production C1 входит в Фазу 14.

### Ссылки

- [Plugins](../contracts/plugins.md)
- [Архитектура — Plugin discovery и activation](../architecture/overview.md#plugin-discovery-и-activation)

---

## Фаза 4 — Profile revisions и application API

**Цель:** создать безопасный путь read → draft → validate → plan → apply → rollback.
**Результат:** CLI/test client редактирует virtual workspace, получает conflict и возвращается к snapshot.
**Трудоёмкость:** L
**Статус:** [~] В процессе: готовы Profile Store, Workspace Configuration и отделённый Application Gateway; DDD migration R1–R4 завершена. Следующая работа — срезы A–E [pre-apply gate](#pre-apply-gate).

### Контекст для агента
> Plugin lifecycle проверен отдельными fixtures, production coordinator ещё требуется. Active automation пока пуст; фаза строит источник истины и command/query boundary.

Первый срез хранит revision record с checksum и unknown JSON fields, а assets — по SHA-256 с публикацией
metadata после bytes. Пути строятся через opaque hash IDs. Второй срез добавил append-only draft
transactions, atomic head с резервной копией, durable receipts и deduplication до проверки версии.
Undo/redo ограничен памятью editor session и намеренно не восстанавливается после restart. При чтении
резервного head следующая mutation сначала безопасно восстанавливает основной указатель. Третий срез
добавил selective export/import и credential placeholders. Named snapshot неизменно ссылается на сохранённую
revision; rollback создаёт новый draft с исходным package lock, не переписывая revision или snapshot.
Export переносит выбранные revision contents как portable templates: поэтому исключение private section
не подменяет checksum/identity исходной revision. Import проверяет bundle/assets до публикации, создаёт
новый draft, оставляет CredentialRef unresolved и не загружает/не запускает package metadata. Следующий
срез добавил transport-independent command/query ApplicationService и in-process client. Profiles
реализует DraftRepository port, а Application Gateway проверяет principal scopes, expected DraftVersion и stable API errors.
Dry-run использует тот же command vocabulary, но не пишет draft и не резервирует idempotency key;
повтор mutation после restart читает durable receipt и согласованный исторический document. Global dedup windows, production runtime coordination и apply пока отсутствуют; порядок закрытия задаёт [gate A–E](#pre-apply-gate).

### Задачи

- [x] Реализовать JSON revision store, assets, atomic write и recovery (→ [Модуль: Profile Store](../architecture/modules/profile-store.md)).
- [x] Реализовать draft commands, undo/redo и named snapshots (→ [ADR-0003](../decisions/0003-storage.md)).
- [x] Реализовать selective export/import, asset reachability и unresolved credential mapping без запуска packages (→ [Profile Store](../architecture/modules/profile-store.md), [Контракты миграций](../contracts/lifecycle.md#миграции-и-неизвестные-данные)).
- [x] Реализовать transport-independent ApplicationService и expected draft revision (→ [Модуль: Local API](../architecture/modules/local-api.md)).
- [ ] Реализовать OperationId, durable operation tracking и status queries через Application Gateway (→ [Модуль: Local API](../architecture/modules/local-api.md)).
- [ ] Реализовать Host apply skeleton с durable commit record, barrier, disruption/degraded policy, package lock и operation receipts (→ [ADR-0012](../decisions/0012-apply-and-execution-lifecycle.md), [Модуль: Host Runtime](../architecture/modules/host-runtime.md)).
- [x] Проверять principals/scopes внутри ApplicationService, per-draft durable dedup до revision check, dry-run без effects (→ [API](../contracts/application-api.md#идемпотентность-и-dry-run)).
- [ ] Завершить global dedup/expiry, identity/ownership, runtime coordination и storage reconciliation по [gate A–E](#pre-apply-gate) до объявления apply готовым.
- [x] Добавить in-process test client; HTTP/WS transport отложить до Фазы 16 (→ [Application API](../contracts/application-api.md)).

### Тесты

- [x] Юнит: draft commands, undo/redo, revision conflicts.
- [x] Integration: crash-safe atomic write и recovery corrupted temp.
- [ ] Scenario: failure до commit оставляет прежнюю выбранную revision; после commit восстанавливается новая без replay actions.
- [ ] Fault: kill на каждой стадии записи/apply, disk full, grant revoke во время prepare, exclusive handoff failure, concurrent draft edits, повтор key после restart.
- [x] Import/export: secrets/grants отсутствуют, unknown fields сохраняются, snapshot rollback использует прежний package hash.

### Ссылки

- [ADR-0008](../decisions/0008-api-first-and-revisions.md)
- [Архитектура — Draft, compile и apply](../architecture/overview.md#draft-compile-и-apply)

---

## Фаза 5 — Минимальный automation engine

**Цель:** выполнить deterministic source → action → feedback path.
**Результат:** virtual button вызывает virtual action, меняет state и feedback; replay повторяет результат.
**Трудоёмкость:** L
**Статус:** [ ] Не начата

### Контекст для агента
> Profile/API и virtual plugin готовы. Реализуется узкий graph subset без schedules и complex UI.

### Задачи

- [ ] Реализовать graph compiler для source/action/state/condition/feedback (→ [Модуль: Automation Engine](../architecture/modules/automation-engine.md)).
- [ ] Заменить ограниченный compiler adapter Фазы 4, сохранив [apply acceptance](#pre-apply-gate); проверять typed graph/schema semantics отдельно от structural JSON validation. M1 проходит через production Host и isolated virtual process.
- [ ] Реализовать immutable ExecutionPlan и serialized workspace scheduler (→ [ADR-0010](../decisions/0010-compiled-automation-plan.md)).
- [ ] Разделить edge queue и coalescing value lane (→ [Архитектура — Конкурентность и QoS](../architecture/overview.md#конкурентность-и-qos)).
- [ ] Реализовать trace/correlation/causation и basic loop guard (→ [Модуль: Diagnostics & Test Lab](../architecture/modules/diagnostics-test-lab.md)).
- [ ] Подключить compile/plan/apply к Host Runtime (→ [Модуль: Host Runtime](../architecture/modules/host-runtime.md)).
- [ ] Реализовать execution epochs, held-output cleanup, bounded cancellation и unknown-outcome retry policy (→ [Lifecycle effects](../contracts/lifecycle.md#executions-cancellation-и-смена-плана)).
- [ ] Создать минимальный opt-in recorder/replay с outcomes и без live dispatch; UI/расширенные fixtures следуют в Фазе 10 (→ [Diagnostics & Test Lab](../architecture/modules/diagnostics-test-lab.md)).
- [ ] Реализовать typed invocation/reply, job correlation и duplicate/late completion guard; проверить mixed descriptor graph без special cases и negative schema edges (→ [Automation](../contracts/automation.md), [Сценарии](../product/scenarios.md)).

### Тесты

- [ ] Юнит: graph validation, schema mismatch, cycle rejection.
- [ ] Deterministic: один fixture + fake clock даёт identical final state.
- [ ] Performance baseline: virtual input → dispatch latency.
- [ ] Ordering/fault: encoder +1,+1,-1 не теряется; axis-before-button не подменяется будущим axis; edge overflow освобождает held outputs; late completion после apply не запускает action.
- [ ] Result: accepted job не считается success; два invocation получают свои outputs; malformed/oversized result отклоняется, duplicate terminal не создаёт второй effect.
- [ ] Cross-context: два одинаковых device instances и service с одинаковыми локальными ports; topology/schema change после plan, revoke во время job и старый completion после apply.

### Ссылки

- [Архитектура — Device event → automation → feedback](../architecture/overview.md#device-event--automation--feedback)
- [Требования DW-AU-001–004](../product/requirements.md#назначения-и-автоматизация)

---

## Фаза 6 — Desktop workspace canvas

**Цель:** дать пользователю визуально настроить virtual device без редактирования JSON.
**Результат:** device виден на infinite canvas, control выбирается, action перетаскивается, draft валидируется и применяется.
**Трудоёмкость:** L
**Статус:** [ ] Не начата

### Контекст для агента
> ApplicationService и minimal graph работают. UI использует только этот contract и не получает plugin handles.

### Задачи

- [ ] До canvas подключить Tauri adapter к Application Gateway с host-derived principal и общими DTO; проверить parity command/query/error с in-process client, receipt replay и snapshot reconnect (→ [Application API](../contracts/application-api.md)).

- [ ] Реализовать virtualized infinite canvas и device cards (→ [Модуль: Desktop Shell](../architecture/modules/desktop-shell.md)).
- [ ] Реализовать action library/search и drag-and-drop binding (→ [UI — Progressive disclosure](../product/ux.md#progressive-disclosure)).
- [ ] Реализовать schema-form inspector, draft status, inline validation, undo/redo/apply (→ [Модуль: Profile Store](../architecture/modules/profile-store.md), [Settings UI](../contracts/plugins.md#settings-и-ui-contributions)).
- [ ] Реализовать Runtime health summary и no-device/first-run states (→ [UI — Состояния интерфейса](../product/ux.md#состояния-интерфейса)).
- [ ] Добавить ru/en localization skeleton и keyboard navigation (→ [UX](../product/ux.md)).

### Тесты

- [ ] Component: selection, drag/drop, inspector mutation.
- [ ] End-to-end: assignment за ≤ 3 действия.
- [ ] Manual: close/reopen WebView сохраняет active runtime/draft.

### Ссылки

- [Модуль: Desktop Shell](../architecture/modules/desktop-shell.md)
- [Требования DW-WS-004](../product/requirements.md#рабочее-пространство-и-устройства)

---

## Фаза 7 — Windows base provider и Platform Input

**Цель:** обеспечить полезные системные actions/states без N4-specific logic.
**Результат:** virtual controls меняют volume/mute/media и active window; штатная клавиатура создаёт triggers через bundled observe/pass-through provider.
**Трудоёмкость:** M
**Статус:** [ ] Не начата

### Контекст для агента
> Native plugin lifecycle и UI binding готовы. Windows functionality реализуется отдельным first-party control package.

### Задачи

- [ ] Реализовать Windows ActionProvider: volume/mute/media/app launch/focus (→ [Модуль: Platform Adapters](../architecture/modules/platform-adapters.md)).
- [ ] Реализовать StateProvider active application/window и audio endpoint state (→ [Capabilities — State](../contracts/capabilities.md#state)).
- [ ] Реализовать bundled Platform Input keyboard source с безопасным observe/pass-through default (→ [Модуль: Platform Input](../architecture/modules/platform-input.md), [ADR-0017](../decisions/0017-bundled-platform-input.md)).
- [ ] Объявлять `physical_device` только при подтверждённой identity, иначе `system_aggregate`; intercept/emit не включать вместе с observe неявно (→ [Capabilities](../contracts/capabilities.md#системная-клавиатура-и-granularity)).
- [ ] Добавить permission descriptors и unavailable reasons (→ [Безопасность — Permission model](../contracts/security.md#permission-model)).
- [ ] Добавить origin handling для emitted media/input commands (→ [Модуль: Automation Engine](../architecture/modules/automation-engine.md)).

### Тесты

- [ ] Unit: OS result → domain outcome mapping.
- [ ] Integration: volume/mute state round-trip на Windows 10/11 CI/manual matrix.
- [ ] Manual: active-window condition переключает virtual feedback.
- [ ] Scenario: штатная клавиша вызывает virtual action и продолжает обычное действие ОС; reconnect/повтор не создаёт duplicate edge.

### Ссылки

- [Требования DW-IN-001](../product/requirements.md#системный-ввод-и-hardware-provisioning)
- [Режимы — Режимы input](../product/modes.md#режимы-input)

---

## Фаза 8 — N4 Pro hardware provider

**Цель:** подключить всё подтверждённое hardware N4 Pro как typed capabilities.
**Результат:** подтверждённые keys/encoders, coordinate samples, screen targets, light/haptic modes видны в registry; ограничения firmware и неподтверждённый contact lifecycle явно отмечены.
**Трудоёмкость:** L
**Статус:** [ ] Не начата

### Контекст для агента
> Windows actions и plugin SDK foundation работают. Эта фаза не создаёт final surface editor, а доказывает protocol/device lifecycle.

### Задачи

- [ ] Реализовать discovery/identity/open/reconnect/keepalive (→ [Модуль: N4 Pro Plugin](../architecture/modules/n4pro-plugin.md)).
- [ ] Описать 10 keys, 4 encoder rotations/presses и фактически наблюдаемые pointer samples/contact phases; отдельно проверить основной display и touch-strip addressing/transform (→ [Capabilities](../contracts/capabilities.md), [N4 source audit](../research/implementation-review.md#n4-pro-полезный-код-и-ограничения-доказательств)).
- [ ] Реализовать full display FrameSink и brightness, если hardware поддерживает (→ [Модуль: Surface Compositor](../architecture/modules/surface-compositor.md)).
- [ ] Реализовать actual light targets и отличить built-in vibration enable от самостоятельного pattern playback; unsupported режимы не эмулируются переключением enable (→ [Требования DW-SF-005](../product/requirements.md#surface-feedback-и-n4-pro)).
- [ ] Создать captured protocol fixtures и manual hardware checklist (→ [Модуль: Diagnostics & Test Lab](../architecture/modules/diagnostics-test-lab.md)).

### Тесты

- [ ] Unit: report parser/encoder, bounds, model/firmware variants.
- [ ] Integration: fake transport connect/events/outputs/disconnect.
- [ ] Manual: real N4 Pro hotplug, all controls, individual rings, haptic.
- [ ] Evidence: truncated touch report, C++ callback regression, sample без release, separate screen dimensions и volatile/persistent command classification. Неподтверждённые P0 блокируют M2 либо требуют пересмотра требования/ADR.

### Ссылки

- [Модуль: N4 Pro Plugin](../architecture/modules/n4pro-plugin.md)
- [research/REFERENCES.md](../research/REFERENCES.md)

---

## Фаза 9 — Surface compositor

**Цель:** вывести свободную композицию на полный экран N4 Pro и связать independent touch regions.
**Результат:** текст/иконки/фон занимают весь canvas без grid gaps; touch regions запускают Windows actions.
**Трудоёмкость:** L
**Статус:** [ ] Не начата

### Контекст для агента
> N4 Pro target addressing и coordinate mapping проверены Фазой 8. Contact lifecycle не предполагается: sample-only provider не включает hold/drag. Нужен built-in scene renderer без custom web runtime.

### Задачи

- [ ] Реализовать retained scene nodes, transforms, clipping и state bindings (→ [Модуль: Surface Compositor](../architecture/modules/surface-compositor.md)).
- [ ] Реализовать independent hit regions и coordinate transforms (→ [ADR-0009](../decisions/0009-surface-scene-model.md)).
- [ ] Реализовать dirty invalidation, per-surface FPS и latest-frame-wins (→ [Архитектура — Scene → hardware frame](../architecture/overview.md#scene--hardware-frame)).
- [ ] Реализовать Surface editor: visual/hit layers, snap templates, preview/stop (→ [UI — Surface editor](../product/ux.md#surface-editor)).
- [ ] Выбрать renderer по benchmark ADR; реализовать frame/base IDs, full resync и preview lease/TTL с отзывом при apply (→ [Frames и preview](../contracts/lifecycle.md#frames-и-preview)).
- [ ] Добавить bounded timeline для стандартных scene nodes (→ [Surface Compositor](../architecture/modules/surface-compositor.md)).
- [ ] Создать стандартные text/icon/image/state widgets без browser runtime (→ [Capabilities — Визуальные поверхности](../contracts/capabilities.md#визуальные-поверхности)).
- [ ] Реализовать LightTopology/LightFrame и единый output arbiter для static scene/preview/preset; проверить irregular virtual keyboard до reactive effects (→ [Lighting](../contracts/lighting.md), [Surface Compositor](../architecture/modules/surface-compositor.md)).

### Тесты

- [ ] Golden image: full canvas и transforms.
- [ ] Hit-test: raw → transformed point → correct region.
- [ ] Load: slow fake sink keeps bounded frame queue.
- [ ] Fault: пропущенный diff/ACK вызывает полный кадр; preview expiry после apply восстанавливает новую сцену, preview touch не запускает production action.
- [ ] Manual: N4 Pro full-screen scene без чёрных промежутков.

### Ссылки

- [Требования DW-SF-001–006](../product/requirements.md#surface-feedback-и-n4-pro)
- [Модуль: Surface Compositor](../architecture/modules/surface-compositor.md)

---

## Фаза 10 — Gamepad и Test Lab

**Цель:** доказать независимость архитектуры от Stream Deck-класса и сделать hardware-independent replay.
**Результат:** Bluetooth gamepad buttons/axes/gyro/rumble работают рядом с N4 Pro; trace воспроизводится virtual provider.
**Трудоёмкость:** M
**Статус:** [ ] Не начата

### Контекст для агента
> N4 vertical slice полезна. Второй класс устройства должен использовать те же domain/plugin contracts без core changes.

### Задачи

- [ ] Реализовать gamepad discovery/buttons/axes/hotplug (→ [Модуль: Gamepad Plugin](../architecture/modules/gamepad-plugin.md)).
- [ ] Публиковать gyro/accelerometer, rumble/light только при actual support (→ [Capabilities — Непрерывный ввод и датчики](../contracts/capabilities.md#непрерывный-ввод-и-датчики)).
- [ ] Реализовать bounded event recorder и redacted trace format (→ [Модуль: Diagnostics & Test Lab](../architecture/modules/diagnostics-test-lab.md)).
- [ ] Реализовать replay runner/fake clock/virtual device fixture (→ [Требования DW-AU-007](../product/requirements.md#назначения-и-автоматизация)).
- [ ] Добавить fake keyboard/mouse/gamepad/surface и production SimulationRunner UI без dependency на testkit; OS virtual endpoints не требуются для этих fixtures (→ [Automation Engine](../architecture/modules/automation-engine.md), [Diagnostics & Test Lab](../architecture/modules/diagnostics-test-lab.md)).
- [ ] Добавить latency/queue/drop metrics в Runtime UI (→ [Модуль: Desktop Shell](../architecture/modules/desktop-shell.md)).

### Тесты

- [ ] Integration: two gamepads/reconnect/identity.
- [ ] Deterministic replay: axis + button + N4 feedback.
- [ ] Manual: Bluetooth rumble cancellation и motion availability.

### Ссылки

- [Модуль: Gamepad Plugin](../architecture/modules/gamepad-plugin.md)
- [Модуль: Diagnostics & Test Lab](../architecture/modules/diagnostics-test-lab.md)

---

## Фаза 11 — Layers и сложные automation

**Цель:** реализовать подтверждённые сценарии beyond one-binding.
**Результат:** temporary layer, active-window condition, sequence/parallel, timer и error policies редактируются и симулируются.
**Трудоёмкость:** L
**Статус:** [ ] Не начата

### Контекст для агента
> Minimal compiler/scheduler и replay уже стабильны. Расширяется node vocabulary без arbitrary code in core.

### Задачи

- [ ] Добавить layers, variables, tap/hold/chord и active-window conditions (→ [Модуль: Automation Engine](../architecture/modules/automation-engine.md)).
- [ ] Добавить sequence/parallel/join и stop/continue/retry/fallback policies (→ [Режимы — Режимы ошибок automation](../product/modes.md#режимы-ошибок-automation)).
- [ ] Добавить monotonic timers, schedules с timezone/DST и bounded cycles (→ [Capabilities — Временная семантика](../contracts/capabilities.md#временная-семантика)).
- [ ] Реализовать Graph editor, collapsed groups и execution timeline (→ [UI — Graph editor](../product/ux.md#graph-editor)).
- [ ] Реализовать conflict/priority diagnostics (→ [Требования DW-AU-008](../product/requirements.md#назначения-и-автоматизация)).
- [ ] Реализовать logical device groups без потери member identity и cleanup temporary layers на disconnect/epoch change (→ [Capability Registry](../architecture/modules/capability-registry.md), [Execution lifecycle](../contracts/lifecycle.md#executions-cancellation-и-смена-плана)).
- [ ] Связать typed result fields через run-local DataEdges; реализовать branch scopes, join/skip/null, bounded reentrancy и query/cancel jobs (DW-AU-011/012) (→ [Automation contract](../contracts/automation.md)).
- [ ] Реализовать scoped ArtifactBroker, transient storage/GC/TTL/quota и redacted run journal; expose через application API, без private данных в profile assets (→ [Host Runtime](../architecture/modules/host-runtime.md), [Artifacts](../contracts/automation.md#artifact-и-доступ-к-данным)).
- [ ] Добавить independent fake transcription/browser/mail jobs и notification collection query; paginated scene view проверяет DW-AU-013, production integrations остаются P2 (→ [Сценарии](../product/scenarios.md)).
- [ ] Реализовать spatial wave и per-key dim/cooldown/fade, retrigger/merge policies и layer feedback на irregular virtual topology (DW-SF-009/010) (→ [Lighting](../contracts/lighting.md), [UX](../product/ux.md#геометрия-и-свет)).

### Тесты

- [ ] State-machine: tap/hold/chord/layer release.
- [ ] Parallel/error: один recorded completion order воспроизводим; разные порядки проверяют явную join/priority policy, а не обещание одинаковых внешних effects.
- [ ] Time: fake clock, timezone/DST schedule.
- [ ] Manual: held keyboard/gamepad control changes N4 encoder assignment.
- [ ] Gate M2: все P0 требования и первый сквозной критерий выполнены, включая export/portable roots/recording; evidence привязано к ID требований.
- [ ] Typed workflow: два run не смешивают transcript/tab handles, unknown mail outcome не повторяется, artifact revoke/expiry останавливает дальнейшее чтение.
- [ ] Collections/lighting: gap/cursor expiry/edit/delete; shared LED, repeat press, fade при frame drop и конфликт preview/hardware preset.

### Ссылки

- [ADR-0010](../decisions/0010-compiled-automation-plan.md)
- [Требования DW-AU-002–008](../product/requirements.md#назначения-и-автоматизация)

---

## Фаза 12 — Plugin SDK и packaging

**Цель:** позволить стороннему автору создать, протестировать и установить native control package.
**Результат:** Rust, TypeScript и Python examples проходят conformance; immutable artifact устанавливается и откатывается.
**Трудоёмкость:** L
**Статус:** [ ] Не начата

### Контекст для агента
> Protocol/supervisor используются first-party packages. Нужно отделить permissive SDK и public tooling от GPL application implementation.

### Задачи

- [ ] Опубликовать Rust SDK helpers без скрытия lifecycle/backpressure (→ [Модель плагинов — SDK](../contracts/plugins.md#sdk)).
- [ ] Опубликовать TypeScript SDK/reference runtime packaging (→ [Модуль: Domain & Protocol](../architecture/modules/domain-protocol.md)).
- [ ] Опубликовать Python SDK для isolated conditions/actions с timeout/cancellation fixtures (→ [Модель плагинов — Языки](../contracts/plugins.md#языки)).
- [ ] Реализовать conformance CLI/fixtures/report (→ [Модуль: Diagnostics & Test Lab](../architecture/modules/diagnostics-test-lab.md)).
- [ ] Реализовать staging install/update/rollback и permission diff (→ [Модуль: Plugin Supervisor](../architecture/modules/plugin-supervisor.md)).
- [ ] Проверять exact artifact hashes/package lock, one-active-version conflict и bounded declarative settings migration без executable hooks (→ [Runtime contracts](../contracts/lifecycle.md#resources-discovery-и-версии-packages)).
- [ ] Описать metadata catalogue format с GitHub immutable artifacts (→ [Модель плагинов — Пакетирование и каталог](../contracts/plugins.md#пакетирование-и-каталог)).

### Тесты

- [ ] Contract: Rust/TS/Python examples produce identical semantic fixtures.
- [ ] Security: path traversal, oversized archive, missing hash, expanded permission.
- [ ] Upgrade: failed new version restores previous package/runtime.

### Ссылки

- [ADR-0007](../decisions/0007-licensing.md)
- [Безопасность — Plugin install/update](../contracts/security.md#plugin-installupdate)

---

## Фаза 13 — Widget и Interaction runtime

**Цель:** поддержать custom settings/surface widgets и host-owned runtime interactions без privileged plugin windows.
**Результат:** sample widget возвращает frame; search interaction возвращает typed selection конкретному invocation и корректно cancel/timeout.
**Трудоёмкость:** L
**Статус:** [ ] Не начата

### Контекст для агента
> Built-in scene nodes уже покрывают простые widgets, а schema-native interactions рендерит desktop host. Lazy separate web host нужен только package-provided custom content.

### Задачи

- [ ] До custom web/PI выполнить executable C1 lifecycle spike на pinned open fixture; C0 manifest scan не закрывает этот риск (→ [отчёт spike](../research/openaction-c0-c1-spike.md)).
- [ ] До presenter завершить Interaction application service: result schema validation, query generation/cursor scope, provider/epoch fencing, monotonic deadlines, byte/count budgets и eviction terminal sessions; R4 содержит только начальный scaffold (→ [Interaction Runtime](../architecture/modules/interaction-runtime.md)).

- [ ] Выбрать/зафиксировать widget backend после Windows/Linux spike (→ [Модуль: Widget Runtime](../architecture/modules/widget-runtime.md)).
- [ ] Реализовать isolated origins, CSP, scoped bridge и rate limits (→ [Безопасность — Web property editors и widgets](../contracts/security.md#web-property-editors-и-widgets)).
- [ ] Подключить WidgetLayer к compositor без unbounded frames (→ [Модуль: Surface Compositor](../architecture/modules/surface-compositor.md)).
- [ ] Реализовать schema form → custom editor fallback chain (→ [Plugins — Settings и UI contributions](../contracts/plugins.md#settings-и-ui-contributions)).
- [ ] Реализовать host-owned schema-native list/search/form/confirm interactions и session lifecycle (→ [Interaction Runtime](../architecture/modules/interaction-runtime.md), [ADR-0018](../decisions/0018-interaction-ui-runtime.md)).
- [ ] Подключить InteractionProvider pages/result через scoped bridge; custom view переиспользует web isolation без Tauri/window API (→ [Automation](../contracts/automation.md#interaction-sessions)).
- [ ] Собрать fake keyboard → search → selected ItemRef → open action scenario до реального Zotero package (→ [Сценарий Zotero](../product/scenarios.md#клавиатура-ноутбука--поиск-zotero--pdf)).

### Тесты

- [ ] Security: forbidden navigation/network/filesystem.
- [ ] Fault: widget crash/hang leaves host/surface alive.
- [ ] Performance: frame budget and idle host not launched when unused.
- [ ] Interaction: concurrent sessions, keyboard navigation, Escape, timeout, provider crash, stale result after apply.
- [ ] Interaction contracts: старый search response после нового query не заменяет page; invalid selected value, result после deadline, clock jump и повторные session open/close не обходят validation/retention.

### Ссылки

- [Требования DW-SF-007–008](../product/requirements.md#surface-feedback-и-n4-pro)
- [Требования DW-AU-015 и DW-PL-008](../product/requirements.md#назначения-и-автоматизация)
- [Модуль: Widget Runtime](../architecture/modules/widget-runtime.md)
- [Модуль: Interaction Runtime](../architecture/modules/interaction-runtime.md)

---

## Фаза 14 — OpenAction compatibility

**Цель:** использовать существующие actions без подчинения core legacy model.
**Результат:** выбранные open packages достигают C0–C3 и имеют versioned compatibility reports.
**Трудоёмкость:** L
**Статус:** [ ] Не начата

### Контекст для агента
> Native SDK и widget isolation существуют. Compatibility host остаётся отдельным process tree и переводит contexts на bindings.

### Задачи

- [ ] Реализовать legacy manifest indexing C0 (→ [Модуль: Compatibility Host](../architecture/modules/compatibility-host.md)).
- [ ] Реализовать registration/lifecycle/key events/settings C1 (→ [Совместимость — Уровни совместимости](../contracts/compatibility.md#уровни-совместимости)).
- [ ] Реализовать scoped Property Inspector C2 (→ [Совместимость — Property Inspector](../contracts/compatibility.md#property-inspector)).
- [ ] Реализовать runtime settings overlay, explicit merge в draft и native replacement migration с rollback (→ [Compatibility settings](../contracts/compatibility.md#property-inspector), [Миграция](../contracts/compatibility.md#миграция-в-нативный-plugin)).
- [ ] Реализовать dial/touch/image/title/state bridge C3 (→ [Модуль: Surface Compositor](../architecture/modules/surface-compositor.md)).
- [ ] Публиковать exact version/OS compatibility matrix (→ [Модуль: Diagnostics & Test Lab](../architecture/modules/diagnostics-test-lab.md)).

### Тесты

- [ ] Conformance fixtures lifecycle/settings/feedback/dial/PI.
- [ ] Fault: legacy crash/re-register/slow/oversized.
- [ ] Isolation/settings: два PI не видят чужие storage/bridge; setSettings/restart не мутирует immutable revision и не перезаписывает открытый draft.
- [ ] Manual: несколько открытых reference packages на N4 Pro layout.

### Ссылки

- [ADR-0006](../decisions/0006-openaction-compatibility.md)
- [Совместимость](../contracts/compatibility.md)

---

## Фаза 15 — Device Studio

**Цель:** упростить добавление standard HID/gamepad-like devices и подготовку vendor provider fixtures.
**Результат:** пользователь видит interfaces/live events, калибрует axis и сохраняет validated descriptor project.
**Трудоёмкость:** L
**Статус:** [ ] Не начата

### Контекст для агента
> Providers, test lab и UI уже существуют. Protocol Lab остаётся безопасным diagnostic tool, а не arbitrary USB console.

### Задачи

- [ ] Реализовать bounded probe session и standard discovery view (→ [Модуль: Device Studio](../architecture/modules/device-studio.md)).
- [ ] Реализовать live event/value inspector и calibration (→ [Capabilities](../contracts/capabilities.md)).
- [ ] Реализовать safe capture trace и descriptor project validation (→ [Модуль: Diagnostics & Test Lab](../architecture/modules/diagnostics-test-lab.md)).
- [ ] Подключить generated declarative mapping как package/project artifact (→ [Plugins](../contracts/plugins.md)).
- [ ] Добавить provisioning descriptor/readback UI без write implementation (→ [Capabilities — Provisioning capability](../contracts/capabilities.md#provisioning-capability)).

### Тесты

- [ ] Integration: generic HID/gamepad virtual fixtures.
- [ ] Safety: unknown output cannot be sent.
- [ ] Manual: calibrate a real axis and export descriptor project.

### Ссылки

- [UI — Device Studio](../product/ux.md#device-studio)
- [Требования DW-IN-005](../product/requirements.md#системный-ввод-и-hardware-provisioning)

---

## Фаза 16 — Codex, CLI и optional MCP

**Цель:** обеспечить полный поддерживаемый machine workflow без direct file edits.
**Результат:** CLI/Codex создаёт draft, валидирует, симулирует и применяет его; MCP adapter можно удалить без потери API.
**Трудоёмкость:** M
**Статус:** [ ] Не начата

### Контекст для агента
> ApplicationService стабилен и используется UI. Фаза добавляет authenticated loopback transport и thin clients.

### Задачи

- [ ] Реализовать loopback HTTP/JSON и WebSocket subscriptions (→ [Модуль: Local API](../architecture/modules/local-api.md)).
- [ ] Реализовать token/origin/rate/size security (→ [Безопасность — Local API](../contracts/security.md#local-api)).
- [ ] Реализовать CLI для query/draft/validate/simulate/apply/operation (→ [Application API](../contracts/application-api.md)).
- [ ] Реализовать optional MCP adapter как отдельный package/process (→ [Application API — MCP adapter](../contracts/application-api.md#mcp-adapter)).
- [ ] Добавить Codex-oriented schemas/examples и revision conflict flow (→ [ADR-0008](../decisions/0008-api-first-and-revisions.md)).

### Тесты

- [ ] Contract: direct/HTTP/CLI/MCP adapters дают одинаковые results.
- [ ] Security: external bind/origin/token/confirmation rejection.
- [ ] API: повтор mutation после потерянного ответа/restart, payload conflict, expired dedup epoch, revoke principal и snapshot/delta resync.
- [ ] End-to-end: Codex-like client builds and applies N4 workspace.

### Ссылки

- [Модуль: Local API](../architecture/modules/local-api.md)
- [Требования DW-API-001–003](../product/requirements.md#api-профили-и-эксплуатация)

---

## Фаза 17 — Portable build и Linux foundation

**Цель:** подтвердить deployment portability и отсутствие Windows dependencies в core.
**Результат:** portable Windows build использует отдельный data root; Linux host/virtual provider/CLI compile и run.
**Трудоёмкость:** L
**Статус:** [ ] Не начата

### Контекст для агента
> Windows v1 features готовы. Linux phase не обязана сразу поддержать весь hardware, но должна доказать platform boundary.

### Задачи

- [ ] Упаковать installed/portable builds поверх data-root/startup policies Фазы 1, проверить update и writable/removable root (→ [Модуль: Host Runtime](../architecture/modules/host-runtime.md)).
- [ ] Расширить export/import Фазы 4 cross-platform unavailable report и Linux fixtures (→ [Модуль: Profile Store](../architecture/modules/profile-store.md)).
- [ ] Создать Linux platform package skeleton и active-window/basic action spike (→ [Модуль: Platform Adapters](../architecture/modules/platform-adapters.md)).
- [ ] Запустить core/plugin/virtual/UI smoke в Linux CI (→ [Архитектура — Технологические решения](../architecture/overview.md#технологические-решения)).
- [ ] Документировать platform matrix и secret portability (→ [Режимы](../product/modes.md), [Безопасность](../contracts/security.md)).

### Тесты

- [ ] Windows installed vs portable data isolation.
- [ ] Import Windows profile on Linux yields degraded, preserves settings.
- [ ] CI: domain/core/protocol/plugin conformance on Windows/Linux.

### Ссылки

- [ADR-0011](../decisions/0011-deployment-modes.md)
- [Требования DW-OP-002, DW-PR-002–003](../product/requirements.md#api-профили-и-эксплуатация)

---

## Фаза 18 — Hardening v1

**Цель:** подготовить публичный foundation release для реального ежедневного использования.
**Результат:** error paths, recovery, performance budgets, docs, packaging и release checklist пройдены.
**Трудоёмкость:** L
**Статус:** [ ] Не начата

### Контекст для агента
> Все v1 verticals реализованы. Новые features запрещены; фаза закрывает failure modes и доказательства качества.

### Задачи

- [ ] Проверить external input paths на panic/unwrap/unbounded work (→ [Архитектура — Режимы отказа](../architecture/overview.md#режимы-отказа)).
- [ ] Выполнить crash/reconnect/corruption/permission fault matrix (→ [Модуль: Diagnostics & Test Lab](../architecture/modules/diagnostics-test-lab.md)).
- [ ] Измерить и зафиксировать latency/RSS/frame/plugin activation budgets (→ [Архитектура — Бюджеты производительности](../architecture/overview.md#бюджеты-производительности)).
- [ ] Проверить secrets/log redaction, local API и package supply chain (→ [Безопасность](../contracts/security.md)).
- [ ] Провести ru/en, keyboard navigation и click-budget usability pass (→ [UX](../product/ux.md)).
- [ ] Проверить все public SDK/API/docs links и provenance (→ [docs/README.md](../README.md)).
- [ ] Собрать signed/checksummed Windows release и documented Linux preview (→ [ADR-0007](../decisions/0007-licensing.md)).

### Тесты

- [ ] Юнит: error branches для всех fallible public functions.
- [ ] Integration: каждый external dependency unavailable/crashed/stale.
- [ ] Soak: N4 Pro + gamepad + actions + frames + UI reopen.
- [ ] Recovery: host kill during profile write/apply/plugin update.
- [ ] Manual acceptance: полный [первый сквозной критерий](../product/requirements.md#первый-сквозной-критерий).

### Ссылки

- [Архитектура — Режимы отказа](../architecture/overview.md#режимы-отказа)
- [Модуль: Host Runtime](../architecture/modules/host-runtime.md)
- [Модуль: Plugin Supervisor](../architecture/modules/plugin-supervisor.md)

---

## Дальнейшие программы после v1

Это не обещания первого релиза. Каждая программа начинается отдельным ADR, threat model, prototype benchmark и собственным phased plan.

### Handy и интеграции приложений

- Gate A: version-pinned Handy CLI probe, установленная модель и WAV→JSON result fixture; interactive toggle отдельно как best-effort.
- Gate B: bounded microphone capture job → artifact lease → transcription; cancel, hotplug, privacy и пустой result.
- Gate C: browser adapter с tab/origin identity → fill composer, optional clipboard с ownership check; send — отдельный action.
- Gate D: mail receipt/idempotency и Telegram account/API/history limits, paginated feed и display view model.
- Сквозной результат DW-AU-014 подтверждается реальными adapters, не успехом mock; [сценарии](../product/scenarios.md), [исследование Handy](../research/implementation-review.md#handy-и-результаты-действий).

### Клавиатуры и аппаратные программы

- Первый RGB provider использует уже проверенные host light scenes; сравнить OpenRGB protocol adapter с прямым device transport по поддержке/ownership.
- Onboard offload начинается с ограниченных volatile presets, затем отдельный equivalence test host/device clock/retrigger/cleanup.
- Persistent program slots требуют provisioning diff/readback/endurance/recovery; firmware flashing исключён.
- Нормативные границы: [lighting](../contracts/lighting.md), [capabilities](../contracts/capabilities.md#provisioning-capability).

### Selective input replacement

- Windows observe → intercept/suppress → virtual keyboard/mouse/gamepad.
- Optional privileged helper/driver, emergency recovery и fail-open policy.
- Persistent keyboard/device profiles.
- Модули: [Platform Adapters](../architecture/modules/platform-adapters.md), [Security](../contracts/security.md#input-safety).

### Remote agents

- Local pairing and widget panel.
- Steam Deck/Android touch/display.
- Cable/Wi-Fi transport abstraction.
- Camera/microphone/speaker and screen capture only with session grants.
- Модуль: [Remote Agent](../architecture/modules/remote-agent.md).

### Low-latency media

- Backend/latency benchmark.
- Audio graph and virtual microphone.
- EQ/noise suppression.
- CLAP and optional GStreamer/VST bridges.
- Модуль: [Media Runtime](../architecture/modules/media-runtime.md).
- Gate DW-MD-004: exact caps/requestable ports, buffer ownership/copies, clock/generation, xruns и crash всего media-host; bypass возможен только при живом совместимом пути (→ [Media contract](../contracts/media.md)).

### Companion bridge

- Public interfaces first.
- Version-pinned internal tRPC adapter only with fixtures.
- Actions/variables/feedback mapping.
- Details: [Совместимость — Будущий Bitfocus Companion bridge](../contracts/compatibility.md#будущий-bitfocus-companion-bridge).

### Дополнительные платформы и hardware

- Full Linux input/output parity.
- macOS adapters and permissions.
- Flexible/magnetic keyboards with travel values.
- Device memory provisioning.
- Wheels/force feedback.
- Virtual display driver.

## Definition of Done для любой фазы

- build/lint/test зелёные на поддерживаемой matrix;
- documentation и schemas обновлены;
- no unbounded queues/process leaks;
- error/permission/unavailable states видимы;
- public API change имеет compatibility note;
- заимствование имеет provenance/license;
- фаза демонстрируется без незавершённого следующего этапа.
- P0/P1 acceptance связывается с конкретной fixture; runtime assertions не считаются выполненными по одной документации.
- Локальные Markdown paths/anchors, ссылки PLAN → modules и отсутствие production → testkit/SDK → GPL dependencies проверяются автоматически.
