# План реализации DeckWeave

План рассчитан на одного разработчика или одного coding agent за фазу. Каждая фаза длится ориентировочно 1–5 рабочих дней, заканчивается работающей системой и не включает скрытую реализацию будущих компонентов.

## Легенда статусов

- [ ] Не начата
- [~] В процессе
- [x] Готово

## Обзор фаз

| Фаза | Название | Результат | Трудоёмкость | Статус |
|---:|---|---|:---:|:---:|
| 1 | Скелет репозитория | Workspace компилируется, UI shell и CI запускаются | S | [ ] |
| 2 | Domain и protocol contracts | Versioned types/schemas/generated code | M | [ ] |
| 3 | Plugin supervisor и virtual package | Первый изолированный plugin end-to-end | L | [ ] |
| 4 | Profile revisions и application API | Draft/validate/apply/rollback через API | L | [ ] |
| 5 | Минимальный automation engine | Source → action → feedback deterministic path | L | [ ] |
| 6 | Desktop workspace canvas | Устройства и bindings редактируются в UI | L | [ ] |
| 7 | Windows base provider | Volume/media/app/active-window работают | M | [ ] |
| 8 | N4 Pro hardware provider | Keys/encoders/touch/light/haptic доступны | L | [ ] |
| 9 | Surface compositor | Полный экран N4 Pro без grid gaps | L | [ ] |
| 10 | Gamepad и test lab | Второй класс hardware + replay/metrics | M | [ ] |
| 11 | Layers и сложные automation | Conditions, sequence/parallel, errors, timers | L | [ ] |
| 12 | Plugin SDK и packaging | Rust/TypeScript/Python SDK, conformance, install/update | L | [ ] |
| 13 | Widget runtime | Custom settings/widgets изолированы | M | [ ] |
| 14 | OpenAction compatibility | C0–C3 reference packages работают | L | [ ] |
| 15 | Device Studio | Standard discovery/calibration/capture | L | [ ] |
| 16 | Codex, CLI и optional MCP | Полный управляемый API workflow | M | [ ] |
| 17 | Portable build и Linux foundation | Separate data root и первый Linux adapter | L | [ ] |
| 18 | Hardening v1 | Ошибки, recovery, performance, docs, release | L | [ ] |

## Milestones

- **M1 — Architecture proof:** после Фазы 5 virtual input вызывает isolated action и feedback.
- **M2 — Daily N4 Pro:** после Фазы 9 panel управляет Windows и выводит свободную full-screen scene.
- **M3 — Extensible beta:** после Фазы 14 доступны SDK и существующие OpenAction plugins.
- **M4 — v1 foundation:** после Фазы 18 готов публичный Windows release и Linux foundation.

---

<a id="фаза-1--скелет-репозитория"></a>
## Фаза 1 — Скелет репозитория

**Цель:** проект компилируется и запускается без бизнес-логики.
**Результат:** Rust workspace, Tauri/Svelte shell, empty tray host и CI проходят smoke tests.
**Трудоёмкость:** S
**Статус:** [ ] Не начата

### Контекст для агента
> Исполняемого кода нет. Не создавать hardware, plugin IPC или graph logic. Нужно заложить package/license boundaries, lint/test policy и минимальный long-lived desktop lifecycle.

### Задачи

