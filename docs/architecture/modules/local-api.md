# Модуль: Local API

**Ответственность:** Application Gateway экспортирует commands, queries, subscriptions, authorization, stable errors и operation tracking для UI/CLI/Codex; Local API является одним из тонких transport adapters.
**Расположение:** contracts в `crates/deckweave-application/`, transports в `crates/deckweave-api/`, client в `apps/cli/`.

## Публичный интерфейс

| Символ | Тип | Описание |
|---|---|---|
| `ApplicationService` | trait | Canonical commands/queries |
| `open_mutation_window` | use case | Авторизованная выдача/возобновление persisted dedup epoch |
| `AppCommand`, `AppQuery` | enums | Stable operations |
| `OperationReceipt` | struct | Accepted mutation/operation ID |
| `OperationSnapshot` | struct | Progress/result |
| `ApplyUseCase` | port | Host-owned plan/apply/status без зависимости Gateway от Host implementation |
| `SubscriptionCursor` | value | Delta/snapshot resume |
| `ApiAuthScope` | enum | Read/draft/apply/admin |

## Зависимости

| Модуль | Что использует |
|---|---|
| Application contracts | Commands/queries и `ApplicationGateway`; Host передаёт только read-only status port |
| Bounded context ports | Маршрутизация use cases без импорта их infrastructure |
| Principal context | Authenticated identity; authorization проверяет service, не только transport |
| Diagnostics | Operation trace |

## Инварианты

- Transport adapters не содержат business logic.
- Gateway не меняет aggregates напрямую: он вызывает application use case контекста-владельца.
- Mutations имеют durable idempotency receipt и command-specific preconditions; apply проверяет draft и active revision.
- Apply отделён от draft edit.
- Dangerous operation требует scoped confirmation.
- Loopback transport не слушает external interfaces.
- MCP adapter не получает privileged bypass.
- Повтор key возвращает прежний receipt до проверки старой revision; несовпадающий payload/истёкшее dedup window отклоняется.
- Авторизация проверяется до выдачи окна и до replay; store различает `idempotency_expired`, conflict и исчерпанную receipt quota.
- Snapshot/delta cursor имеет HostBootId/sequence; gap требует resync.
- Dry-run/simulation не запускают plugin/hardware/network effects.

## Намеренно НЕ обрабатывает

- plugin wire protocol;
- direct file editing;
- remote multi-user server;
- LLM prompting.

## Заметки для агента

> Сначала реализовать in-process test adapter, затем loopback HTTP/WS и CLI. OpenAPI/JSON Schema генерируется из того же command vocabulary. Не делать Tauri-only methods.

In-process contract уже поддерживает `PlanApply`, `ApplyDraft` и `Operation`: plan авторизуется scope `simulation`, commit — `apply`, status — `read`. Explicit `allow_disruption` связан plan hash и durable intent, поэтому consent к downtime нельзя подставить после plan. `OperationId` выводится Host из server-issued dedup window/key, поэтому lost reply повторно адресует тот же durable intent. Desktop production composition подключается отдельно с runtime coordinator, а не через особый Tauri-only путь.

Нормативный контракт: [Application API](../../contracts/application-api.md), включая scopes с первого service adapter и [идемпотентность](../../contracts/application-api.md#идемпотентность-и-dry-run).
