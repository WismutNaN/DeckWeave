# Документация DeckWeave

DeckWeave — публичная modular desktop platform для объединения устройств, приложений, визуальных поверхностей и автоматизаций. Документация отделяет подтверждённые требования, стабильные контракты и отложенные runtime-компоненты, чтобы первая N4 Pro версия не зависела от реализации всего будущего продукта.

## Рекомендуемый порядок чтения

1. [PREPROJECT.md](PREPROJECT.md) — зачем существует проект и какие решения подтверждены.
2. [REQUIREMENTS.md](REQUIREMENTS.md) — проверяемые требования и первая сквозная приёмка.
3. [MODES.md](MODES.md) — как сочетаются deployment, workspace, input и plugin modes.
4. [ARCHITECTURE.md](ARCHITECTURE.md) — границы системы и data flow.
5. [PLAN.md](PLAN.md) — порядок реализации короткими рабочими фазами.

## Спецификации

| Документ | Назначение |
|---|---|
| [ARCHITECTURE.md](ARCHITECTURE.md) | Системный контекст, процессы, компоненты и evolution rules |
| [CAPABILITY_MODEL.md](CAPABILITY_MODEL.md) | Nodes, devices, elements, ports, identity и временная семантика |
| [PLUGIN_MODEL.md](PLUGIN_MODEL.md) | Package format, contributions, protocol, lifecycle, SDK и catalog |
| [COMPATIBILITY.md](COMPATIBILITY.md) | OpenAction/Elgato levels и будущий Companion bridge |
| [API.md](API.md) | Общий application API для desktop UI, CLI, Codex и MCP |
| [SECURITY.md](SECURITY.md) | Trust tiers, permissions, secrets и опасные операции |
| [UI.md](UI.md) | UX-принципы, editors, click budgets и состояния |
| [research/REFERENCES.md](research/REFERENCES.md) | Первичные источники и правила заимствования |
| [research/stack.md](research/stack.md) | Исходное пользовательское исследование; сохраняется без переписывания |

## ADR

| ADR | Решение |
|---|---|
| [0001](adr/0001-stack-and-product-shape.md) | Rust modular monolith, Tauri/Svelte, Windows-first |
| [0002](adr/0002-plugin-execution.md) | Общий package format и несколько runtime classes |
| [0003](adr/0003-storage.md) | Versioned documents, revisions, snapshots и asset store |
| [0004](adr/0004-concurrency-and-realtime.md) | Deterministic control plane и отдельный media data plane |
| [0005](adr/0005-failure-isolation.md) | Process isolation, bounded queues и degraded operation |
| [0006](adr/0006-openaction-compatibility.md) | OpenAction/Elgato bridge на границе |
| [0007](adr/0007-licensing.md) | GPL application/core и permissive protocol SDK |
| [0008](adr/0008-api-first-and-revisions.md) | Единый application API и optimistic revisions |
| [0009](adr/0009-surface-scene-model.md) | Непрерывный scene canvas и независимые hit regions |
| [0010](adr/0010-compiled-automation-plan.md) | Draft graph компилируется в immutable execution plan |
| [0011](adr/0011-deployment-modes.md) | Per-user tray host и portable distribution |

## Модули

| Модуль | Ответственность |
|---|---|
| [Domain & Protocol](../modules/domain-protocol.md) | Стабильные ID, schemas, envelopes и wire contracts |
| [Host Runtime](../modules/host-runtime.md) | Lifecycle, active revision и координация |
| [Capability Registry](../modules/capability-registry.md) | Nodes, devices, elements, ports и availability |
| [Plugin Supervisor](../modules/plugin-supervisor.md) | Discovery, permissions, process lifecycle и health |
| [Automation Engine](../modules/automation-engine.md) | Compile, state, layers, schedules и execution |
| [Profile Store](../modules/profile-store.md) | Drafts, revisions, snapshots, assets и export |
| [Surface Compositor](../modules/surface-compositor.md) | Scene graph, hit testing и frame scheduling |
| [Widget Runtime](../modules/widget-runtime.md) | Изолированные custom web widgets |
| [Desktop Shell](../modules/desktop-shell.md) | Tauri tray UI и editors |
| [Local API](../modules/local-api.md) | Commands, queries, subscriptions и Codex access |
| [Compatibility Host](../modules/compatibility-host.md) | OpenAction/Elgato translation |
| [Platform Adapters](../modules/platform-adapters.md) | OS input/output, active app и virtual targets |
| [Device Studio](../modules/device-studio.md) | Discovery, calibration, capture и conformance |
| [N4 Pro Plugin](../modules/n4pro-plugin.md) | Mirabox-specific protocol и capabilities |
| [Gamepad Plugin](../modules/gamepad-plugin.md) | Buttons, axes, motion и haptics |
| [Remote Agent](../modules/remote-agent.md) | Paired network nodes и future media permissions |
| [Media Runtime](../modules/media-runtime.md) | Future low-latency audio/video graph |
| [Diagnostics & Test Lab](../modules/diagnostics-test-lab.md) | Traces, replay, metrics и fixtures |

## Текущий статус

Текущая фаза: **архитектурная спецификация завершена; реализация не начата**. Следующая работа — [Фаза 1](PLAN.md#фаза-1--скелет-репозитория). Ни одна отложенная возможность не должна добавляться в core раньше своего acceptance test и ADR.

