# Модуль: Plugin Supervisor

**Ответственность:** индексирует packages без запуска, проверяет grants, управляет process sessions, leases, health, restart и updates.
**Расположение:** `crates/deckweave-plugin-supervisor/`.

## Публичный интерфейс

| Символ | Тип | Описание |
|---|---|---|
| `PackageIndex` | service | Immutable installed package metadata |
| `PluginSupervisor` | owner | Process/session lifecycle |
| `RuntimeLease` | handle | Причина держать runtime активным |
| `PluginSession` | struct | Negotiated protocol/session generation |
| `PermissionGrant` | struct | Actual scoped grant |
| `PluginHealth` | snapshot | State, heartbeat, crash/backoff |

## Зависимости

| Модуль | Что использует |
|---|---|
| Domain & Protocol | Handshake/envelopes |
| Capability Registry | Registration lifecycle |
| Package index / host authorization ports | Immutable package metadata и текущие grants; профиль не является grant store |
| Diagnostics | Logs/metrics |

## Инварианты

- Indexing не исполняет package code.
- Default process granularity — active runtime group package.
- Runtime active только после protocol/descriptor validation.
- Restart создаёт новую session generation.
- Все queues, messages и timeouts bounded.
- Deadline включает outbound write и ожидание завершения reader threads; заполненный stdin pipe и унаследованные descendants pipes не блокируют watchdog/kill path.
- Reliable control использует bounded FIFO и faults runtime при overflow; replaceable frame/value data использует отдельный bounded keyed latest-value lane с coalescing metric.
- Handshake согласует required features/schema versions и effective limits; pending jobs удерживают lease лишь до terminal outcome/deadline и bounded cleanup.
- Expanded permissions блокируют automatic activation.
- Runtime key = package/artifact hash/group; v1 разрешает одну active version package на host.
- Discovery до первого binding требует scoped lease; static index не запускает provider.
- Prepare не исполняет user actions; exclusive handoff объявляет downtime и использует общий ResourceLease arbiter.
- Grant revoke блокирует dispatch и инициирует cleanup, без обещания отменить уже выполненный effect.
- Prepare/recovery получают точные package/version/hash/runtime-group/contribution requirements из Automation; выбор «первого» group/contribution и разбор graph JSON запрещены.

## Намеренно НЕ обрабатывает

- domain execution;
- legacy WebSocket semantics;
- media buffers;
- доказуемую sandbox malicious executable.

## Заметки для агента

> Stderr — diagnostics, stdout — framed protocol; никогда не смешивать. Process tree cleanup должен тестироваться на Windows и Linux. Circuit breaker важнее бесконечного restart loop.

Обязательны [resource/version/discovery contracts](../../contracts/lifecycle.md#resources-discovery-и-версии-packages) и package-lock validation при apply/update/rollback.

`ProductionRuntimeCoordinator` соединяет `RuntimeController` и `ProcessRuntime` через Host planning/cutover ports. Он владеет prepared/active generation, bounded monitor, grant-revision revalidation, registry publication/cleanup и shutdown; desktop composition использует тот же путь, что сквозной virtual fixture. Prepare и recovery вызывают один resolver сохранённых runtime requirements и не разбирают `fixture.kind`. Coordinator запускает несколько distinct package/runtime groups и очищает весь набор при partial prepare, cutover failure или shutdown. Несколько contributions одного group пока требуют будущего grouped-activation protocol и отклоняются явно. Автоматический restart после внешнего crash остаётся отдельным fault-hardening, а не переносится в UI или SDK.

Configured instances/settings, group multiplexing и service dependency leases следуют [композиции](../../contracts/plugin-composition.md), [ADR-0019](../../decisions/0019-plugin-composition.md) и [pre-6 gate](../../implementation/pre-phase-6.md). Открытый массив `dependencies` в schema не считается dependency resolver. Наличие runtime group не означает готовность двух настроенных accounts.
