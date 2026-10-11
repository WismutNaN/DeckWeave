# Архитектура DeckWeave

## Цель архитектуры

DeckWeave должен начать с N4 Pro и обычных системных действий, но не встраивать их особенности в ядро. Стабильная часть продукта — идентичность, capability vocabulary, revisions, automation semantics и application API. Всё, что зависит от конкретного hardware, ОС, чужой экосистемы, web runtime или real-time media, находится за заменяемой границей.

Архитектура оптимизируется не под максимальное число функций в первом релизе, а под дешёвое добавление нового provider без изменения уже работающих контрактов.

## Системный контекст

```text
                         ┌──────────────────────────────┐
                         │ Пользователь / автор plugin  │
                         └──────────────┬───────────────┘
                                        │ Desktop UI / CLI
                                        v
┌───────────────┐   local API   ┌──────────────────────────────────┐
│ Codex / MCP   │──────────────▶│ DeckWeave per-user tray host     │
└───────────────┘               │                                  │
                                │ Application service              │
┌───────────────┐  plugin IPC   │ Registry · Automation · Store    │
│ Control       │◀─────────────▶│ Surface · Diagnostics            │
│ plugins       │               └───────┬───────────┬──────────────┘
└──────┬────────┘                       │           │
       │ hardware/API                   │ legacy    │ future plans
       v                                v           v
┌───────────────┐              ┌──────────────┐  ┌──────────────────┐
│ Devices, OS,  │              │ OpenAction / │  │ Widget / Media / │
│ apps, cloud   │              │ Elgato host  │  │ Remote runtimes  │
└───────────────┘              └──────────────┘  └──────────────────┘
```

DeckWeave является локальным single-user продуктом. Cloud services могут использоваться плагинами, но master workspace и его runtime не требуют облачного аккаунта.

## Топология процессов

```text
deckweave-desktop.exe
├── Rust application core
├── tray + autostart lifecycle
├── Tauri WebView, создаваемый/скрываемый независимо от runtime
├── local API listener, только loopback
└── Plugin Supervisor
    ├── один process на активный native control package
    ├── openaction-host process tree, только при legacy actions
    ├── widget-host, только при custom web widgets
    └── media-host, только при future media graph

remote agent
└── отдельное paired приложение на Android / Steam Deck / другом ПК
```

В v1 нет system-wide daemon и многопользовательского сервера. «Host» — роль long-lived per-user desktop process, а не отдельная обязательная служба. Авария всего процесса останавливает runtime; crash WebView или закрытие окна — нет.

## Архитектурные слои

1. **Domain & Protocol.** ID, descriptors, schemas, envelopes, revisions и ошибки без OS/UI dependencies.
2. **Application Core.** Registry, draft/apply, automation, orchestration и query snapshots.
3. **Runtime Adapters.** Plugin supervisor, profile store, renderer, local API и diagnostics.
4. **Platform & Device Plugins.** Windows actions, N4 Pro, gamepad, keyboard, OBS, SprutHub и другие packages.
5. **Compatibility Runtimes.** OpenAction/Elgato, позже Companion и иные ecosystems.
6. **Specialized Data Planes.** Widgets, remote streaming и low-latency media.
7. **Presentation.** Tauri/Svelte editors, tray и CLI.

Зависимости направлены внутрь: domain не импортирует application, application не импортирует конкретный plugin и UI.

## Компоненты

