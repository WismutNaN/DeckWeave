# Модель плагинов DeckWeave

DeckWeave использует единый package/manifest format, но несколько runtime classes. Package может одновременно предоставить device, actions, state и UI settings, однако это не означает отдельный процесс на каждую contribution или instance.

## Цели

- добавлять hardware и integrations без изменения core;
- писать control plugins на Rust, TypeScript и Python;
- запускать только используемые packages;
- локализовать crash/hang;
- не заставлять audio/video buffers проходить через control IPC;
- поддерживать custom settings/widget UI без privileged доступа к host;
- позволить существующим ecosystems подключаться bridge-плагинами;
- предоставить conformance suite, чтобы качество package было измеряемым.

## Термины

- **Artifact** — immutable archive/release с hash и provenance.
- **Package** — установленная версия artifact с manifest, binaries/scripts, assets и schemas.
- **Contribution** — предоставляемая функция package.
- **Instance** — настроенное использование contribution в workspace.
- **Runtime** — процесс или specialized host, исполняющий package.
- **Session** — один handshake конкретного runtime process.
- **Lease** — причина удерживать contribution/runtime активным.
- **Grant** — разрешение пользователя с resource scope.

## Contribution types

| Type | Ответственность |
|---|---|
| `DeviceProvider` | Discovery/open устройств, input events, output commands, provisioning descriptors |
| `ActionProvider` | Typed commands: OS, apps, HTTP, OBS, SprutHub, scripts |
| `StateProvider` | Versioned states: active window, media, tasks, notifications |
| `AutomationProvider` | Custom transform/condition/action nodes без изменения engine |
| `RendererProvider` | Declarative scene nodes/templates или custom widget contribution |
| `ProtocolBridge` | OpenAction/Elgato, позже Companion/MIDI/OSC и иные ecosystems |
| `MediaNodeProvider` | Sources/filters/sinks только для specialized media runtime |
| `RemoteNodeProvider` | Discovery/pairing remote agents и capability projection |
| `PlatformProvider` | OS-specific input/output, active app, virtual devices; credentials только через host broker |

Package может объявить несколько contributions. Runtime class определяется entrypoint/contribution group, а не названием package.

## Manifest

Manifest читается и валидируется без запуска executable.

| Поле | Назначение |
|---|---|
| `package_id` | Reverse-DNS stable ID |
| `version` | SemVer artifact |
| `manifest_version` | Version manifest schema |
| `license` | SPDX expression |
| `source` | Repository/revision/provenance metadata |
| `artifacts` | OS/architecture entrypoints и hashes |
| `runtime_groups` | Какие contributions разделяют process/runtime |
| `contributions` | Static descriptors и activation conditions |
| `protocol_range` | Поддерживаемые control protocol versions |
| `permissions` | Requested scopes с объяснениями |
| `schemas` | Versioned settings/ports/results schemas |
| `assets` | Icons, localization, templates, property editors/widgets |
| `dependencies` | External runtime/system dependencies |
| `migrations` | Ограниченные декларативные settings transforms, исполняемые host без plugin code |

Manifest не содержит secrets, install scripts и произвольные команды определения совместимости. Dynamic discovery разрешена только внутри заранее объявленной contribution.

## Runtime classes

| Class | Transport | Что исполняется | Trust |
|---|---|---|---|
| Isolated control | Length-delimited Protobuf over stdio | Devices, actions, states, automation | User-trusted executable, crash-isolated |
| OpenAction compatibility | Local WebSocket/JSON внутри process tree | Unmodified legacy packages и PI | Legacy permissions, отдельный host |
| Widget | Scoped messages + rendered frames | Custom web widget/property UI | Web sandbox + process isolation |
| Trusted media | Native ABI/shared/ring buffers | Audio/video nodes | High-trust native code |
| Remote agent | Authenticated network control + negotiated streams | Remote capabilities | Paired remote trust |
| Built-in audited | Direct Rust traits | Только базовая plumbing/test infrastructure | Core review |

WASM может появиться как дополнительный control runtime после доказанной потребности; модель не требует его для v1.

## Один процесс на package

Default: один процесс на активную `runtime_group` package. Несколько instances и contributions multiplexed внутри session.

