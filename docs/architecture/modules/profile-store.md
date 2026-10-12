# Модуль: Profile Store

**Ответственность:** infrastructure adapter, который надёжно хранит drafts, immutable revisions, package locks, commit/operation records, snapshots и assets по ports контекста [Workspace Configuration](workspace-configuration.md). Domain policy export/import и инварианты draft этому adapter не принадлежат. Actual grants находятся в отдельном host authorization store.
**Расположение:** `crates/deckweave-profiles/`; модели и repository port импортируются из `deckweave-workspace`.

## Публичный интерфейс

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
| `MutationWindow` | document | Выданный Host срок действия dedup epoch для principal |
| `PackageLock` | document | Exact versions/hashes/settings schemas |

## Зависимости

| Модуль | Что использует |
|---|---|
| Domain & Protocol | IDs/document schemas |
| Workspace Configuration | Repository ports и persisted DTO |
| Storage adapter port | Atomic replace/flush/lock contract, без зависимости от OS keychain |

## Инварианты

- Applied revision не изменяется in place.
- Сначала сохраняются revision/assets/receipts, затем единый commit pointer; atomic replace одного файла не заменяет multi-document protocol.
- Undo/redo относится к draft commands.
- Rollback создаёт новую revision.
- Secret values отсутствуют в profile/assets/default export.
- Migration pure и сохраняет pre-migration backup/report.
- Revision ID уникален; checksum проверяет bytes, rollback создаёт новый ID даже для прежнего content.
- Acknowledged draft mutation durable; undo stack v1 может потеряться после restart с явным сообщением.
- Mutation key глобален для `(principal, epoch, key)`, а не локален одному draft; target входит в fingerprint.
- До публикации draft head сохраняется immutable pending intent; recovery либо допубликовывает candidate/receipt, либо диагностирует divergence.
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

Global ledger разделён на `mutations/windows`, `mutations/pending` и `mutations/receipts`. Последовательность document mutation: immutable transaction → pending intent → draft head → immutable receipt → удаление pending. Любая операция store сначала восстанавливает bounded pending set. Это recovery protocol для process crash; неоднозначные результаты OS replace и power-loss guarantees отдельно закрывает gate E0.