| Компонент | Ответственность | Публичная граница |
|---|---|---|
| Domain & Protocol | Стабильные types, IDs, schemas и wire envelopes | `Descriptor`, `SignalEnvelope`, `CommandEnvelope` |
| Host Runtime | Startup, active revision, apply coordinator, shutdown | `HostCommand`, `HostSnapshot`, `ApplyReport` |
| Capability Registry | Nodes, devices, elements, ports, sessions и availability | `RegistrySnapshot`, `PortRef`, `Availability` |
| Plugin Supervisor | Index, permissions, spawn, handshake, health, restart | `PackageRecord`, `PluginSession`, `RuntimeLease` |
| Automation Engine | Validate/compile graph, state/layers, schedule и execution | `WorkspaceGraph`, `ExecutionPlan`, `ExecutionOutcome` |
| Profile Store | Drafts, immutable revisions, snapshots, assets, import/export | `ProfileRevision`, `DraftId`, `ExportSelection` |
| Surface Compositor | Scene graph, bindings, hit testing, raster plan и frame dispatch | `SurfaceScene`, `HitRegion`, `FrameIntent` |
| Widget Runtime | Sandboxed custom web widgets и event-driven snapshots | `WidgetInstance`, `WidgetFrame` |
| Local API | Commands, queries, subscriptions, auth и dry-run | `CommandApi`, `QueryApi`, `SubscriptionApi` |
| Compatibility Host | Legacy manifest/lifecycle/context translation | `LegacyContextMap`, `CompatibilityReport` |
| Platform Adapters | OS input/output, active app, virtual targets и privilege probes | `PlatformCapability`, `InputDecision` |
| Device Studio | Probe sessions, calibration, traces и descriptor projects | `ProbeSession`, `CaptureTrace` |
| Diagnostics & Test Lab | Metrics, structured logs, recording/replay, fixtures | `TraceId`, `ReplayReport` |
| Remote Agent Gateway | Pairing, capability advertisement и revocable grants | `RemoteNodeSession`, `RemoteGrant` |
| Media Runtime | Negotiated stream graph и real-time processing | `MediaGraphPlan`, `StreamEndpoint` |

