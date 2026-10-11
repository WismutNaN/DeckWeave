# Модуль: Local API

**Ответственность:** экспортирует application commands, queries, subscriptions, auth, revisions и operation tracking для UI/CLI/Codex.
**Расположение:** contracts в `crates/deckweave-application/`, transports в `crates/deckweave-api/`, client в `apps/cli/`.

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
| Application contracts | Commands/queries/service ports; реализацию Host передаёт composition root |
| Principal context | Authenticated identity; authorization проверяет service, не только transport |
| Diagnostics | Operation trace |

## Инварианты

- Transport adapters не содержат business logic.
- Mutations имеют durable idempotency receipt и command-specific preconditions; apply проверяет draft и active revision.
- Apply отделён от draft edit.
- Dangerous operation требует scoped confirmation.
- Loopback transport не слушает external interfaces.
- MCP adapter не получает privileged bypass.
- Повтор key возвращает прежний receipt до проверки старой revision; несовпадающий payload/истёкшее dedup window отклоняется.
- Snapshot/delta cursor имеет HostBootId/sequence; gap требует resync.
- Dry-run/simulation не запускают plugin/hardware/network effects.

## Намеренно НЕ обрабатывает

- plugin wire protocol;
- direct file editing;
- remote multi-user server;
- LLM prompting.

## Заметки для агента

> Сначала реализовать in-process test adapter, затем loopback HTTP/WS и CLI. OpenAPI/JSON Schema генерируется из того же command vocabulary. Не делать Tauri-only methods.

Нормативный контракт: [Application API](../../contracts/application-api.md), включая scopes с первого service adapter и [идемпотентность](../../contracts/application-api.md#идемпотентность-и-dry-run).
