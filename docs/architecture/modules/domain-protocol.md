# Модуль: Domain & Protocol

**Ответственность:** владеет стабильными идентификаторами, descriptors, schemas, envelopes и versioned wire contracts DeckWeave.
**Расположение:** `crates/deckweave-domain/`, `crates/deckweave-protocol/`, `protocol/`.

## Публичный интерфейс

| Символ | Тип | Описание |
|---|---|---|
| `WorkspaceId`, `WorkspaceRevisionId`, `DraftVersion` | value types | Configuration identity и optimistic editing version |
| `HostBootId`, `ActivationEpoch`, `IngressOrdinal` | value types | Runtime lifetime и упорядочение ingress |
| `NodeDescriptor`, `DeviceDescriptor` | struct | Объявленная topology |
| `PortDescriptor`, `SchemaRef` | struct | Typed connection contract |
| `SignalEnvelope` | struct | Event/value/state с time/session/trace |
| `CommandEnvelope`, `CommandOutcome` | struct | Invocation и semantic result |
| `OperationDescriptor`, `InvocationReply`, `JobId` | schema/envelopes | Input/result/error, completion predicate, progress/cancel и limits |
| `RuntimeArtifactRef`, `DescriptorRevision`, `LightTopology` | value/descriptors | Scoped result data, dynamic ports и отдельная geometry |
| `DeckWeaveError` | tagged type | Stable error categories |
| `ProtocolEnvelope` | protobuf message | Native plugin wire envelope |

## Зависимости

| Модуль | Что использует |
|---|---|
| Никакой application module | Domain остаётся leaf dependency |
| Serde/Protobuf runtime | Serialization и generated types |

## Инварианты

- ID не зависят от локализованных labels и OS paths.
- Wire types не содержат UI framework или device-specific commands.
- Unknown optional document/settings fields сохраняются; wire relay использует opaque bytes, codegen round-trip проверяется fixtures.
- Edge/value/state semantics различаются типом, а не convention.
- `session_generation` обязателен для runtime messages.
- Source monotonic time всегда имеет clock domain; host ingress time/order не выводится из remote timestamp.
- Permissive protocol/SDK не импортирует GPL domain; преобразование wire ↔ application types находится со стороны application.

## Намеренно НЕ обрабатывает

- хранение документов;
- plugin lifecycle;
- graph execution;
- HTTP/Tauri/MCP transports;
- raw media buffers.

## Заметки для агента

> Это самый стабильный слой. Не добавлять «удобное» поле только для N4 Pro или Windows. Namespaced extension предпочтительнее изменения core vocabulary. Protocol package лицензируется отдельно под MIT OR Apache-2.0; application domain implementation может оставаться GPL.

Семантика типов: [Lifecycle](../../contracts/lifecycle.md#разные-виды-версий). `ApplicationService`/commands/queries размещаются отдельно в `deckweave-application`, без реализации Host и transports.

До SDK freeze проверяются [automation](../../contracts/automation.md), [lighting](../../contracts/lighting.md) и [media descriptor](../../contracts/media.md) на независимых fixtures. Конкретный media buffer ABI не фиксируется до backend ADR. `GraphNodeId`, capability `NodeId`, invocation/job IDs различаются; `RunId` означает корневой ExecutionId.
