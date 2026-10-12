# Модуль: Capability Registry

**Ответственность:** хранит runtime topology nodes/devices/elements/ports, session identity, availability и resource leases.
**Расположение:** `crates/deckweave-registry/`.

## Публичный интерфейс

| Символ | Тип | Описание |
|---|---|---|
| `CapabilityRegistry` | struct/owner | Mutable registry behind command queue |
| `RegistryCommand` | enum | Register/update/unregister/session change |
| `RegistrySnapshot` | immutable struct | Query-safe topology |
| `StableEndpointRef`, `ResolvedEndpoint` | value types | Persisted identity отдельно от session-scoped resolution |
| `Availability` | enum | Available/Degraded/Unavailable + reason |
| `ResourceLease` | RAII/value | Shared/exclusive resource ownership |
| `DescriptorRevision`, `PortTemplate` | versioned descriptors | Atomic dynamic topology и requestable instances |

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
- Session становится видимой только вместе с валидным initial descriptor snapshot; неуспешный candidate не заменяет прежнюю topology.
- Delta требует expected base; gap вызывает snapshot resync, incompatible change приостанавливает затронутые routes.
- Resolved endpoint включает descriptor revision; removed port не переназначает stable binding на соседний индекс.
- Stable endpoint содержит package/node/provider, discriminated device/service target и element/capability/port path; локальные IDs не считаются глобальными.
- Runtime group/session/generation/revision не сохраняются в binding и появляются только в resolved handle.
- Lease conflict виден compiler/apply до side effect, где возможно.
- Resource lease принадлежит полной runtime session либо bounded prepare/probe operation; package ID сам по себе не является cleanup key.
- Replacement/invalidation, cancel и deadline expiry освобождают leases только точного owner; stale generation не затрагивает текущую.
- Canonical resource key описывает physical interface/endpoint независимо от provider aliases; probe использует тот же arbiter.
- Discovery candidates не открывают exclusive handles и не дают output permission.
- Disconnect/sequence gap инициирует resync и cleanup held outputs; freshness не преобразуется в false/zero.

## Намеренно НЕ обрабатывает

- persistence profile;
- automation routing;
- discovery hardware;
- permission prompts.

## Заметки для агента

> Registry — runtime truth, profile — desired configuration. Не записывать transient OS path в stable profile identity. Snapshots должны быть дешёвыми и пригодными для UI/API без доступа к mutable maps.

См. [идентичность](../../contracts/lifecycle.md#разные-виды-версий) и [resources](../../contracts/lifecycle.md#resources-discovery-и-версии-packages).

Dynamic/requestable ports определены в [capability negotiation](../../contracts/capabilities.md#изменения-descriptor-во-время-сессии). Light topology и media port count изменяются через этот контракт, а не приватный callback UI.
