# Модуль: Local API

**Ответственность:** экспортирует application commands, queries, subscriptions, auth, revisions и operation tracking для UI/CLI/Codex.
**Расположение:** `crates/deckweave-api/`, client в `apps/cli/`.

## Публичный интерфейс

| Символ | Тип | Описание |
|---|---|---|
| `ApplicationService` | trait | Canonical commands/queries |
| `AppCommand`, `AppQuery` | enums | Stable operations |
| `OperationReceipt` | struct | Accepted mutation/operation ID |
| `OperationSnapshot` | struct | Progress/result |
| `SubscriptionCursor` | value | Delta/snapshot resume |
| `ApiAuthScope` | enum | Read/draft/apply/admin |

## Зависимости

| Модуль | Что использует |
|---|---|
| Host Runtime | Command/query implementation |
| Profile Store | Draft/revision operations |
| Diagnostics | Operation trace |

## Инварианты

- Transport adapters не содержат business logic.
- Mutations имеют idempotency key и expected revision.
- Apply отделён от draft edit.
- Dangerous operation требует scoped confirmation.
- Loopback transport не слушает external interfaces.
- MCP adapter не получает privileged bypass.

## Намеренно НЕ обрабатывает

- plugin wire protocol;
- direct file editing;
- remote multi-user server;
- LLM prompting.

## Заметки для агента

> Сначала реализовать in-process test adapter, затем loopback HTTP/WS и CLI. OpenAPI/JSON Schema генерируется из того же command vocabulary. Не делать Tauri-only methods.

