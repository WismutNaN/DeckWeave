# Модуль: Capability Registry

**Ответственность:** хранит runtime topology nodes/devices/elements/ports, session identity, availability и resource leases.
**Расположение:** `crates/deckweave-registry/`.

## Публичный интерфейс

| Символ | Тип | Описание |
|---|---|---|
| `CapabilityRegistry` | struct/owner | Mutable registry behind command queue |
| `RegistryCommand` | enum | Register/update/unregister/session change |
| `RegistrySnapshot` | immutable struct | Query-safe topology |
| `PortRef` | value type | Stable resolved port reference |
| `Availability` | enum | Available/Degraded/Unavailable + reason |
| `ResourceLease` | RAII/value | Shared/exclusive resource ownership |

## Зависимости

| Модуль | Что использует |
|---|---|
| Domain & Protocol | Descriptors/IDs |
| Diagnostics | Registry changes и conflicts |

## Инварианты

- Plugin не выбирает global IDs без host namespace assignment.
- Session generation монотонно меняется при reconnect/restart.
- Unregister session инвалидирует session-scoped handles.
- Descriptor change проходит compatibility validation.
- Lease conflict виден compiler/apply до side effect, где возможно.

## Намеренно НЕ обрабатывает

- persistence profile;
- automation routing;
- discovery hardware;
- permission prompts.

## Заметки для агента

> Registry — runtime truth, profile — desired configuration. Не записывать transient OS path в stable profile identity. Snapshots должны быть дешёвыми и пригодными для UI/API без доступа к mutable maps.

