# Модуль: Domain & Protocol

**Ответственность:** владеет стабильными идентификаторами, descriptors, schemas, envelopes и versioned wire contracts DeckWeave.
**Расположение:** `crates/deckweave-domain/`, `crates/deckweave-protocol/`, `protocol/`.

## Публичный интерфейс

| Символ | Тип | Описание |
|---|---|---|
| `WorkspaceId`, `RevisionId` | value types | Стабильная identity workspace/revision |
| `NodeDescriptor`, `DeviceDescriptor` | struct | Объявленная topology |
| `PortDescriptor`, `SchemaRef` | struct | Typed connection contract |
| `SignalEnvelope` | struct | Event/value/state с time/session/trace |
| `CommandEnvelope`, `CommandOutcome` | struct | Invocation и semantic result |
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
- Unknown optional fields сохраняются, если format это допускает.
- Edge/value/state semantics различаются типом, а не convention.
- `session_generation` обязателен для runtime messages.

## Намеренно НЕ обрабатывает

- хранение документов;
- plugin lifecycle;
- graph execution;
- HTTP/Tauri/MCP transports;
- raw media buffers.

## Заметки для агента

> Это самый стабильный слой. Не добавлять «удобное» поле только для N4 Pro или Windows. Namespaced extension предпочтительнее изменения core vocabulary. Protocol package лицензируется отдельно под MIT OR Apache-2.0; application domain implementation может оставаться GPL.

