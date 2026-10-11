# Архитектура DeckWeave

## Цель архитектуры

DeckWeave должен начать с N4 Pro и обычных системных действий, но не встраивать их особенности в ядро. Стабильная часть продукта — идентичность, capability vocabulary, revisions, automation semantics и application API. Всё, что зависит от конкретного hardware, ОС, чужой экосистемы, web runtime или real-time media, находится за заменяемой границей.

Архитектура оптимизируется не под максимальное число функций в первом релизе, а под дешёвое добавление нового provider без изменения уже работающих контрактов.

Нормативный порядок apply/recovery, execution epochs, signals, leases и frames задан в [Lifecycle](../contracts/lifecycle.md). При реализации этот документ читается вместе с модульной спецификацией.

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
    ├── один process на активную runtime group native control package
    ├── openaction-host process tree, только при legacy actions
    ├── widget-host, только при custom web widgets
    └── media-host, только при future media graph

remote agent
└── отдельное paired приложение на Android / Steam Deck / другом ПК
```

В v1 нет system-wide daemon и многопользовательского сервера. «Host» — роль long-lived per-user desktop process, а не отдельная обязательная служба. Авария всего процесса останавливает runtime; crash WebView или закрытие окна — нет.

В v1 разрешён один host на пользовательскую сессию и один active workspace; другие workspaces можно хранить и редактировать. Single-instance и data-root writer locks приобретаются до запуска API/plugins. См. [правила владения](../contracts/lifecycle.md#владение-host-и-workspace).

## Архитектурные слои

### Пути исполнения

Общий capability graph описывает связи, но не заставляет все данные исполняться одним механизмом:

| Путь | Владелец | Работа |
|---|---|---|
| Events/state | Automation scheduler | Быстрые transitions, layers, timers и semantic intents |
| Workflow/results | Automation + async dispatcher; artifact broker в Host | Typed jobs, run-local результаты, bounded большие данные |
| Visual output | Surface Compositor | Display/light scenes, geometry, animation и единый output owner |
| Media | Optional media-host/backend | Negotiated realtime buffers, clocks и DSP |

Общие границы — IDs, grants, revisions, descriptor versions, resource leases и lifecycle. Это не четыре обязательных новых процесса: workflow является частью Automation Engine, lighting — частью Surface Compositor. Детальные контракты: [automation](../contracts/automation.md), [lighting](../contracts/lighting.md), [media](../contracts/media.md); мотивировка — [ADR-0014](../decisions/0014-specialized-execution-and-results.md).

### Границы компонентов

1. **Shared Kernel & Protocol.** Общие ID, descriptors, schemas и envelopes без OS/UI dependencies; это не владелец всех domain models.
2. **Bounded Contexts.** Workspace Configuration, Host Lifecycle, Application Gateway, Automation, Registry, Plugin Runtime, Surface, Platform Input и Interaction UI.
3. **Runtime Adapters.** Plugin supervisor, profile store, renderer, local API и diagnostics.
4. **Platform & Device Plugins.** Windows actions, N4 Pro, gamepad, keyboard, OBS, SprutHub и другие packages.
5. **Compatibility Runtimes.** OpenAction/Elgato, позже Companion и иные ecosystems.
6. **Specialized Data Planes.** Widgets, remote streaming и low-latency media.
7. **Presentation.** Tauri/Svelte editors, tray и CLI.

Зависимости направлены внутрь: domain не импортирует application, application не импортирует конкретный plugin и UI.

### DDD context map

DeckWeave является модульным монолитом с вертикальными bounded contexts. Workspace Configuration владеет draft/revisions; Host Lifecycle — active selection/apply/epochs; Application Gateway — authorization, routing и operation tracking; Platform Input — системными источниками ввода; Interaction UI — короткими host-owned диалогами с typed result. Ни один adapter не становится владельцем бизнес-инварианта.

Нормативная карта, Shared Kernel policy, ограничения кода и поэтапная миграция находятся в [DDD и границы контекстов](ddd.md). Модуль может жить в существующем crate во время миграции, но его dependency direction уже должна соответствовать целевой границе.

### Направление зависимостей

Стрелка означает compile dependency, а не поток runtime messages:

```text
SDK ───────────────> protocol (permissive, без GPL dependencies)
shared domain ─────> protocol
bounded-context application ──> собственный domain + shared kernel
diagnostics ───────> domain/protocol (без зависимости от runtimes)
registry/automation/profiles/surface ──> domain + diagnostics/узкие ports
host ──────────────> application + реализации core компонентов
api adapters ──────> application (без зависимости от host implementation)
desktop composition ──> host + api + platform-host + runtime adapters
testkit/tests ─────> тестируемые компоненты (только test/dev направление)
```

Core получает OS services через injected ports. `platform-host` реализует locks, data roots, credentials, autostart и process cleanup; пользовательские input/volume/window actions остаются isolated plugins. Store не импортирует keychain, Surface не импортирует widget-host, production code не импортирует testkit. Подробнее: [ADR-0013](../decisions/0013-dependency-and-host-service-boundaries.md).

## Компоненты

| Компонент | Ответственность | Публичная граница |
|---|---|---|
| Shared Kernel & Protocol | Стабильные общие types, IDs, schemas и wire envelopes | `Descriptor`, `SignalEnvelope`, `CommandEnvelope` |
| Workspace Configuration | Draft aggregate, revisions, snapshots и переносимость profile | `WorkspaceDraft`, `DraftCommand`, `WorkspaceRepository` |
| Host Runtime | Startup, active revision, apply, shutdown и scoped runtime artifacts | `HostCommand`, `HostSnapshot`, `ApplyReport`, `ArtifactBroker` |
| Capability Registry | Nodes, devices, elements, ports, sessions и availability | `RegistrySnapshot`, `PortRef`, `Availability` |
| Plugin Supervisor | Index, permissions, spawn, handshake, health, restart | `PackageRecord`, `PluginSession`, `RuntimeLease` |
| Automation Engine | Compile graph, state/layers, schedule, typed jobs и results | `WorkspaceGraph`, `ExecutionPlan`, `Invocation`, `ExecutionOutcome` |
| Profile Store | Infrastructure persistence для Workspace Configuration | `ProfileStoreAdapter`, atomic document/assets ports |
| Surface Compositor | Display/light scenes, geometry, hit testing и output arbitration | `SurfaceScene`, `LightScene`, `HitRegion`, `FrameIntent` |
| Widget Runtime | Sandboxed custom web widgets и event-driven snapshots | `WidgetInstance`, `WidgetFrame` |
| Interaction Runtime | Host-owned palettes/dialogs и typed user selection | `InteractionSession`, `InteractionRequest`, `InteractionResult` |
| Local API | Commands, queries, subscriptions, auth и dry-run | `CommandApi`, `QueryApi`, `SubscriptionApi` |
| Compatibility Host | Legacy manifest/lifecycle/context translation | `LegacyContextMap`, `CompatibilityReport` |
| Platform Adapters | OS input/output, active app, virtual targets и privilege probes | `PlatformCapability`, `InputDecision` |
| Platform Input | Bundled keyboard/mouse observation, interception и synthetic output | `InputSourceDescriptor`, `InputObservation`, `SyntheticInput` |
| Device Studio | Probe sessions, calibration, traces и descriptor projects | `ProbeSession`, `CaptureTrace` |
| Diagnostics / Test Lab | Production instrumentation отдельно от replay fixtures/fakes/conformance; production replay исполняет Automation | `TraceId`, `ReplayReport` |
| Host Platform Services | Locks, data roots, credentials, autostart, process cleanup | `HostPlatform`, `CredentialBroker` |
| Remote Agent Gateway | Pairing, capability advertisement и revocable grants | `RemoteNodeSession`, `RemoteGrant` |
| Media Runtime | Negotiated stream graph и real-time processing | `MediaGraphPlan`, `StreamEndpoint` |

Каждый компонент подробно описан в [модульных документах](modules/README.md).

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
    │   └── actions, typed results/jobs, targets, feedback
    ├── SurfaceScenes
    │   ├── visual nodes
    │   └── independent hit regions
    ├── LightScenes (geometry, waveforms, reactive envelopes)
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
- `WorkspaceRevisionId` однозначно связан с неизменяемым документом и checksum; rollback создаёт новый ID.
- `HostBootId`/`ActivationEpoch` отделяют исполнения даже при повторной активации той же revision.

### Инварианты

- Event старой session generation не изменяет active state.
- Edge event не coalesce; continuous value может coalesce только с drop metric.
- Отсутствующее/stale state не преобразуется в `false` или ноль.
- Никакой UI component не владеет hardware handle.
- Graph edge существует только между совместимыми schemas/units либо через явный converter.
- Синхронный cycle запрещён; cycle требует положительного delay или ожидания нового внешнего события и execution budget.
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
    │ persist candidate + prepare resources без новых outputs
    v
Quiesce / drain / revalidate → durable commit record → epoch cutover
    │
    ├── до commit: прежняя revision; report о failed prepare/handoff
    └── после commit: новая revision; applied/degraded/recovery report
```

