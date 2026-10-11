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

Canonical Rust application contract находится в `deckweave-application` и не зависит от HTTP/Tauri/MCP. Transport adapters аутентифицируют client, выполняют serialization/version negotiation и передают `PrincipalContext`. Application service авторизует каждую операцию по scopes и актуальным grants, включая direct adapter. UI и MCP не могут обойти эту проверку. Реализация service находится в host; API adapters не импортируют её.

## Transport v1

- Bind только `127.0.0.1`/`::1` на random или configured port.
- Bearer token идентифицирует отдельного principal и scopes; credential хранится с user-only ACL и может быть отозван.
- HTTP/JSON для commands/queries и WebSocket для subscriptions/progress.
- API version передаётся в path/handshake.
- Browser origins запрещены по умолчанию; Desktop WebView использует direct adapter.
- Discovery file содержит endpoint, HostBootId, API version и bootstrap instructions. Широкий admin token не раздаётся всем clients через discovery; CLI/Codex получают отдельный scoped credential через host broker.

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
| Runs / Jobs | scoped results, redacted progress, known/unknown outcome | start/cancel run, reconcile при поддержке provider |
| Runtime artifacts | metadata/expiration и authorized bounded read | create/promote/delete по scope; никакого произвольного path access |
| Collections | provider query descriptors, page/cursor status | invoke read-only paginated query; acknowledge — отдельный write command |
| Surfaces | scene, preview, frame stats | update draft scene, preview/stop preview |
| Operations | status/progress/result | cancel |
| Diagnostics | metrics, traces, compatibility report | start/stop recording, replay |
| Export | selectable content report | create/import bundle |

Provisioning и future media/remote operations выделяются отдельными API groups с дополнительным confirmation/grant workflow.

Data/result bindings, light scenes и notification view models редактируются тем же draft API, что и обычный graph. Система IDs различает application `OperationId`, root execution/`RunId`, step `InvocationId` и provider `JobId`. Receipt apply/launch не означает завершение workflow. Queries результатов снова проверяют актуальные grants; secret values не сохраняются в durable dedup receipt. [Automation contract](automation.md) определяет job/result/artifact semantics; API не создаёт второй scheduler.

## Mutation envelope

```json
{
  "request_id": "uuid",
  "idempotency_key": "client-generated-uuid",
  "dedup_epoch": "server-issued-window-id",
  "preconditions": {
    "draft_id": "uuid",
    "expected_draft_version": 12
  },
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

Preconditions определяются command schema. Draft edit проверяет `DraftVersion`; apply — `draft_id`, точную draft version, expected active workspace/revision и `plan_hash`; grant/update — revision соответствующего ресурса; cancel — `OperationId` и допустимое состояние. Для создания ресурса применяется `must_not_exist` с client-selected ID. Один неоднозначный `expected_revision` не обслуживает все эти случаи.

## Идемпотентность и dry-run

Ключ дедупликации — `(PrincipalId, dedup_epoch, idempotency_key)`, payload fingerprint включает command и preconditions. Повтор с тем же содержимым возвращает прежний receipt/OperationId **до** проверки устаревшей revision; другой payload с тем же ключом возвращает `idempotency_conflict`. Актуальная авторизация проверяется и при чтении старого результата, secret payload не сохраняется в receipt.

Host выдаёт persisted `dedup_epoch` с `expires_at`; v1 хранит receipts не менее семи дней и до завершения pending operation. Истёкшее окно возвращает `idempotency_expired`, а не исполняет старый запрос как новый. Restart не обнуляет действующие окна и keys. Новое окно/ключ означают новую операцию и не являются способом автоматически повторить неизвестный side effect.

Store фиксирует intent до side effect, а для document mutations публикует результат и receipt через единый commit protocol. При crash между внешним side effect и подтверждением outcome становится `unknown_outcome`. API dedup не обеспечивает exactly-once на USB, process launch или чужом HTTP API; повтор там возможен только по [execution contract](lifecycle.md#executions-cancellation-и-смена-плана).

`dry_run`, `validate`, `plan` и default `simulate/replay` не запускают packages, network requests, process actions и hardware output. Они используют static manifests, snapshots и fake targets; отсутствие live evidence показывается как `requires_probe`. Probe, preview и live test — отдельные commands с grants/deadline. Dry-run не резервирует mutation key: apply использует новый key и утверждённый plan hash, который повторно проверяется перед commit.

## Optimistic concurrency

Каждый mutable resource имеет revision. При mismatch API возвращает:

- expected/current revision;
- machine-readable conflict paths;
- безопасный rebase hint, если command commutative;
- ссылку на актуальный snapshot.

API не делает silent last-write-wins для workspace, grants или provisioning.

Readiness report отделён от structural validation: offline device/missing grant можно сохранить в draft. Apply разрешает unavailable routes только согласно явной policy и никогда не включает unknown required semantics. Перенос drifted draft поверх нового active base требует нового plan, даже если сам draft не менялся.

## Длительные операции

Install, apply, compatibility scan, replay и provisioning возвращают `OperationId`.

States:

`queued → running → succeeded | failed | cancelled | unknown_outcome`.

`cancel_requested` — промежуточный флаг, не доказательство отмены эффекта. После commit apply отмена возвращает `too_late`; требуется новый rollback apply. `succeeded` означает завершённую операцию; runtime health (`applied/degraded`) возвращается отдельно вместе с commit status. Recovery не переписывает failed external action в success без подтверждения.

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

Default scopes для Codex: read, draft и simulation; apply, plugin install, permission grant и provisioning требуют соответствующего scope и, где предусмотрено, operation-bound confirmation. Client с read/draft credential не может сам выдать себе apply/admin scope или получить confirmation token. Политика проверяется ApplicationService и одинакова для CLI, Codex, MCP и direct adapter.

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

Cursor включает `HostBootId`, topic и sequence. Начальная подписка атомарно связывает snapshot с первым delta после него, чтобы не было окна потери событий. Каждый delta содержит base/current sequence; gap, истечение retention или новый HostBootId возвращает `resync_required`. Client заменяет cache snapshot, а не применяет delta поверх неизвестной базы. Terminal operation result читается через query независимо от потерь progress subscription.

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
