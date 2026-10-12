# Локальный API DeckWeave

API — единственная application boundary для Desktop UI, CLI, Codex и optional MCP adapter. Он не повторяет plugin protocol: plugins предоставляют capabilities, а application API редактирует и наблюдает пользовательскую систему.

## Принципы

Текущий `ApplicationGateway` реализует queries `Host`, `Draft`, `PlanApply`, `Operation`, `RuntimeTrace` и `PluginCatalog`, commands `PatchDraft`/`ApplyDraft` и выдачу mutation window. Остальные группы ниже — целевой API, а не обещание уже существующих endpoints. Locale обслуживает отдельный `LocalePreferencesService` с тем же authorizer; Tauri/HTTP adapters обязаны получать principal от host.

`PluginCatalog` принимает точные package ID/version/artifact SHA-256, проверяет scope `Read` до обращения к index и возвращает bounded static schemas, units/ranges, ports/actions, contribution/group IDs, translations и manifest SHA-256. Это проекция того же проверяемого каталога, которым пользуется compiler; import/query не запускает plugin. Неверный hash, неоднозначный package и изменённый manifest отклоняются. Ответ не содержит executable paths, process handles или secret values. Schema shape в DTO — read model, не второй validator; `Json` не считается структурой для schema-form/field binding. Запрос не доказывает live availability и не перечисляет ещё не установленные packages.

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

`RuntimeTrace` v1 читается как bounded snapshot под scope `simulation`: он содержит plan/schema hashes, epoch, sequence, correlation/causation slots и redacted machine outcomes. Default query не возвращает signal/result payload, private text, audio или credentials; полная recording fixture является отдельной явно разрешённой диагностической операцией.

Data/result bindings, light scenes и notification view models редактируются тем же draft API, что и обычный graph. Система IDs различает application `OperationId`, root execution/`RunId`, step `InvocationId` и provider `JobId`. Receipt apply/launch не означает завершение workflow. Queries результатов снова проверяют актуальные grants; secret values не сохраняются в durable dedup receipt. [Automation contract](automation.md) определяет job/result/artifact semantics; API не создаёт второй scheduler.

## Реализованный Desktop adapter

`app_query`, `app_command`, `mutation_window`, `open_workspace`, `locale_get`, `locale_change`, `locale_reconcile` — Tauri entry points. Principal и trace создаются host из доверенного окна `main`; JavaScript не выбирает principal/scopes. Работа с Profile Store/compiler/runtime выполняется вне UI thread. Открытие `desktop-main` — авторизованный use case Draft: существующий ID возвращает текущий document/version, отсутствие создаёт seed с постоянным ID. Read queries не создают draft.

`EditorSnapshot` содержит bounded installed package version/hash + templates, registry revision и node generations/descriptors, runtime health и active selection. Это последовательные snapshots разных владельцев, не атомарный снимок всего runtime. `PluginCatalog` проверяет точные version/artifact hash. Failed metadata query не подменяется успешным пустым списком. Live refresh не меняет локальный draft или selection.

`DraftView` содержит package lock. `ReplaceConfiguration { document, package_lock }` меняет оба поля одним CAS/receipt; применение графа с новой action dependency не должно требовать промежуточной несовместимой записи. Обычный `ReplaceDocument` сохраняет lock. Editor preview metadata находится в `GraphDocument.extensions["org.deckweave.editor"]` версии 1, а принадлежность node простому binding — в `node.extensions["org.deckweave.editorBinding"]`. Эти extensions не являются runtime resource handles.

Frontend хранит ещё не подтверждённый envelope до отправки в локальном recovery journal, изолированном WebView data-root. После transport failure/restart повторяет тот же envelope; проверка operation/active state не генерирует новую mutation. Даже definitive denied при replay не доказывает, что первый вызов не исполнился до потери ответа. CAS conflict сохраняет local/base/remote и не разрешает слепую замену чужой версии. Profile Store остаётся источником сохранённого draft; recovery journal не является вторым workspace store и не переносится в export.

## Mutation envelope

```json
{
  "requestId": "request-uuid",
  "idempotencyKey": "client-generated-uuid",
  "dedupEpoch": "server-issued-window-id",
  "dryRun": false,
  "command": {
    "type": "patch_draft",
    "draft_id": "desktop-main",
    "expected_version": 12,
    "edit": {
      "type": "set",
      "pointer": "/extensions/org.example.label",
      "value": "Основное"
    }
  }
}
```