Каждый компонент подробно описан в [модульных документах](README.md#модули).

## Доменная модель

```text
Host
└── Workspace
    ├── WorkspaceRevision (immutable when stored/applied)
    ├── Nodes
    │   ├── Local node
    │   ├── Remote node
    │   └── Virtual node
    ├── Devices / Software services
    │   └── Elements
    │       └── Capabilities
    │           └── Ports
    ├── AutomationGraph
    │   ├── Sources, transforms, conditions
    │   ├── state, layers, timers, schedules
    │   └── actions, targets, feedback
    ├── SurfaceScenes
    │   ├── visual nodes
    │   └── independent hit regions
    └── PluginBindings
        └── package contribution + instance settings + grant references
```

### Идентичность

- `PackageId` и `ContributionId` стабильны между совместимыми версиями package.
- `NodeId` сохраняется после pairing или между local starts.
- `DeviceIdentity` описывает физический экземпляр; OS path является только evidence.
- `DeviceSessionId` и `PluginSessionId` меняются при reconnect/restart.
- `ElementId` стабилен в пределах descriptor compatibility.
- `PortId` не содержит переводимый UI label.
- `BindingId` стабилен в workspace; legacy context остаётся временным.
- `RevisionId` является content-derived или однозначно связанным с неизменяемым документом.

### Инварианты

- Event старой session generation не изменяет active state.
- Edge event не coalesce; continuous value может coalesce только с drop metric.
- Отсутствующее/stale state не преобразуется в `false` или ноль.
- Никакой UI component не владеет hardware handle.
- Graph edge существует только между совместимыми schemas/units либо через явный converter.
- Синхронный cycle запрещён; cycle требует state/delay/latch node.
- Persistent provisioning не исполняется как побочный эффект обычного workspace apply.
- Secrets хранятся по reference и не входят в profile revision.
- Unsupported и temporarily unavailable являются разными состояниями.

## Draft, compile и apply

UI и API редактируют `WorkspaceDraft`. Active runtime никогда не исполняет mutable draft напрямую.

```text
Draft commands
    │ undo/redo
    v
WorkspaceDraft
    │ schema + reference + permission + cycle validation
    v
ValidatedGraph
    │ compile, resolve ports, precompute routes and policies
    v
ExecutionPlan + ActivationPlan
    │ prepare plugin sessions / assets / surfaces
    v
Atomic logical cutover to new Revision
    │
    ├── success: retire old resources
    └── failure: keep old Revision, emit ApplyReport
```

`ExecutionPlan` является immutable и не содержит UI geometry, кроме явно скомпилированных surface/touch transforms. `ActivationPlan` описывает resource diff: packages to start/stop, device leases, subscriptions и surface targets.

Apply имеет логическую транзакцию, но не притворяется аппаратной транзакцией. Reversible runtime changes подготавливаются до cutover. Необратимые provisioning actions запускаются отдельной подтверждаемой операцией с readback и собственным recovery report.

## Data flow

<a id="device-event--automation--feedback"></a>
### Device event → automation → feedback

1. Provider отправляет `SignalEnvelope` с source port, session generation, sequence, monotonic time и payload.
2. Transport проверяет размер/version; Registry проверяет активность source.
3. Edge попадает в bounded edge queue; latest-value update — в coalescing value slot.
4. Workspace scheduler обрабатывает сигнал относительно одного `ExecutionPlan` и одного логического clock.
5. Engine обновляет state/layers и создаёт ноль или несколько `CommandIntent`.
6. Independent branches запускаются параллельно; зависимые steps следуют explicit ordering/join policy.
7. Dispatcher проверяет target availability, permission, deadline, reentrancy и rate limit.
8. Provider возвращает accepted/completed/rejected/unknown; continuation снова проходит scheduler.
9. Feedback изменяет state, scene, light или haptic target.
10. UI получает throttled snapshot и trace; runtime не ждёт WebView.

Каждая цепочка сохраняет `trace_id`, `correlation_id`, `causation_id` и `origin`. Default loop guard блокирует повторное прохождение synthetic input к породившему его route.

### Изменение workspace

1. Client читает base revision и открывает draft.
2. Каждая mutation указывает expected draft revision.
3. Application service применяет command, сохраняет undo record и возвращает validation delta.
4. `validate` строит полный report без side effects.
5. `plan` рассчитывает activation diff и resource/permission requirements.
6. Пользователь или агент вызывает `apply` с expected base revision.
7. Host готовит новые sessions, assets и plan; затем делает logical cutover.
8. Profile Store атомарно сохраняет immutable revision и snapshot metadata.
9. При ошибке old plan продолжает работу; client получает точный failed stage.

### Plugin discovery и activation

1. Installer распаковывает artifact в staging, проверяет path traversal, hash, manifest и schemas.
2. Indexer сохраняет metadata без запуска executable.
3. Workspace compiler определяет требуемые contribution instances.
4. Supervisor сравнивает permissions и platform/architecture.
5. Host запускает один package process с one-time session token.
6. Plugin и host согласуют protocol range и descriptors.
7. После validation создаются runtime leases для contributions.
8. При отсутствии leases, subscribers и pending work process останавливается после grace period.

<a id="scene--hardware-frame"></a>
### Scene → hardware frame

1. State change invalidates только связанные scene nodes.
2. Compositor рассчитывает layout и dirty regions в logical coordinates.
3. Static/vector/text nodes рендерятся встроенным backend; custom widget даёт bounded bitmap/layer.
4. Scene преобразуется в preferred pixel format конкретного `FrameIn`.
5. Frame scheduler соблюдает max FPS и latest-frame-wins.
6. Provider подтверждает accepted frame и, если умеет, partial update.
7. Touch coordinate проходит обратную transform и hit test независимо от visual cell grid.

### Local API / Codex

1. Client проходит loopback authentication и читает capability/API version.
2. Query возвращает immutable snapshot с revision.
3. Mutation создаёт draft command с idempotency key и expected revision.
4. Client может вызвать validate, simulate и dry-run apply.
5. Apply требует явного вызова; опасные grants/provisioning требуют отдельного confirmation token.
6. Subscription передаёт deltas, health и operation progress.
7. MCP adapter только переводит tools/resources в эти операции и не читает внутренние файлы напрямую.

### Hardware provisioning

1. Provider публикует read-only `ProvisioningDescriptor` с supported operations и risk.
2. User/API создаёт `ProvisioningPlan` и получает diff текущего/целевого состояния.
3. Host проверяет dedicated permission, device identity и возможность readback.
4. Provider выполняет bounded steps; после каждого критичного шага проверяет result.
5. Итоговый readback и recovery instructions сохраняются в operation report.
6. Firmware flashing всегда отклоняется как unsupported.

### Remote и media

Remote pairing создаёт `NodeIdentity` и grants, а не автоматически открытые streams. Control events используют надёжный ordered transport; high-rate media получает отдельно negotiated endpoint. Core видит stream descriptors и health, но не копирует raw buffers через automation queues.

## Конкурентность и QoS

| Контур | Модель | Backpressure |
|---|---|---|
| Application commands | Сериализованный owner состояния host/draft | Bounded request queue, deadlines |
| Edge events | Упорядоченная workspace lane | Не coalesce; overflow = явная fault/degraded policy |
| Continuous values | Latest-value slots с sequence | Coalesce разрешён, drops измеряются |
| Async actions | Task execution вне scheduler; outcome возвращается message | Concurrency limit на provider/action |
| Surface frames | Per-surface scheduler | Latest-frame-wins, FPS budget |
| UI subscriptions | Throttled snapshots/deltas | Slow client пропускает промежуточные snapshots |
| Logs/traces | Bounded rotating storage | Sampling для high-frequency values |
| Audio/video | Отдельный media runtime | Ring/shared buffers по backend contract |

Automation scheduler не блокируется ожиданием network, filesystem или plugin action. Long-running step превращается в pending execution с cancellation/deadline.

## Ключевые интерфейсы

Интерфейсы показывают семантику, а не фиксируют окончательный Rust syntax.

```rust
pub trait ContributionRuntime {
    async fn activate(
        &mut self,
        request: ActivationRequest,
    ) -> Result<ActivationReply, PluginError>;

    async fn deactivate(
        &mut self,
        lease: RuntimeLeaseId,
        reason: StopReason,
    ) -> Result<(), PluginError>;
}

pub trait DeviceProvider {
    async fn discover(&self, scope: DiscoveryScope)
        -> Result<Vec<DeviceCandidate>, DeviceError>;

    async fn open(&self, request: OpenDeviceRequest)
        -> Result<DeviceSessionDescriptor, DeviceError>;
}

pub trait CommandTarget {
    fn input_schema(&self) -> SchemaRef;
    async fn invoke(&self, command: CommandEnvelope)
        -> Result<CommandOutcome, CommandError>;
}

pub trait FrameSink {
    fn descriptor(&self) -> SurfaceDescriptor;
    async fn submit(&self, frame: FramePacket)
        -> Result<FrameReceipt, FrameError>;
}

pub trait ApplicationService {
    async fn execute(&self, command: AppCommand)
        -> Result<OperationReceipt, AppError>;
    async fn query(&self, query: AppQuery)
        -> Result<AppSnapshot, AppError>;
}
```

Обязательные свойства:

- все mutation commands имеют idempotency key и expected revision;
- activation token не переиспользуется между plugin sessions;
- cancellation и deadline являются частью длительной операции;
- schemas versioned и согласуются при handshake;
- plugin не назначает global IDs самостоятельно;
- transport error не равен action outcome;
- descriptors доступны без hardware side effects;
- media buffers не реализуют `CommandTarget`.

## Plugin control protocol

Native control plugins используют versioned length-delimited Protocol Buffers по inherited stdio pipes. Это даёт code generation для Rust/TypeScript/Python, unknown-field evolution, бинарные limits и не требует слушающего TCP port. Message envelope поддерживает request/response, events, cancellation, heartbeat, session generation и flow-control notices.

Named pipes/Unix sockets могут быть добавлены для attachable runtimes, но не являются вторым доменным API. Legacy OpenAction продолжает использовать ожидаемый WebSocket/JSON protocol внутри compatibility host.

## Application API

Канонический контракт — application commands/queries, а не HTTP handlers и не Tauri commands. Он имеет адаптеры:

- direct in-process adapter для desktop shell;
- authenticated loopback HTTP/JSON + WebSocket subscriptions для CLI/Codex;
- optional MCP server adapter;
- test adapter с fake clock и in-memory store.

API не выдаёт raw plugin process handles и не позволяет обходить permission/apply workflow. Подробности: [API.md](API.md).

## Surface и widget architecture

Surface scene — retained tree:

- `Group`, `Transform`, `Clip`;
- `Shape`, `Image`, `Icon`, `Text`;
- `StateBinding` и conditional visibility;
- `Timeline` с ограниченным FPS;
- `WidgetLayer`, предоставляющий frame из optional widget runtime;
- независимые `HitRegion`: rectangle, rounded rectangle, polygon, path.

Logical scene не зависит от hardware pixel format. Device plugin объявляет resolution, color model, rotation, alpha, partial updates и max FPS. Initial renderer выбирается отдельным benchmark ADR; scene API не зависит от `wgpu`, Skia или CPU rasterizer.

Custom web widget не исполняется внутри основного UI WebView. Lazy widget host получает только scoped state и pointer events, а возвращает frame/accessibility metadata. Простые дата, время, погода, трек и notifications должны реализовываться стандартными scene nodes без отдельного browser process.

## System input и virtual outputs

Platform plugin объявляет фактические capabilities отдельно:

- observe per-device/system input;
- intercept и decision deadline;
- emit keyboard/mouse/media/gamepad;
- active application/window state;
- audio endpoint volume/mute;
- optional privileged helper status.

Core не предполагает, что observe означает suppress. Полный selective interception на Windows может потребовать optional signed helper/driver и отдельного security ADR. Его отсутствие не мешает specialized HID devices и ordinary actions.

## Remote nodes

Remote agent использует ту же capability model, но отдельный protocol family:

- mutual pairing и stable node identity;
- versioned capability advertisement;
- grants на display, touch, camera, microphone, speaker и sensors;
- heartbeat, reconnect и session generations;
- transport abstraction над Wi-Fi/ethernet/cable IP link;
- reliable control channel и negotiated media channels.

Первая реализация — widget surface + touch coordinates. Window stream и virtual monitor не смешиваются: virtual monitor требует OS display adapter/driver и отдельного target capability.

## Media runtime

Media runtime является зарезервированным специализированным process:

- graph sources/filters/sinks и format negotiation;
- real-time threads без allocation, blocking locks и JSON;
- control bridge для parameters, meters и health;
- bypass/stop policy на failure;
- OS adapters для capture/playback и virtual endpoints;
- compatibility bridges к CLAP и, при необходимости, GStreamer/VST3/AU;
- latency budget и per-node measurement.

Python, web widgets и ordinary control plugins никогда не исполняются в audio callback. Архитектурная граница определена сейчас, но production DSP не входит в v1.

## Целевая структура репозитория

```text
DeckWeave/
├── Cargo.toml
├── apps/
│   ├── desktop/                 # Tauri shell + per-user host
│   └── cli/                     # local API client
├── crates/
│   ├── deckweave-domain/
│   ├── deckweave-protocol/
│   ├── deckweave-host/
│   ├── deckweave-registry/
│   ├── deckweave-automation/
│   ├── deckweave-profiles/
│   ├── deckweave-surface/
│   ├── deckweave-plugin-supervisor/
│   ├── deckweave-api/
│   └── deckweave-testkit/
├── runtimes/
│   ├── openaction-host/
│   ├── widget-host/
│   └── media-host/              # reserved until its roadmap phase
├── plugins/
│   ├── device-n4pro/
│   ├── device-gamepad/
│   ├── platform-windows/
│   └── virtual-test/
├── sdks/
│   ├── rust/
│   ├── typescript/
│   └── python/                  # after protocol stabilization
├── protocol/
│   ├── proto/
│   ├── manifest.schema.json
│   └── settings.schema.json
├── ui/
│   └── desktop/
├── modules/                     # implementation context documents
├── docs/
├── fixtures/
└── tests/
```

Не все каталоги создаются в Фазе 1. Reserved runtimes появляются только при начале соответствующей фазы.

## Технологические решения

| Решение | Выбор | Обоснование |
|---|---|---|
| Форма системы | Modular monolith + isolated plugin processes | Один источник истины без микросервисной сложности |
| Core | Rust stable + Tokio | Memory safety, async I/O, portable libraries |
| Desktop | Tauri 2 + Svelte/TypeScript | Компактный shell и сильный визуальный editor |
| Native plugin protocol | Protocol Buffers over length-delimited stdio | Multi-language codegen, evolution и отсутствие local ports |
| Settings | JSON Schema + localized UI metadata | Generated forms и language-neutral validation |
| Legacy protocol | OpenAction/Elgato WebSocket/JSON | Совместимость с существующей экосистемой |
| Profiles | Versioned JSON documents + content-addressed assets | Прозрачность, export, diff и recovery |
| Internal index | In-memory; optional rebuildable SQLite cache только по измерению | Не делать БД источником истины без потребности |
| Secrets | OS credential store references | Не хранить tokens в profiles |
| Automation | Compiled immutable plan + deterministic scheduler | Predictability, replay, hot path performance |
| Rendering | Backend-neutral retained scene; backend выбирается spike | Не связывать contract с одним GPU/CPU stack |
| External API | Loopback HTTP/JSON + WebSocket поверх application service | Удобно CLI, Codex и разным языкам |
| Media | Отдельный native runtime/data plane | Low latency и isolation |
| Лицензия | GPL-3.0-or-later app/core; MIT OR Apache-2.0 protocol/SDK/examples | Copyleft продукта при низком барьере для plugins |

## Бюджеты производительности

Это engineering targets для benchmark, а не обещание на любом hardware.

| Путь | Начальная цель |
|---|---|
| Idle host, без active third-party runtimes | Запущен только desktop host; RSS фиксируется baseline test |
| Digital input → command dispatch | p95 ≤ 10 ms, p99 ≤ 20 ms без external action latency |
| Encoder/axis update | Latest value не старше одного configured frame interval |
| UI snapshot | Не чаще configured throttle; slow UI не влияет на engine |
| N4 Pro frame | Device max FPS; очередь не длиннее current + latest |
| Plugin activation | Не блокирует host startup; deadline и progress |
| Apply | Old revision работает до cutover; validation не блокирует UI thread |
| Audio callback, future | Backend-specific buffer deadline; zero blocking/allocation in callback |

Числа пересматриваются только по benchmark evidence и фиксируются ADR.

## Режимы отказа

| Сбой | Влияние | Митигация |
|---|---|---|
| Plugin process crash | Его ports unavailable | Session invalidation, bounded restart/backoff, circuit breaker |
| Plugin hang | Pending actions задержаны | Deadline, cancel, health probe, terminate только package process |
| USB/Bluetooth reconnect | Временная недоступность | Stable identity, new session generation, descriptor revalidation |
| Edge queue overflow | Возможна потеря семантики | Не молчать: fault/degraded state, trace, configurable safe action |
| Value flood | Рост latency | Latest-value coalescing и drop metrics |
| Slow renderer/sink | Устаревшие frames | Dirty regions, FPS limit, latest-frame-wins |
| Invalid/stale profile | Apply невозможен | Pure migration, backup, validation report, old revision remains active |
| Missing secret | Contribution не активируется | `NeedsAttention`; остальной workspace продолжает работу |
| Property editor/widget crash | Settings/visual layer unavailable | Separate process/context, restart, fallback representation |
| Local API client lost | Operation status неизвестен client | Idempotency key и operation query |
| Host crash | Все local runtime останавливаются | Atomic profiles, startup recovery, safe-output cleanup where supported |
| Remote disconnect | Remote ports unavailable | Cancel session operations, optional fallback routes |
| Media filter crash | Stream degraded/stopped | Separate media host, bypass/stop policy, never corrupted continuation |
| Privileged input helper unavailable | Suppression/emission недоступны | Downgrade to observe, explicit diagnostics, safe OS input remains |

## Границы безопасности

- Installed plugin считается выбранным пользователем, но не автоматически безопасным.
- Process isolation — crash boundary, не обещание защиты от злонамеренного executable.
- Manifest permissions и actual grants хранятся раздельно.
- USB/HID, network, filesystem, process, input, screen, camera, microphone, secrets и provisioning имеют отдельные scopes.
- Custom property editors/widgets не получают privileged Tauri commands.
- Local API слушает только loopback, использует session/token auth и CSRF/origin protections.
- Remote agent требует mutual pairing и revocable grants.
- Native media code исполняется только в media host и считается high-trust.
- Логи проходят redaction; raw input capture включается отдельной диагностической сессией.

Подробности: [SECURITY.md](SECURITY.md).

## Правила эволюции

1. Новый standard capability добавляется после двух независимых providers либо общеупотребимого стандарта.
2. Device-specific field остаётся namespaced extension.
3. Breaking wire change создаёт новую major protocol version; host может поддерживать несколько соседних majors через adapters.
4. Profile migrations являются pure transformations и всегда создают report/backup.
5. Unknown optional fields сохраняются round-trip.
6. Новый runtime class требует ADR; contribution type сам по себе не требует отдельного процесса.
7. Heavy dependency не добавляется в desktop baseline, если может быть optional runtime/plugin.
8. UI feature не получает приватный путь к core; сначала расширяется application service.
9. Compatibility claim публикуется только с fixture и version matrix.
10. Необратимая hardware operation не становится обычным automation action без отдельного risk design.

## Вне области видимости v1

- system service и одновременное обслуживание нескольких OS users;
- собственный kernel/filter driver и полный selective interception всех клавиатур;
- firmware flashing;
- force-feedback engine для рулей;
- публичный marketplace с автоматическим доверием;
- Bitfocus Companion bridge;
- Android/Steam Deck camera, microphone и full-screen streaming;
- virtual display driver;
- production audio DSP host, virtual microphone и CLAP/VST bridges;
- облачная синхронизация;
- обещание совместимости со всеми закрытыми Elgato plugins;
- выполнение untrusted plugins в доказуемой OS sandbox;
- MCP как внутренний transport.

Эти пункты отложены, но для них определены ports, runtime boundaries и permission vocabulary. Реализация каждого начинается с отдельного ADR и acceptance test, а не с расширения core «на всякий случай».