Ключ runtime включает package ID, artifact hash и group. В v1 одновременно активна одна version package на host; exact version/hash записывается в revision package lock. Установка новой версии не меняет active lock. Порядок обновления и exclusive handoff описаны в [контрактах resources](RUNTIME_CONTRACTS.md#resources-discovery-и-версии-packages).

Преимущества:

- меньше baseline памяти, чем process per action;
- crash остаётся локализован package;
- общие network/device connections могут переиспользоваться;
- update/restart имеет одну observable boundary.

Исключения:

- package сам объявляет несколько runtime groups с разными permissions;
- compatibility host создаёт ожидаемое legacy process tree;
- media node работает в media host, а control contribution того же package — в control process;
- policy может изолировать конкретный unstable instance в отдельный process в будущем.

## Native control protocol

Wire contract: length-delimited Protocol Buffers через inherited stdin/stdout. Stderr зарезервирован для bounded diagnostic logs.

Protocol envelope поддерживает:

- hello/handshake и version negotiation;
- package/session identity и one-time token;
- request/response IDs;
- events и state snapshots;
- activation/deactivation leases;
- cancellation и deadlines;
- heartbeat/health;
- session generation;
- backpressure notices;
- structured errors;
- schema references;
- chunked bounded assets/metadata и `FramePacket` для `FrameIn`, но не audio/video streams.

Host применяет hard limits на frame/message size, nesting, concurrent requests и startup time. Wire compatibility не гарантирует unknown-field round-trip в каждой codegen library: endpoint может пропустить optional metadata, relay сохраняет opaque bytes, JSON settings сохраняют unknown keys отдельно. Unknown required command/feature отклоняется; обязательные features согласуются handshake, semantic breaking changes требуют новой major. Это проверяется на Rust/TS/Python fixtures, включая enum/presence/unknown fields.

Frame chunks не занимают неограниченный участок stdio очереди: control/heartbeat/cancel имеют бюджет задержки и scheduling между bounded chunks. Partial frames передаются с base frame ID; потеря ACK требует full resync ([контракт frames](RUNTIME_CONTRACTS.md#frames-и-preview)).

## Lifecycle

```text
Discovered
   │ manifest/hash/schema validation
   v
Indexed ── user disables ──> Disabled
   │ compiled workspace requires contribution
   v
PermissionCheck ── missing ──> NeedsAttention
   │ granted
   v
Starting ── handshake ──> Active
   │ timeout/error          │ no leases after grace period
   v                        v
Failed <── backoff ─── Stopping ──> Indexed
   │ repeated
   v
CircuitOpen / Disabled
```

Правила:

- discovery/index не запускают package;
- settings import не активирует runtime;
- Active наступает только после handshake и descriptor validation;
- restart всегда создаёт новую session generation;
- late messages старой session отбрасываются;
- stop имеет deadline, затем host завершает только plugin process tree;
- crash loop открывает circuit и требует user action;
- active hardware/resource lease предотвращает idle stop.
- persisted discovery subscription также удерживает runtime; user disable прекращает её и не возобновляется автоматически.

## Activation

Runtime запускается, когда:

- contribution присутствует в active `ExecutionPlan`;
- пользователь включил scoped discovery lease, чтобы обнаружить device до первого binding;
- открыта Device Studio probe session;
- legacy action появился на active surface;
- открыт custom property editor, которому нужен runtime;
- external trigger имеет заранее разрешённую subscription;
- пользователь явно запускает diagnostics.

Activation condition не может самостоятельно расширять permissions. Runtime может остановиться после grace period, если leases, subscribers и pending operations отсутствуют.

Static index никогда не вызывает executable для discover/validate/migrate. Runtime discovery является отдельной разрешённой операцией; найденный candidate не даёт exclusive/output permission. `prepare` обязан отличаться от запуска пользовательских effects; не умеющий это provider объявляет disruptive handoff. RuntimeLease удерживает процесс, ResourceLease выдаётся Registry на canonical physical resource — probe не обходит этот механизм.

## Settings и UI contributions

Порядок предпочтения:

1. native form из JSON Schema + DeckWeave UI hints;
2. package preset/template;
3. isolated custom property editor;
4. legacy OpenAction/Elgato property inspector;
5. generic JSON editor только для diagnostics.

Editor всегда работает с draft. Live preview использует scoped lease, deadline и восстановление текущей applied scene; apply отзывает конфликтующий preview. Custom editor не получает Tauri commands, filesystem или arbitrary local API token. Dry-run/simulation не открывают этот канал и используют fake targets.

Custom widget отличается от property editor: widget создаёт visual layer для output surface и может получать только явно связанные state/interaction ports.

## Permissions

| Permission | Пример scope |
|---|---|
| `device.usb.open` | VID/PID/interface/serial selector |
| `device.bluetooth.observe` | service/device selector |
| `device.open.exclusive` | конкретный interface |
| `device.provision` | model/identity/operation set |
| `input.observe` | selected devices/system-wide |
| `input.intercept` | selected devices, privileged status |
| `input.emit` | keyboard/mouse/media/gamepad targets |
| `display.write` | surface IDs |
| `light.write` | target unit IDs |
| `haptic.write` | actuator IDs |
| `network.client` | domains/ports/protocols |
| `network.listen` | loopback/interface/port policy |
| `process.spawn` | executable selectors |
| `filesystem.read/write` | user-selected/app-owned paths |
| `secrets.use` | credential reference purpose |
| `camera.capture` | endpoint + session |
| `microphone.capture` | endpoint + session |
| `screen.capture` | source + session |

Manifest requests permission; user grant определяет фактический scope. Update с расширенным request не активируется автоматически.

Process isolation не предотвращает злонамеренный plugin от действий, доступных его OS user. DeckWeave честно показывает эту границу и не называет её полной sandbox.

Contributions одной runtime group разделяют память/OS process и фактически доверяют друг другу. Host проверяет scopes для каждого broker request, но не обещает изоляцию secret уже переданного соседнему instance. UI показывает суммарные permissions группы; требование разной trust boundary означает отдельную runtime group/процесс, а не только другой ContributionId.

## Пакетирование и каталог

Central catalogue является подписанным/версионированным metadata index, а не магазином исполняемого кода:

- package ID, version, license и supported OS/arch;
- GitHub repository и immutable release artifact URL;
- hash/signature, если publisher её предоставляет;
- contribution summary и permissions;
- conformance/compatibility results;
- update channel.

Установка всегда использует staging и immutable version directory. Никаких install-time scripts. V1 требует подтверждения каждого update; update policy пока регулирует проверку наличия/уведомления. Новая версия не заменяет active lock до validation и committed activation plan. Rollback сохраняет предыдущий artifact и settings revision, но не восстанавливает отозванные grants.

Direct local package installation остаётся доступной, но помечается source/trust.

## Версионирование

- Manifest, protocol, contribution schemas и profile settings versioned независимо.
- Package SemVer не заменяет protocol negotiation.
- Contribution ID стабилен между совместимыми releases.
- Unknown optional settings сохраняются round-trip.
- Breaking settings migration создаёт report и новую profile revision.
- V1 использует host-owned core migrations и bounded declarative package transforms; arbitrary executable hooks не запускаются при import/validate. Missing migration оставляет данные opaque/unavailable ([контракт миграций](RUNTIME_CONTRACTS.md#миграции-и-неизвестные-данные)).
- Host не запускает unsupported major.
- Deprecation имеет минимум один documented replacement path.

## SDK

### Общая permissive часть

Под `MIT OR Apache-2.0` публикуются:

- `.proto` contracts и generated types;
- manifest/settings schemas;
- reference client libraries;
- test harness и protocol fixtures;
- минимальные example plugins;
- conformance report format;
- package/archive tooling.

SDK/generated wire types не импортируют GPL `deckweave-domain`, host или production testkit. SDK harness работает через public wire contract; внутренние fixture runners могут оставаться GPL. CI проверяет transitive package boundaries ([ADR-0013](adr/0013-dependency-and-host-service-boundaries.md)).

### Языки

- **Rust SDK:** reference implementation и первая доступная SDK.
- **TypeScript SDK:** control process с bundled runtime или documented external dependency.
- **Python SDK:** после стабилизации protocol; interpreter declaration обязательна.

SDK не скрывает cancellation, deadlines и backpressure. «Простой» API является helper над тем же protocol, а не вторым несовместимым API.

## Conformance suite

Каждый control plugin проверяется минимум на:

1. valid/invalid manifest;
2. protocol negotiation;
3. activate/deactivate idempotency;
4. reconnect/session generation;
5. crash, hang и cancellation;
6. permission denied/changed;
7. oversized/unknown messages;
8. slow consumer/backpressure;
9. settings validation/migration;
10. deterministic virtual device/action fixture;
11. secret/log redaction;
12. package update/rollback.

Device providers дополнительно проверяют disconnect cleanup, identity и safe outputs. Renderer widgets — frame budget и crash. Media nodes получают отдельный validator перед реализацией media runtime.

## Error contract

Plugin возвращает structured category:

- `invalid_input`;
- `unsupported`;
- `unavailable`;
- `permission_denied`;
- `resource_busy`;
- `deadline_exceeded`;
- `cancelled`;
- `transient_external`;
- `permanent_external`;
- `unknown_outcome`;
- `internal`.

Human message локализуется UI/package, но category и machine details стабильны. Stack trace остаётся diagnostic attachment и не является user message.

## Намеренно запрещено

- arbitrary DLL в главном host;
- raw USB writes из UI/graph;
- firmware flashing;
- unbounded queues;
- raw audio/video через control protocol;
- secrets в manifest/settings/logs;
- direct profile file mutation plugin-ом;
- install scripts;
- custom editor с privileged Tauri bridge;
- compatibility claim без versioned fixture;
- автоматическое переписывание исходников legacy plugin.
