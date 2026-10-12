# Модуль: Profile Store

**Ответственность:** infrastructure adapter, который надёжно хранит drafts, immutable revisions, package locks, commit/operation records, snapshots и assets по ports контекста [Workspace Configuration](workspace-configuration.md). Domain policy export/import и инварианты draft этому adapter не принадлежат. Actual grants находятся в отдельном host authorization store.
**Расположение:** `crates/deckweave-profiles/`; модели и repository port импортируются из `deckweave-workspace`.

## Публичный интерфейс

Реализованы `JsonProfileStore::compact_draft`, `compact_command_claims` и `archive_terminal_operations`. Checkpoint публикуется до удаления старых transaction snapshots; активные receipt snapshots и backup head сохраняются. GC допускается после подтверждённой ротации dedup epoch, а не только по wall-clock expiration. Pending apply защищает claim. Terminal operations переносятся атомарным rename в `operations/archive`, сохраняют historical query и не входят в обычный recovery scan. Оба commit heads защищены; revisions/assets не удаляются. Квоты используют rebuildable cached counts под mutation lock и сбрасывают cache после неопределённого write outcome. Это предполагает один adapter под существующим host process lock; внешние writers не поддерживаются.

Desktop выполняет archive/claim maintenance после recovery; quota pressure также запускает соответствующее освобождение. Автоматический checkpoint budget долгой editor session и ограничение размера archive ещё требуют acceptance в [6G.0](../../implementation/pre-phase-6.md). Archive сохраняется до явной retention/export policy; перемещение истории не ограничивает её общий объём на диске. Fault tests подтверждают process-level recovery, а не power-loss durability.

| Символ | Тип | Описание |
|---|---|---|
| `ProfileStore` | trait/service | Load/save/query revisions |
| `WorkspaceRepositoryAdapter` | adapter | Реализация repository ports Workspace Configuration |
| `WorkspaceRevisionRecord` | persisted document | Stored config envelope/checksum |
| `SnapshotRef` | struct | Named pointer/comment |
| `AssetId` | content hash | Deduplicated asset |
| `ExportSelection` | struct | Public/private content selection |
| `MigrationReport` | struct | Pure transform result |
| `ApplyCommitRecord`, `MutationReceipt` | documents | Crash recovery и durable deduplication |
| `ApplyOperationRepository` | port/adapter | Immutable apply intent, status, active commit и bounded recovery scan |
| `MutationWindow` | document | Выданный Host срок действия dedup epoch для principal |
| `CommandClaimRepository` | port/adapter | Общий durable scope `(principal, epoch, key)` для всех application mutations |
| `PackageLock` | document | Exact versions/hashes/settings schemas |

## Зависимости

| Модуль | Что использует |
|---|---|
| Domain & Protocol | IDs/document schemas |
| Workspace Configuration | Repository ports и persisted DTO |
| Storage adapter port | Atomic replace/flush/lock contract, без зависимости от OS keychain |

Maintenance перед новой mutation выполняется каждые 256 подтверждённых изменений draft (настраиваемо), а также при receipt quota. Он завершается до нового commit, поэтому его сбой не маскирует уже принятую mutation. Внутри действующей dedup epoch receipts сохраняются; quota исчерпания сообщает backpressure. GC допускается только после выдачи новой persisted epoch, а не по wall clock. В тесте `growth` 256 mutations и четыре открытия store сохраняют постоянный write budget (4 обычные atomic writes, до 6 при checkpoint), действующий replay и ограниченный размер transaction/receipt set при ротации epoch. Это проверка алгоритма и process restart, не benchmark latency физических накопителей.

Terminal archive ограничен `maximum_archived_apply_operations` (по умолчанию 100 000). При заполнении старые записи остаются в hot store; после его quota новые apply отклоняются с capacity error. Автоматическое удаление истории запрещено; явный export/purge с проверкой выбранных revisions — отдельный административный use case. Cached directory accounting восстанавливается при открытии store и предполагает один host writer под существующим data-root lock.

## Инварианты

- Applied revision не изменяется in place.
- Сначала сохраняются revision/assets/receipts, затем единый commit pointer; atomic replace одного файла не заменяет multi-document protocol.
- Undo/redo относится к draft commands.
- Rollback создаёт новую revision.
- Secret values отсутствуют в profile/assets/default export.
- Migration pure и сохраняет pre-migration backup/report.
- Revision ID уникален; checksum проверяет bytes, rollback создаёт новый ID даже для прежнего content.
- Acknowledged draft mutation durable; undo stack v1 может потеряться после restart с явным сообщением.
- Mutation key глобален для `(principal, epoch, key)`, а не локален draft/apply use case; Gateway резервирует opaque command fingerprint через общий claim до dispatch.
- До публикации draft head сохраняется immutable pending intent; recovery либо допубликовывает candidate/receipt, либо диагностирует divergence.
- Apply intent сохраняется отдельно от status; единственный `active/commit.json` является точкой выбора revision. Перед сменой head status прежнего commit запечатывается; startup различает активный, superseded committed и never-committed intent. Лимит recovery считается по pending set, а terminal history обходится потоково.
- Receipt lookup адресуется хешем полного mutation key и не сканирует draft history; receipt/pending имеют явные quotas.
- Import не переносит grants и не сопоставляет CredentialRef локальному secret автоматически.
- GC учитывает revisions/snapshots/drafts/operations/preview; missing migration сохраняет opaque settings.
- Runtime artifacts jobs не относятся к profile assets и не участвуют в snapshot/export по умолчанию; promotion — отдельная явная операция через ArtifactBroker.

## Намеренно НЕ обрабатывает

- domain commands и JSON mutation semantics;
- ownership инвариантов WorkspaceDraft;
- runtime apply;
- plugin execution;
- OS keychain implementation;
- event history как event sourcing.

## Заметки для агента

> JSON документы — source of truth. SQLite допустим только как rebuildable index/cache. Проверять hash assets и сохранять unknown optional fields. Нормативны [storage/apply](../../contracts/lifecycle.md#сохранение-и-apply) и [pure migrations](../../contracts/lifecycle.md#миграции-и-неизвестные-данные); fault tests охватывают каждую точку записи и disk full.

Global ledger разделён на `mutations/windows`, общие `mutations/claims`, draft-specific `mutations/pending` и `mutations/receipts`. Последовательность document mutation: command claim → immutable transaction → pending intent → draft head → immutable receipt → удаление pending. Любая операция store сначала восстанавливает bounded pending set. Это recovery protocol для process crash. Gate E0 добавил в storage port явные `NotPublished`/`Unknown`, повторное чтение target после replace error и сохранение staged bytes при невозможности сверки. Power-loss guarantees остаются отдельными от process-crash evidence и зависят от filesystem/устройства.
