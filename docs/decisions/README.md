# Архитектурные решения

ADR фиксирует причину и последствия выбора, а не дублирует весь wire contract. Принятое решение уточняется ссылкой на новый ADR; старый номер не используется заново. Статус «принято» означает решение спецификации, не завершённую реализацию.

| ADR | Решение |
|---|---|
| [0001](0001-stack-and-product-shape.md) | Rust modular monolith, Tauri/Svelte, Windows-first |
| [0002](0002-plugin-execution.md) | Общий package format и несколько runtime classes |
| [0003](0003-storage.md) | Versioned documents, revisions, snapshots и asset store |
| [0004](0004-concurrency-and-realtime.md) | Deterministic control plane и отдельный media data plane |
| [0005](0005-failure-isolation.md) | Process isolation, bounded queues и degraded operation |
| [0006](0006-openaction-compatibility.md) | OpenAction/Elgato bridge на границе |
| [0007](0007-licensing.md) | GPL application/core и permissive protocol SDK |
| [0008](0008-api-first-and-revisions.md) | Единый application API и optimistic revisions |
| [0009](0009-surface-scene-model.md) | Непрерывный scene canvas и независимые hit regions |
| [0010](0010-compiled-automation-plan.md) | Draft graph компилируется в immutable execution plan |
| [0011](0011-deployment-modes.md) | Per-user tray host и portable distribution |
| [0012](0012-apply-and-execution-lifecycle.md) | Durable apply, execution epochs и outcome/recovery semantics |
| [0013](0013-dependency-and-host-service-boundaries.md) | Ациклические dependencies и минимальные host OS services |
| [0014](0014-specialized-execution-and-results.md) | Общие lifecycle границы, typed jobs/results и специализированные data paths |
| [0015](0015-device-geometry-and-output-ownership.md) | Geometry, light composition, output ownership и evidence touch capabilities |
| [0016](0016-ddd-bounded-contexts.md) | Bounded contexts, Shared Kernel и постепенная DDD-миграция |
| [0017](0017-bundled-platform-input.md) | Обычная клавиатура как bundled first-party provider |
| [0018](0018-interaction-ui-runtime.md) | Host-owned interaction sessions и plugin views |
| [0019](0019-plugin-composition.md) | Configured instances, explicit service bindings и владение plugin data |
| [0020](0020-windows-provider-runtime-groups.md) | Windows first-party package с раздельными control и input-observe runtime groups |
| [0021](0021-brokered-user-process-launch.md) | Host-brokered user process launch без ослабления plugin Job Object |