`ExecutionPlan` является immutable и не содержит UI geometry, кроме явно скомпилированных surface/touch transforms. `ActivationPlan` описывает resource diff: packages to start/stop, device leases, subscriptions и surface targets.

Physical/light geometry не является UI geometry: plan фиксирует необходимые descriptor revisions и visual plan references. При изменении topology Registry переоценивает затронутые routes, не перенумеровывает bindings. Временные audio/text artifacts принадлежат run, не WorkspaceRevision; Host broker предоставляет scoped access через injected storage port.

Apply имеет логическую транзакцию, но не является аппаратной транзакцией. Exclusive provider handoff может требовать разрешённого downtime; после durable commit возврат выполняется новым apply. Pending actions, held outputs и late completions обрабатываются по [контракту lifecycle](../contracts/lifecycle.md#executions-cancellation-и-смена-плана). Provisioning запускается отдельной подтверждаемой операцией с readback и собственным recovery report.

## Data flow

<a id="device-event--automation--feedback"></a>
### Device event → automation → feedback

1. Provider отправляет `SignalEnvelope` с source port, session generation, sequence, monotonic time и payload.
2. Transport проверяет размер/version; Registry проверяет активность source.
3. Host присваивает ingress ordinal/epoch; edge и latest-value lane сохраняют порядок относительно barriers по [контракту сигналов](../contracts/lifecycle.md#порядок-сигналов-и-время).
4. Workspace scheduler обрабатывает сигнал относительно одного `ExecutionPlan` и одного логического clock.
5. Engine обновляет state/layers и создаёт ноль или несколько `CommandIntent`.
6. Independent branches запускаются параллельно; зависимые steps следуют explicit ordering/join policy.
7. Dispatcher проверяет target availability, permission, deadline, reentrancy и rate limit.
8. Provider возвращает rejection, terminal typed result либо accepted job; progress и terminal outcome коррелируются с invocation. Continuation после completion снова проходит scheduler.
9. Feedback изменяет state, scene, light или haptic target.
10. UI получает throttled snapshot и trace; runtime не ждёт WebView.

Каждая цепочка сохраняет `trace_id`, `correlation_id`, `causation_id` и `origin`. Default loop guard блокирует повторное прохождение synthetic input к породившему его route.

### Изменение workspace

1. Client читает base revision и открывает draft.
2. Каждая mutation указывает expected draft revision.
3. Application service применяет command, сохраняет undo record и возвращает validation delta.
4. `validate` строит полный report без side effects.
5. `plan` рассчитывает activation diff и resource/permission requirements.
6. Пользователь или агент вызывает `apply` с expected active revision, draft version и plan hash.
7. Store сохраняет candidate revision/assets/package lock; Host готовит sessions и resources без исполнения новых actions.
8. На cutover barrier Host повторяет проверки, Store публикует durable commit record, затем Host переключает epoch/plan.
9. До commit ошибка сохраняет прежнюю выбранную revision; после commit новая остаётся выбранной с явным health/recovery report. Порядок и crash matrix: [контракт apply](../contracts/lifecycle.md#сохранение-и-apply).

### Plugin discovery и activation

1. Installer распаковывает artifact в staging, проверяет path traversal, hash, manifest и schemas.
2. Indexer сохраняет metadata без запуска executable.
3. Workspace compiler определяет contribution instances; разрешённый discovery/probe lease также может запустить provider до появления первого binding.
4. Supervisor сравнивает permissions и platform/architecture.
5. Host запускает process для runtime group зафиксированного artifact с one-time session token.
6. Plugin и host согласуют protocol range и descriptors.
7. После validation создаются runtime leases для contributions.
8. При отсутствии leases, subscribers и pending work process останавливается после grace period.

<a id="scene--hardware-frame"></a>
### Scene → hardware frame

1. State change invalidates только связанные scene nodes.
2. Compositor рассчитывает layout и dirty regions в logical coordinates.
3. Static/vector/text nodes рендерятся встроенным backend; custom widget даёт bounded bitmap/layer.
4. Scene преобразуется в preferred pixel format конкретного `FrameIn`.
5. Frame scheduler соблюдает max FPS; latest-frame-wins применяется к полному desired frame, diff требует подтверждённой базы.
6. Provider различает accepted и applied receipt; потеря base/ACK вызывает full-frame resync по [контракту frames](../contracts/lifecycle.md#frames-и-preview).
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
| Continuous values | Latest-value segments с sequence/ingress order | Coalesce до следующего edge/barrier, drops измеряются |
| Async actions | Task execution вне scheduler; outcome возвращается message | Concurrency limit на provider/action |
| Surface frames | Per-surface scheduler | Latest full-frame wins; diff только от подтверждённой базы |
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
    fn descriptor(&self) -> OperationDescriptor; // input/result/error, limits, policies
    async fn invoke(&self, command: CommandEnvelope)
        -> Result<InvocationReply, CommandError>; // terminal result или accepted JobId
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

- mutation commands имеют idempotency key и command-specific preconditions; apply проверяет draft и active revision отдельно;
- activation token не переиспользуется между plugin sessions;
- cancellation и deadline являются частью длительной операции;
- schemas versioned и согласуются при handshake;
- plugin не назначает global IDs самостоятельно;
- transport error не равен action outcome;
- descriptors доступны без hardware side effects;
- media buffers не реализуют `CommandTarget`.

Это иллюстрация service boundaries, не готовый SDK/ABI. Completion/progress долгих jobs проходят versioned envelopes по [automation contract](../contracts/automation.md), а не удерживают async call бесконечно. Необходимые extensions согласуются до активации contribution.

## Plugin control protocol

Native control plugins используют versioned length-delimited Protocol Buffers по inherited stdio pipes. Это даёт code generation для Rust/TypeScript/Python, additive wire evolution, бинарные limits и не требует слушающего TCP port. Message envelope поддерживает request/response, events, cancellation, heartbeat, session generation и flow-control notices. Round-trip unknown fields проверяется для конкретных SDK; typed decode/re-encode не считается transparent relay автоматически.

Named pipes/Unix sockets могут быть добавлены для attachable runtimes, но не являются вторым доменным API. Legacy OpenAction продолжает использовать ожидаемый WebSocket/JSON protocol внутри compatibility host.

## Application API

Канонический контракт — application commands/queries, а не HTTP handlers и не Tauri commands. Он имеет адаптеры:

- direct in-process adapter для desktop shell;
- authenticated loopback HTTP/JSON + WebSocket subscriptions для CLI/Codex;
- optional MCP server adapter;
- test adapter с fake clock и in-memory store.

API не выдаёт raw plugin process handles и не позволяет обходить permission/apply workflow. Подробности: [Application API](../contracts/application-api.md).

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

LightScene использует тот же visual ownership/frame scheduling, но собственную geometry и renderer: input matrix, key positions, LED mapping и surface pixels независимы. Engine меняет parameters/envelopes, compositor формирует LightFrame. Host/onboard mode, waveform, cooldown/fade и conflict policies заданы в [lighting contract](../contracts/lighting.md). Notification widget читает bounded view model/query results, не хранит всю историю сервиса.

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

Python, web widgets и ordinary control plugins никогда не исполняются в audio callback. Архитектурная граница определена сейчас, но production DSP не входит в v1. Buffer ownership, clock mapping, requestable ports, reconfigure и пределы bypass заданы в [media contract](../contracts/media.md). Конечная запись WAV для transcription может выполняться отдельным capture job без внедрения полноценного DSP graph.

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
│   ├── deckweave-application/     # commands/queries и service ports
│   ├── deckweave-host/
│   ├── deckweave-platform-host/   # locks, credentials, autostart
│   ├── deckweave-registry/
│   ├── deckweave-automation/
│   ├── deckweave-profiles/
│   ├── deckweave-surface/
│   ├── deckweave-plugin-supervisor/
│   ├── deckweave-api/
│   ├── deckweave-diagnostics/     # production instrumentation
│   ├── deckweave-device-studio/   # diagnostic application service
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
├── docs/                        # product, architecture/modules, contracts,
│                                # implementation, decisions, research, archive
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
| Axis/continuous value | Latest value не старше одного configured frame interval |
| Encoder delta | Все edges в порядке ingress; latency/overflow измеряются отдельно |
| UI snapshot | Не чаще configured throttle; slow UI не влияет на engine |
| N4 Pro frame | Device max FPS; очередь не длиннее current + latest |
| Plugin activation | Не блокирует host startup; deadline и progress |
| Apply | Old revision работает до barrier; disruptive handoff объявляет downtime; validation вне UI thread |
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
| Media filter crash | Stream degraded/stopped | Separate media host; bypass только при живом совместимом пути, иначе silence/stop |
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

Подробности: [Безопасность](../contracts/security.md).

## Правила эволюции

1. Новый standard capability добавляется после двух независимых providers либо общеупотребимого стандарта.
2. Device-specific field остаётся namespaced extension.
3. Breaking wire change создаёт новую major protocol version; host может поддерживать несколько соседних majors через adapters.
4. Profile migrations являются pure transformations с report/backup; package migrations v1 декларативны и не запускают executable.
5. Unknown optional document/settings fields сохраняются round-trip; неизвестная обязательная семантика блокирует execution, wire relay сохраняет opaque bytes.
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
- production Handy/capture/browser/mail/Telegram packages и произвольный onboard program compiler; v1 проверяет общие contracts на fixtures;
- облачная синхронизация;
- обещание совместимости со всеми закрытыми Elgato plugins;
- выполнение untrusted plugins в доказуемой OS sandbox;
- MCP как внутренний transport.

Эти пункты отложены, но для них определены ports, runtime boundaries и permission vocabulary. Реализация каждого начинается с отдельного ADR и acceptance test, а не с расширения core «на всякий случай».