- [ ] Создать только необходимые каталоги из целевой структуры (→ [Архитектура — Целевая структура репозитория](ARCHITECTURE.md#целевая-структура-репозитория)).
- [ ] Создать Rust crates/apps с пустыми публичными boundaries (→ [Модуль: Domain & Protocol](../modules/domain-protocol.md), [Модуль: Host Runtime](../modules/host-runtime.md)).
- [ ] Создать Tauri/Svelte shell: окно, tray, close-to-tray, явный Exit (→ [Модуль: Desktop Shell](../modules/desktop-shell.md)).
- [ ] Зафиксировать GPL и permissive package license files/headers (→ [ADR-0007](adr/0007-licensing.md)).
- [ ] Настроить format/lint/test/license/provenance checks в CI (→ [Модуль: Diagnostics & Test Lab](../modules/diagnostics-test-lab.md)).

### Тесты

- [ ] Юнит: каждый crate импортируется без panic.
- [ ] Интеграционный: desktop стартует, окно закрывается, tray host остаётся жив.
- [ ] Ручная проверка: Exit из tray завершает process без orphan children.

### Ссылки

- [ADR-0001](adr/0001-stack-and-product-shape.md)
- [ADR-0011](adr/0011-deployment-modes.md)
- [Требования DW-OP-001](REQUIREMENTS.md#api-профили-и-эксплуатация)

---

## Фаза 2 — Domain и protocol contracts

**Цель:** зафиксировать минимальную capability/revision/message model до реализации runtime.
**Результат:** schemas генерируют Rust/TypeScript types; compatibility fixtures подтверждают round-trip.
**Трудоёмкость:** M
**Статус:** [ ] Не начата

### Контекст для агента
> Скелет готов, но типов domain ещё нет. Реализуется только минимальный vocabulary плюс extension mechanism; remote/media получают descriptors, но не runtime.

### Задачи

- [ ] Реализовать IDs, descriptors, port kinds, envelopes, outcomes и errors (→ [Модуль: Domain & Protocol](../modules/domain-protocol.md)).
- [ ] Создать Protobuf control envelope и JSON Schemas manifest/settings/profile (→ [Модель плагинов — Native control protocol](PLUGIN_MODEL.md#native-control-protocol)).
- [ ] Реализовать unknown-field/schema-version round-trip tests (→ [Архитектура — Правила эволюции](ARCHITECTURE.md#правила-эволюции)).
- [ ] Зафиксировать reserved `StreamPort`/remote descriptors без data plane (→ [Модуль: Remote Agent](../modules/remote-agent.md), [Модуль: Media Runtime](../modules/media-runtime.md)).
- [ ] Создать generator task для Rust/TypeScript types (→ [Модель плагинов — SDK](PLUGIN_MODEL.md#sdk)).

### Тесты

- [ ] Юнит: ID/descriptor/envelope validation и stable serialization.
- [ ] Property-based: unknown optional fields и invalid ranges.
- [ ] Contract: Rust и TypeScript читают один fixture одинаково.

### Ссылки

- [CAPABILITY_MODEL.md](CAPABILITY_MODEL.md)
- [ADR-0002](adr/0002-plugin-execution.md)

---

## Фаза 3 — Plugin supervisor и virtual package

**Цель:** доказать lazy, isolated, restartable plugin lifecycle.
**Результат:** virtual test package индексируется без запуска, активируется lease, публикует device и восстанавливается после crash.
**Трудоёмкость:** L
**Статус:** [ ] Не начата

### Контекст для агента
> Domain/protocol types существуют. Hardware и UI ещё не нужны; virtual package служит единственным provider и reference fixture.

### Задачи

- [ ] Реализовать manifest index/staging/hash validation (→ [Модуль: Plugin Supervisor](../modules/plugin-supervisor.md)).
- [ ] Реализовать Protobuf stdio handshake, limits, heartbeat и stop deadline (→ [Модель плагинов — Lifecycle](PLUGIN_MODEL.md#lifecycle)).
- [ ] Реализовать Capability Registry и session invalidation (→ [Модуль: Capability Registry](../modules/capability-registry.md)).
- [ ] Создать permissive virtual DeviceProvider/ActionProvider example (→ [Модуль: Diagnostics & Test Lab](../modules/diagnostics-test-lab.md)).
- [ ] Добавить restart/backoff/circuit-breaker tests (→ [ADR-0005](adr/0005-failure-isolation.md)).

### Тесты

- [ ] Юнит: manifest/permission/session state machines.
- [ ] Интеграционный: index без process; lease starts; release stops after grace.
- [ ] Fault injection: crash, hang, oversized message, late old-session event.

### Ссылки

- [PLUGIN_MODEL.md](PLUGIN_MODEL.md)
- [Архитектура — Plugin discovery и activation](ARCHITECTURE.md#plugin-discovery-и-activation)

---

## Фаза 4 — Profile revisions и application API

**Цель:** создать безопасный путь read → draft → validate → plan → apply → rollback.
**Результат:** CLI/test client редактирует virtual workspace, получает conflict и возвращается к snapshot.
**Трудоёмкость:** L
**Статус:** [ ] Не начата

### Контекст для агента
> Plugin lifecycle работает. Active automation пока пуст; фаза строит источник истины и command/query boundary.

### Задачи

- [ ] Реализовать JSON revision store, assets, atomic write и recovery (→ [Модуль: Profile Store](../modules/profile-store.md)).
- [ ] Реализовать draft commands, undo/redo и named snapshots (→ [ADR-0003](adr/0003-storage.md)).
- [ ] Реализовать ApplicationService, expected revision и OperationId (→ [Модуль: Local API](../modules/local-api.md)).
- [ ] Реализовать Host apply skeleton с prepare/cutover/failure report (→ [Модуль: Host Runtime](../modules/host-runtime.md)).
- [ ] Добавить in-process test client; HTTP/WS transport отложить до Фазы 16 (→ [API.md](API.md)).

### Тесты

- [ ] Юнит: draft commands, undo/redo, revision conflicts.
- [ ] Integration: crash-safe atomic write и recovery corrupted temp.
- [ ] Scenario: failed activation leaves previous revision active.

### Ссылки

- [ADR-0008](adr/0008-api-first-and-revisions.md)
- [Архитектура — Draft, compile и apply](ARCHITECTURE.md#draft-compile-и-apply)

---

## Фаза 5 — Минимальный automation engine

**Цель:** выполнить deterministic source → action → feedback path.
**Результат:** virtual button вызывает virtual action, меняет state и feedback; replay повторяет результат.
**Трудоёмкость:** L
**Статус:** [ ] Не начата

### Контекст для агента
> Profile/API и virtual plugin готовы. Реализуется узкий graph subset без schedules и complex UI.

### Задачи

- [ ] Реализовать graph compiler для source/action/state/condition/feedback (→ [Модуль: Automation Engine](../modules/automation-engine.md)).
- [ ] Реализовать immutable ExecutionPlan и serialized workspace scheduler (→ [ADR-0010](adr/0010-compiled-automation-plan.md)).
- [ ] Разделить edge queue и coalescing value lane (→ [Архитектура — Конкурентность и QoS](ARCHITECTURE.md#конкурентность-и-qos)).
- [ ] Реализовать trace/correlation/causation и basic loop guard (→ [Модуль: Diagnostics & Test Lab](../modules/diagnostics-test-lab.md)).
- [ ] Подключить compile/plan/apply к Host Runtime (→ [Модуль: Host Runtime](../modules/host-runtime.md)).

### Тесты

- [ ] Юнит: graph validation, schema mismatch, cycle rejection.
- [ ] Deterministic: один fixture + fake clock даёт identical final state.
- [ ] Performance baseline: virtual input → dispatch latency.

### Ссылки

- [Архитектура — Device event → automation → feedback](ARCHITECTURE.md#device-event--automation--feedback)
- [Требования DW-AU-001–004](REQUIREMENTS.md#назначения-и-автоматизация)

---

## Фаза 6 — Desktop workspace canvas

**Цель:** дать пользователю визуально настроить virtual device без редактирования JSON.
**Результат:** device виден на infinite canvas, control выбирается, action перетаскивается, draft валидируется и применяется.
**Трудоёмкость:** L
**Статус:** [ ] Не начата

### Контекст для агента
> ApplicationService и minimal graph работают. UI использует только этот contract и не получает plugin handles.

### Задачи

- [ ] Реализовать virtualized infinite canvas и device cards (→ [Модуль: Desktop Shell](../modules/desktop-shell.md)).
- [ ] Реализовать action library/search и drag-and-drop binding (→ [UI — Progressive disclosure](UI.md#progressive-disclosure)).
- [ ] Реализовать inspector, draft status, inline validation, undo/redo/apply (→ [Модуль: Profile Store](../modules/profile-store.md)).
- [ ] Реализовать Runtime health summary и no-device/first-run states (→ [UI — Состояния интерфейса](UI.md#состояния-интерфейса)).
- [ ] Добавить ru/en localization skeleton и keyboard navigation (→ [UI.md](UI.md)).

### Тесты

- [ ] Component: selection, drag/drop, inspector mutation.
- [ ] End-to-end: assignment за ≤ 3 действия.
- [ ] Manual: close/reopen WebView сохраняет active runtime/draft.

### Ссылки

- [Модуль: Desktop Shell](../modules/desktop-shell.md)
- [Требования DW-WS-004](REQUIREMENTS.md#рабочее-пространство-и-устройства)

---

## Фаза 7 — Windows base provider

**Цель:** обеспечить полезные системные actions/states без N4-specific logic.
**Результат:** virtual controls меняют volume, mute, media playback, запускают app и реагируют на active window.
**Трудоёмкость:** M
**Статус:** [ ] Не начата

### Контекст для агента
> Native plugin lifecycle и UI binding готовы. Windows functionality реализуется отдельным first-party control package.

### Задачи

- [ ] Реализовать Windows ActionProvider: volume/mute/media/app launch/focus (→ [Модуль: Platform Adapters](../modules/platform-adapters.md)).
- [ ] Реализовать StateProvider active application/window и audio endpoint state (→ [CAPABILITY_MODEL.md — State](CAPABILITY_MODEL.md#state)).
- [ ] Добавить permission descriptors и unavailable reasons (→ [SECURITY.md — Permission model](SECURITY.md#permission-model)).
- [ ] Добавить origin handling для emitted media/input commands (→ [Модуль: Automation Engine](../modules/automation-engine.md)).

### Тесты

- [ ] Unit: OS result → domain outcome mapping.
- [ ] Integration: volume/mute state round-trip на Windows 10/11 CI/manual matrix.
- [ ] Manual: active-window condition переключает virtual feedback.

### Ссылки

- [Требования DW-IN-001](REQUIREMENTS.md#системный-ввод-и-hardware-provisioning)
- [MODES.md — Режимы input](MODES.md#режимы-input)

---

## Фаза 8 — N4 Pro hardware provider

**Цель:** подключить всё подтверждённое hardware N4 Pro как typed capabilities.
**Результат:** keys, encoders/press, raw touch, frame sink, rings/light и haptic видны в registry и diagnostics.
**Трудоёмкость:** L
**Статус:** [ ] Не начата

### Контекст для агента
> Windows actions и plugin SDK foundation работают. Эта фаза не создаёт final surface editor, а доказывает protocol/device lifecycle.

### Задачи

- [ ] Реализовать discovery/identity/open/reconnect/keepalive (→ [Модуль: N4 Pro Plugin](../modules/n4pro-plugin.md)).
- [ ] Описать 10 keys, 4 encoder rotations/presses и touch coordinates (→ [CAPABILITY_MODEL.md](CAPABILITY_MODEL.md)).
- [ ] Реализовать full display FrameSink и brightness, если hardware поддерживает (→ [Модуль: Surface Compositor](../modules/surface-compositor.md)).
- [ ] Реализовать actual independent light targets и haptic enable/pattern degradation (→ [Требования DW-SF-005](REQUIREMENTS.md#surface-feedback-и-n4-pro)).
- [ ] Создать captured protocol fixtures и manual hardware checklist (→ [Модуль: Diagnostics & Test Lab](../modules/diagnostics-test-lab.md)).

### Тесты

- [ ] Unit: report parser/encoder, bounds, model/firmware variants.
- [ ] Integration: fake transport connect/events/outputs/disconnect.
- [ ] Manual: real N4 Pro hotplug, all controls, individual rings, haptic.

### Ссылки

- [Модуль: N4 Pro Plugin](../modules/n4pro-plugin.md)
- [research/REFERENCES.md](research/REFERENCES.md)

---

## Фаза 9 — Surface compositor

**Цель:** вывести свободную композицию на полный экран N4 Pro и связать independent touch regions.
**Результат:** текст/иконки/фон занимают весь canvas без grid gaps; touch regions запускают Windows actions.
**Трудоёмкость:** L
**Статус:** [ ] Не начата

### Контекст для агента
> N4 Pro FrameSink и touch coordinates проверены. Нужен built-in scene renderer без custom web runtime.

### Задачи

- [ ] Реализовать retained scene nodes, transforms, clipping и state bindings (→ [Модуль: Surface Compositor](../modules/surface-compositor.md)).
- [ ] Реализовать independent hit regions и coordinate transforms (→ [ADR-0009](adr/0009-surface-scene-model.md)).
- [ ] Реализовать dirty invalidation, per-surface FPS и latest-frame-wins (→ [Архитектура — Scene → hardware frame](ARCHITECTURE.md#scene--hardware-frame)).
- [ ] Реализовать Surface editor: visual/hit layers, snap templates, preview/stop (→ [UI — Surface editor](UI.md#surface-editor)).
- [ ] Создать стандартные text/icon/image/state widgets без browser runtime (→ [CAPABILITY_MODEL.md — Визуальные поверхности](CAPABILITY_MODEL.md#визуальные-поверхности)).

### Тесты

- [ ] Golden image: full canvas и transforms.
- [ ] Hit-test: raw → transformed point → correct region.
- [ ] Load: slow fake sink keeps bounded frame queue.
- [ ] Manual: N4 Pro full-screen scene без чёрных промежутков.

### Ссылки

- [Требования DW-SF-001–006](REQUIREMENTS.md#surface-feedback-и-n4-pro)
- [Модуль: Surface Compositor](../modules/surface-compositor.md)

---

## Фаза 10 — Gamepad и Test Lab

**Цель:** доказать независимость архитектуры от Stream Deck-класса и сделать hardware-independent replay.
**Результат:** Bluetooth gamepad buttons/axes/gyro/rumble работают рядом с N4 Pro; trace воспроизводится virtual provider.
**Трудоёмкость:** M
**Статус:** [ ] Не начата

### Контекст для агента
> N4 vertical slice полезна. Второй класс устройства должен использовать те же domain/plugin contracts без core changes.

### Задачи

- [ ] Реализовать gamepad discovery/buttons/axes/hotplug (→ [Модуль: Gamepad Plugin](../modules/gamepad-plugin.md)).
- [ ] Публиковать gyro/accelerometer, rumble/light только при actual support (→ [CAPABILITY_MODEL.md — Непрерывный ввод и датчики](CAPABILITY_MODEL.md#непрерывный-ввод-и-датчики)).
- [ ] Реализовать bounded event recorder и redacted trace format (→ [Модуль: Diagnostics & Test Lab](../modules/diagnostics-test-lab.md)).
- [ ] Реализовать replay runner/fake clock/virtual device fixture (→ [Требования DW-AU-007](REQUIREMENTS.md#назначения-и-автоматизация)).
- [ ] Добавить latency/queue/drop metrics в Runtime UI (→ [Модуль: Desktop Shell](../modules/desktop-shell.md)).

### Тесты

- [ ] Integration: two gamepads/reconnect/identity.
- [ ] Deterministic replay: axis + button + N4 feedback.
- [ ] Manual: Bluetooth rumble cancellation и motion availability.

### Ссылки

- [Модуль: Gamepad Plugin](../modules/gamepad-plugin.md)
- [Модуль: Diagnostics & Test Lab](../modules/diagnostics-test-lab.md)

---

## Фаза 11 — Layers и сложные automation

**Цель:** реализовать подтверждённые сценарии beyond one-binding.
**Результат:** temporary layer, active-window condition, sequence/parallel, timer и error policies редактируются и симулируются.
**Трудоёмкость:** L
**Статус:** [ ] Не начата

### Контекст для агента
> Minimal compiler/scheduler и replay уже стабильны. Расширяется node vocabulary без arbitrary code in core.

### Задачи

- [ ] Добавить layers, variables, tap/hold/chord и active-window conditions (→ [Модуль: Automation Engine](../modules/automation-engine.md)).
- [ ] Добавить sequence/parallel/join и stop/continue/retry/fallback policies (→ [MODES.md — Режимы ошибок automation](MODES.md#режимы-ошибок-automation)).
- [ ] Добавить monotonic timers, schedules с timezone/DST и bounded cycles (→ [CAPABILITY_MODEL.md — Временная семантика](CAPABILITY_MODEL.md#временная-семантика)).
- [ ] Реализовать Graph editor, collapsed groups и execution timeline (→ [UI — Graph editor](UI.md#graph-editor)).
- [ ] Реализовать conflict/priority diagnostics (→ [Требования DW-AU-008](REQUIREMENTS.md#назначения-и-автоматизация)).

### Тесты

- [ ] State-machine: tap/hold/chord/layer release.
- [ ] Parallel/error: deterministic outcomes under reordered external completions.
- [ ] Time: fake clock, timezone/DST schedule.
- [ ] Manual: held keyboard/gamepad control changes N4 encoder assignment.

### Ссылки

- [ADR-0010](adr/0010-compiled-automation-plan.md)
- [Требования DW-AU-002–008](REQUIREMENTS.md#назначения-и-автоматизация)

---

## Фаза 12 — Plugin SDK и packaging

**Цель:** позволить стороннему автору создать, протестировать и установить native control package.
**Результат:** Rust, TypeScript и Python examples проходят conformance; immutable artifact устанавливается и откатывается.
**Трудоёмкость:** L
**Статус:** [ ] Не начата

### Контекст для агента
> Protocol/supervisor используются first-party packages. Нужно отделить permissive SDK и public tooling от GPL application implementation.

### Задачи

- [ ] Опубликовать Rust SDK helpers без скрытия lifecycle/backpressure (→ [Модель плагинов — SDK](PLUGIN_MODEL.md#sdk)).
- [ ] Опубликовать TypeScript SDK/reference runtime packaging (→ [Модуль: Domain & Protocol](../modules/domain-protocol.md)).
- [ ] Опубликовать Python SDK для isolated conditions/actions с timeout/cancellation fixtures (→ [Модель плагинов — Языки](PLUGIN_MODEL.md#языки)).
- [ ] Реализовать conformance CLI/fixtures/report (→ [Модуль: Diagnostics & Test Lab](../modules/diagnostics-test-lab.md)).
- [ ] Реализовать staging install/update/rollback и permission diff (→ [Модуль: Plugin Supervisor](../modules/plugin-supervisor.md)).
- [ ] Описать metadata catalogue format с GitHub immutable artifacts (→ [Модель плагинов — Пакетирование и каталог](PLUGIN_MODEL.md#пакетирование-и-каталог)).

### Тесты

- [ ] Contract: Rust/TS/Python examples produce identical semantic fixtures.
- [ ] Security: path traversal, oversized archive, missing hash, expanded permission.
- [ ] Upgrade: failed new version restores previous package/runtime.

### Ссылки

- [ADR-0007](adr/0007-licensing.md)
- [SECURITY.md — Plugin install/update](SECURITY.md#plugin-installupdate)

---

## Фаза 13 — Widget runtime

**Цель:** поддержать custom settings UI и surface widgets без зависимости active output от Desktop WebView.
**Результат:** sample widget получает scoped state, возвращает frame, переживает закрытие UI и изолированно падает.
**Трудоёмкость:** M
**Статус:** [ ] Не начата

### Контекст для агента
> Built-in scene nodes уже покрывают простые widgets. Фаза добавляет только package-provided web content через lazy separate host.

### Задачи

- [ ] Выбрать/зафиксировать widget backend после Windows/Linux spike (→ [Модуль: Widget Runtime](../modules/widget-runtime.md)).
- [ ] Реализовать isolated origins, CSP, scoped bridge и rate limits (→ [SECURITY.md — Web property editors и widgets](SECURITY.md#web-property-editors-и-widgets)).
- [ ] Подключить WidgetLayer к compositor без unbounded frames (→ [Модуль: Surface Compositor](../modules/surface-compositor.md)).
- [ ] Реализовать schema form → custom editor fallback chain (→ [PLUGIN_MODEL.md — Settings и UI contributions](PLUGIN_MODEL.md#settings-и-ui-contributions)).

### Тесты

- [ ] Security: forbidden navigation/network/filesystem.
- [ ] Fault: widget crash/hang leaves host/surface alive.
- [ ] Performance: frame budget and idle host not launched when unused.

### Ссылки

- [Требования DW-SF-007–008](REQUIREMENTS.md#surface-feedback-и-n4-pro)
- [Модуль: Widget Runtime](../modules/widget-runtime.md)

---

## Фаза 14 — OpenAction compatibility

**Цель:** использовать существующие actions без подчинения core legacy model.
**Результат:** выбранные open packages достигают C0–C3 и имеют versioned compatibility reports.
**Трудоёмкость:** L
**Статус:** [ ] Не начата

### Контекст для агента
> Native SDK и widget isolation существуют. Compatibility host остаётся отдельным process tree и переводит contexts на bindings.

### Задачи

- [ ] Реализовать legacy manifest indexing C0 (→ [Модуль: Compatibility Host](../modules/compatibility-host.md)).
- [ ] Реализовать registration/lifecycle/key events/settings C1 (→ [COMPATIBILITY.md — Уровни совместимости](COMPATIBILITY.md#уровни-совместимости)).
- [ ] Реализовать scoped Property Inspector C2 (→ [COMPATIBILITY.md — Property Inspector](COMPATIBILITY.md#property-inspector)).
- [ ] Реализовать dial/touch/image/title/state bridge C3 (→ [Модуль: Surface Compositor](../modules/surface-compositor.md)).
- [ ] Публиковать exact version/OS compatibility matrix (→ [Модуль: Diagnostics & Test Lab](../modules/diagnostics-test-lab.md)).

### Тесты

- [ ] Conformance fixtures lifecycle/settings/feedback/dial/PI.
- [ ] Fault: legacy crash/re-register/slow/oversized.
- [ ] Manual: несколько открытых reference packages на N4 Pro layout.

### Ссылки

- [ADR-0006](adr/0006-openaction-compatibility.md)
- [COMPATIBILITY.md](COMPATIBILITY.md)

---

## Фаза 15 — Device Studio

**Цель:** упростить добавление standard HID/gamepad-like devices и подготовку vendor provider fixtures.
**Результат:** пользователь видит interfaces/live events, калибрует axis и сохраняет validated descriptor project.
**Трудоёмкость:** L
**Статус:** [ ] Не начата

### Контекст для агента
> Providers, test lab и UI уже существуют. Protocol Lab остаётся безопасным diagnostic tool, а не arbitrary USB console.

### Задачи

- [ ] Реализовать bounded probe session и standard discovery view (→ [Модуль: Device Studio](../modules/device-studio.md)).
- [ ] Реализовать live event/value inspector и calibration (→ [CAPABILITY_MODEL.md](CAPABILITY_MODEL.md)).
- [ ] Реализовать safe capture trace и descriptor project validation (→ [Модуль: Diagnostics & Test Lab](../modules/diagnostics-test-lab.md)).
- [ ] Подключить generated declarative mapping как package/project artifact (→ [PLUGIN_MODEL.md](PLUGIN_MODEL.md)).
- [ ] Добавить provisioning descriptor/readback UI без write implementation (→ [CAPABILITY_MODEL.md — Provisioning capability](CAPABILITY_MODEL.md#provisioning-capability)).

### Тесты

- [ ] Integration: generic HID/gamepad virtual fixtures.
- [ ] Safety: unknown output cannot be sent.
- [ ] Manual: calibrate a real axis and export descriptor project.

### Ссылки

- [UI — Device Studio](UI.md#device-studio)
- [Требования DW-IN-005](REQUIREMENTS.md#системный-ввод-и-hardware-provisioning)

---

## Фаза 16 — Codex, CLI и optional MCP

**Цель:** обеспечить полный поддерживаемый machine workflow без direct file edits.
**Результат:** CLI/Codex создаёт draft, валидирует, симулирует и применяет его; MCP adapter можно удалить без потери API.
**Трудоёмкость:** M
**Статус:** [ ] Не начата

### Контекст для агента
> ApplicationService стабилен и используется UI. Фаза добавляет authenticated loopback transport и thin clients.

### Задачи

- [ ] Реализовать loopback HTTP/JSON и WebSocket subscriptions (→ [Модуль: Local API](../modules/local-api.md)).
- [ ] Реализовать token/origin/rate/size security (→ [SECURITY.md — Local API](SECURITY.md#local-api)).
- [ ] Реализовать CLI для query/draft/validate/simulate/apply/operation (→ [API.md](API.md)).
- [ ] Реализовать optional MCP adapter как отдельный package/process (→ [API.md — MCP adapter](API.md#mcp-adapter)).
- [ ] Добавить Codex-oriented schemas/examples и revision conflict flow (→ [ADR-0008](adr/0008-api-first-and-revisions.md)).

### Тесты

- [ ] Contract: direct/HTTP/CLI/MCP adapters дают одинаковые results.
- [ ] Security: external bind/origin/token/confirmation rejection.
- [ ] End-to-end: Codex-like client builds and applies N4 workspace.

### Ссылки

- [Модуль: Local API](../modules/local-api.md)
- [Требования DW-API-001–003](REQUIREMENTS.md#api-профили-и-эксплуатация)

---

## Фаза 17 — Portable build и Linux foundation

**Цель:** подтвердить deployment portability и отсутствие Windows dependencies в core.
**Результат:** portable Windows build использует отдельный data root; Linux host/virtual provider/CLI compile и run.
**Трудоёмкость:** L
**Статус:** [ ] Не начата

### Контекст для агента
> Windows v1 features готовы. Linux phase не обязана сразу поддержать весь hardware, но должна доказать platform boundary.

### Задачи

- [ ] Реализовать installed/portable data-root/startup policies (→ [Модуль: Host Runtime](../modules/host-runtime.md)).
- [ ] Добавить selective export/import и cross-platform unavailable report (→ [Модуль: Profile Store](../modules/profile-store.md)).
- [ ] Создать Linux platform package skeleton и active-window/basic action spike (→ [Модуль: Platform Adapters](../modules/platform-adapters.md)).
- [ ] Запустить core/plugin/virtual/UI smoke в Linux CI (→ [Архитектура — Технологические решения](ARCHITECTURE.md#технологические-решения)).
- [ ] Документировать platform matrix и secret portability (→ [MODES.md](MODES.md), [SECURITY.md](SECURITY.md)).

### Тесты

- [ ] Windows installed vs portable data isolation.
- [ ] Import Windows profile on Linux yields degraded, preserves settings.
- [ ] CI: domain/core/protocol/plugin conformance on Windows/Linux.

### Ссылки

- [ADR-0011](adr/0011-deployment-modes.md)
- [Требования DW-OP-002, DW-PR-002–003](REQUIREMENTS.md#api-профили-и-эксплуатация)

---

## Фаза 18 — Hardening v1

**Цель:** подготовить публичный foundation release для реального ежедневного использования.
**Результат:** error paths, recovery, performance budgets, docs, packaging и release checklist пройдены.
**Трудоёмкость:** L
**Статус:** [ ] Не начата

### Контекст для агента
> Все v1 verticals реализованы. Новые features запрещены; фаза закрывает failure modes и доказательства качества.

### Задачи

- [ ] Проверить external input paths на panic/unwrap/unbounded work (→ [Архитектура — Режимы отказа](ARCHITECTURE.md#режимы-отказа)).
- [ ] Выполнить crash/reconnect/corruption/permission fault matrix (→ [Модуль: Diagnostics & Test Lab](../modules/diagnostics-test-lab.md)).
- [ ] Измерить и зафиксировать latency/RSS/frame/plugin activation budgets (→ [Архитектура — Бюджеты производительности](ARCHITECTURE.md#бюджеты-производительности)).
- [ ] Проверить secrets/log redaction, local API и package supply chain (→ [SECURITY.md](SECURITY.md)).
- [ ] Провести ru/en, keyboard navigation и click-budget usability pass (→ [UI.md](UI.md)).
- [ ] Проверить все public SDK/API/docs links и provenance (→ [docs/README.md](README.md)).
- [ ] Собрать signed/checksummed Windows release и documented Linux preview (→ [ADR-0007](adr/0007-licensing.md)).

### Тесты

- [ ] Юнит: error branches для всех fallible public functions.
- [ ] Integration: каждый external dependency unavailable/crashed/stale.
- [ ] Soak: N4 Pro + gamepad + actions + frames + UI reopen.
- [ ] Recovery: host kill during profile write/apply/plugin update.
- [ ] Manual acceptance: полный [первый сквозной критерий](REQUIREMENTS.md#первый-сквозной-критерий).

### Ссылки

- [Архитектура — Режимы отказа](ARCHITECTURE.md#режимы-отказа)
- [Модуль: Host Runtime](../modules/host-runtime.md)
- [Модуль: Plugin Supervisor](../modules/plugin-supervisor.md)

---

## Дальнейшие программы после v1

Это не обещания первого релиза. Каждая программа начинается отдельным ADR, threat model, prototype benchmark и собственным phased plan.

### Selective input replacement

- Windows observe → intercept/suppress → virtual keyboard/mouse/gamepad.
- Optional privileged helper/driver, emergency recovery и fail-open policy.
- Persistent keyboard/device profiles.
- Модули: [Platform Adapters](../modules/platform-adapters.md), [Security](SECURITY.md#input-safety).

### Remote agents

- Local pairing and widget panel.
- Steam Deck/Android touch/display.
- Cable/Wi-Fi transport abstraction.
- Camera/microphone/speaker and screen capture only with session grants.
- Модуль: [Remote Agent](../modules/remote-agent.md).

### Low-latency media

- Backend/latency benchmark.
- Audio graph and virtual microphone.
- EQ/noise suppression.
- CLAP and optional GStreamer/VST bridges.
- Модуль: [Media Runtime](../modules/media-runtime.md).

### Companion bridge

- Public interfaces first.
- Version-pinned internal tRPC adapter only with fixtures.
- Actions/variables/feedback mapping.
- Details: [COMPATIBILITY.md — Будущий Bitfocus Companion bridge](COMPATIBILITY.md#будущий-bitfocus-companion-bridge).

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
