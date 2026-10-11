# Границы модулей

Модульный документ описывает ответственность, публичные ports, dependencies и инварианты. Детальная семантика принадлежит [контрактам](../../README.md#контракты), порядок работы — [плану](../../implementation/roadmap.md). Пути crates/plugins в документах являются целевыми: исполняемого кода пока нет.

| Модуль | Ответственность |
|---|---|
| [Shared Kernel & Protocol](domain-protocol.md) | Стабильные общие ID, schemas, envelopes и wire contracts |
| [Workspace Configuration](workspace-configuration.md) | Draft aggregate, revisions, snapshots и export/import policy |
| [Host Runtime](host-runtime.md) | Lifecycle, active revision, координация и runtime artifact broker |
| [Host Platform Services](host-platform-services.md) | Locks, data roots, credentials, grants, autostart и process cleanup |
| [Capability Registry](capability-registry.md) | Nodes, devices, elements, ports и availability |
| [Plugin Supervisor](plugin-supervisor.md) | Discovery, permissions, process lifecycle и health |
| [Automation Engine](automation-engine.md) | Compile, state/layers, typed jobs/results, schedules и execution |
| [Profile Store](profile-store.md) | Infrastructure persistence adapter для Workspace Configuration |
| [Surface Compositor](surface-compositor.md) | Display/light scenes, geometry, effects, hit testing и output ownership |
| [Widget Runtime](widget-runtime.md) | Изолированные custom web widgets |
| [Interaction Runtime](interaction-runtime.md) | Host-owned palettes/dialogs с typed result |
| [Desktop Shell](desktop-shell.md) | Tauri tray UI и editors |
| [Local API](local-api.md) | Commands, queries, subscriptions и Codex access |
| [Compatibility Host](compatibility-host.md) | OpenAction/Elgato translation |
| [Platform Adapters](platform-adapters.md) | OS input/output, active app и virtual targets |
| [Platform Input](platform-input.md) | Bundled keyboard/mouse sources, intercept и emit semantics |
| [Device Studio](device-studio.md) | Discovery, calibration, capture и conformance |
| [N4 Pro Plugin](n4pro-plugin.md) | Mirabox-specific protocol и capabilities |
| [Gamepad Plugin](gamepad-plugin.md) | Buttons, axes, motion и haptics |
| [Remote Agent](remote-agent.md) | Paired network nodes и future media permissions |
| [Media Runtime](media-runtime.md) | Future low-latency audio/video graph |
| [Diagnostics & Test Lab](diagnostics-test-lab.md) | Traces, replay, metrics и fixtures |
