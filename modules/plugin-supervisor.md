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
| Profile Store | Grants/package metadata |
| Diagnostics | Logs/metrics |

## Инварианты

- Indexing не исполняет package code.
- Default process granularity — active runtime group package.
- Runtime active только после protocol/descriptor validation.
- Restart создаёт новую session generation.
- Все queues, messages и timeouts bounded.
- Expanded permissions блокируют automatic activation.

## Намеренно НЕ обрабатывает

- domain execution;
- legacy WebSocket semantics;
- media buffers;
- доказуемую sandbox malicious executable.

## Заметки для агента

> Stderr — diagnostics, stdout — framed protocol; никогда не смешивать. Process tree cleanup должен тестироваться на Windows и Linux. Circuit breaker важнее бесконечного restart loop.

