# Локальный API DeckWeave

API — единственная application boundary для Desktop UI, CLI, Codex и optional MCP adapter. Он не повторяет plugin protocol: plugins предоставляют capabilities, а application API редактирует и наблюдает пользовательскую систему.

## Принципы

- одна операция имеет одинаковую семантику во всех transports;
- queries не имеют side effects;
- mutations адресуют draft или явно named runtime operation;
- expected revision предотвращает lost update;
- idempotency key позволяет безопасно повторить запрос после потери ответа;
- validate/simulate/plan доступны до apply;
- dangerous operation требует отдельного short-lived confirmation token;
- API возвращает operation ID для длительных действий;
- raw profile/plugin files не являются публичным API.

## Слои

```text
Desktop direct adapter ─┐
Loopback HTTP/WS ───────┼──> ApplicationService ──> Host/Core
CLI client ─────────────┤
Optional MCP adapter ───┘
```

Canonical Rust application service не зависит от HTTP/Tauri/MCP. Transport adapters выполняют auth, serialization и version negotiation, но не содержат бизнес-логику.

## Transport v1

- Bind только `127.0.0.1`/`::1` на random или configured port.
- Session bearer token хранится в user-only app data/credential mechanism.
- HTTP/JSON для commands/queries и WebSocket для subscriptions/progress.
- API version передаётся в path/handshake.
- Browser origins запрещены по умолчанию; Desktop WebView использует direct adapter.
- CLI получает token через user-local discovery file с ограниченными ACL.

Named pipe/Unix socket adapter может быть добавлен без изменения application contract.

## Ресурсы и операции

| Группа | Queries | Commands |
|---|---|---|
| Host | health, version, runtime snapshot | shutdown/restart только при явном разрешении |
| Workspaces | list/get revisions/get active | create draft, apply, snapshot, rollback |
| Drafts | document, validation, diff, undo state | patch command, undo, redo, validate, simulate |
| Devices | registry snapshot, descriptor, live availability | identify, open probe, reconnect |
| Plugins | catalogue/index/health/permissions | install, grant, activate, disable, update, rollback |
| Automation | graph, compiled diagnostics, trace | test signal, start/cancel execution |
| Surfaces | scene, preview, frame stats | update draft scene, preview/stop preview |
| Operations | status/progress/result | cancel |
| Diagnostics | metrics, traces, compatibility report | start/stop recording, replay |
| Export | selectable content report | create/import bundle |

Provisioning и future media/remote operations выделяются отдельными API groups с дополнительным confirmation/grant workflow.

## Mutation envelope

```json
{
  "request_id": "uuid",
  "idempotency_key": "client-generated-uuid",
  "expected_revision": "revision-id",
  "dry_run": false,
  "command": {
    "type": "draft.bind_action",
    "payload": {}
  }
}
```

Ответ содержит:

- accepted/rejected;
- resulting draft revision или operation ID;
- structured errors;
- validation delta;
- warnings и required confirmations;
- trace ID.

Payload schemas публикуются в OpenAPI/JSON Schema, но domain vocabulary остаётся source of truth.

## Optimistic concurrency

Каждый mutable resource имеет revision. При mismatch API возвращает:

- expected/current revision;
- machine-readable conflict paths;
- безопасный rebase hint, если command commutative;
- ссылку на актуальный snapshot.

API не делает silent last-write-wins для workspace, grants или provisioning.

## Длительные операции

Install, apply, compatibility scan, replay и provisioning возвращают `OperationId`.

States:

`queued → running → succeeded | failed | cancelled | unknown_outcome`.

Client может подписаться на progress или позже запросить result. Disconnect client не отменяет operation, если cancellation policy не говорит обратное.

## Codex

Codex получает высокоуровневые операции:

- list devices/capabilities/actions;
- create and patch draft;
- validate graph/settings;
- simulate with fixture;
- explain validation/error;
- plan and apply revision;
- inspect operation/health;
- scaffold plugin project metadata через SDK tooling.

Default policy: Codex может читать, создавать draft и запускать simulation; apply, plugin install, permission grant и provisioning требуют явно разрешённой операции. Это policy продукта, а не ограничение model provider.

## MCP adapter

MCP server является optional executable/package поверх local API:

- MCP tools отображаются на application commands;
- resources отображаются на immutable snapshots/diagnostics;
- prompts не являются authority;
- adapter не читает profile directory и не общается с plugins напрямую;
- удаление MCP adapter не меняет Desktop/CLI capabilities.

Таким образом, устаревание MCP SDK не затрагивает внутренний runtime.

## Подписки

Topics:

- host health;
- registry changes;
- state deltas;
- draft validation;
- apply/operation progress;
- plugin health;
- surface statistics;
- diagnostics trace summaries.

Slow subscriber получает новый snapshot cursor вместо неограниченной очереди deltas. Edge events не публикуются всем clients по умолчанию; raw/live stream требует diagnostic permission и rate limit.

## Ошибки

```json
{
  "code": "revision_conflict",
  "message_key": "api.error.revision_conflict",
  "details": {},
  "retryable": false,
  "trace_id": "uuid"
}
```

Stable codes отделены от локализованного текста. Transport status не заменяет domain error.

## Совместимость

- API major меняется только при breaking semantics.
- Client сначала запрашивает version/capabilities.
- Optional fields ignored/preserved where applicable.
- Deprecated operation имеет replacement metadata.
- CLI и MCP adapter тестируются теми же contract fixtures, что Desktop direct adapter.

## Не является API

- пути к JSON profiles;
- Rust internal structs без published schema;
- plugin stdin/stdout;
- Tauri private commands;
- OpenAction WebSocket;
- SQLite/cache layout;
- log file format.