Это текущий сериализованный DTO Rust/Tauri: поля envelope и read-model structs — camelCase, поля вариантов `AppQuery`/`AppCommand`/`DraftCommand` — snake_case. Preconditions принадлежат выбранной command, отдельного универсального поля `preconditions` в v1 нет.

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

Ключ дедупликации — `(PrincipalId, dedup_epoch, idempotency_key)`, payload fingerprint включает command kind, target и preconditions. Gateway до dispatch атомарно публикует общий durable claim; draft receipt и apply operation не создают раздельных namespaces. Повтор с тем же содержимым возвращает прежний receipt/OperationId **до** проверки устаревшей revision; другой payload или другой command kind с тем же ключом возвращает `idempotency_conflict`. Актуальная авторизация проверяется и при чтении старого результата, secret payload не сохраняется в receipt.

Host выдаёт persisted `dedup_epoch` с `expires_at`; v1 хранит receipts не менее семи дней и до завершения pending operation. Истёкшее окно возвращает `idempotency_expired`, а не исполняет старый запрос как новый. Restart не обнуляет действующие окна и keys. Новое окно/ключ означают новую операцию и не являются способом автоматически повторить неизвестный side effect.

Клиент получает или возобновляет окно отдельным авторизованным вызовом `open_mutation_window`; caller не выбирает epoch. Пока окно действует, повторный вызов возвращает то же значение. В текущем single-user Host одновременно хранится одно активное окно на principal; после expiry новый вызов атомарно публикует следующее. `dry_run` окна не требует и ключ не резервирует.

Store фиксирует intent до side effect, а для document mutations публикует результат и receipt через единый commit protocol. При crash между внешним side effect и подтверждением outcome становится `unknown_outcome`. API dedup не обеспечивает exactly-once на USB, process launch или чужом HTTP API; повтор там возможен только по [execution contract](lifecycle.md#executions-cancellation-и-смена-плана).

Если mutation response содержит и receipt, и document, document соответствует `after_version` и checksum именно этого receipt, включая replay после последующих изменений. Repository возвращает согласованный результат одной mutation; отдельный `get_draft` после записи не обеспечивает это свойство. Текущее состояние запрашивается самостоятельным query. Fingerprint включает target draft/workspace: смена target с тем же ключом — conflict в общем scope principal/epoch, а не новая операция в другом локальном журнале.

Retention receipts не равна undo history. Checkpoint/compaction draft сохраняет действующие dedup windows и pending operations; quota возвращает отдельную диагностируемую ошибку до записи. Нельзя обходить переполнение истории новым ключом или считать истёкший receipt отсутствующим разрешением повторить side effect.

### Переход с раннего profile format

Drafts, revisions и transaction chain, созданные до global ledger, остаются читаемыми. Старые caller-chosen epochs не считаются выданными Host и возвращают `idempotency_expired`: клиент запрашивает новое окно и новый key. Старые per-draft receipts намеренно не реконструируются в глобальный индекс автоматически, потому что без полного сканирования всех drafts нельзя доказать отсутствие коллизии одного ключа на разных targets.

`dry_run`, `validate`, `plan` и default `simulate/replay` не запускают packages, network requests, process actions и hardware output. Они используют static manifests, snapshots и fake targets; отсутствие live evidence показывается как `requires_probe`. Probe, preview и live test — отдельные commands с grants/deadline. Dry-run не резервирует mutation key: apply использует новый key и утверждённый plan hash, который повторно проверяется перед commit.

`PlanApply` и `ApplyDraft` передают одинаковый `allow_disruption`. Значение входит в plan hash, idempotency payload, durable operation intent и commit record. Если compiler объявил disruptive handoff, default `false` отклоняет plan; consent к одному plan нельзя перенести на другой hash или добавить только в момент commit.

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

Phase 6 editor additions: `ActiveDocument` returns the persisted active selection and
its document under Read authorization for the review diff. Compiler validation errors
include bounded `diagnostics` entries `{code,nodeId,field}` in the existing error DTO;
field values, credentials and private payloads are excluded. A disruptive planning
request without consent returns `api.error.disruption_required`; the editor must request
a fresh plan with explicit consent, then apply that plan's exact preconditions.
