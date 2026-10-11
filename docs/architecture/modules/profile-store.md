# Модуль: Profile Store

**Ответственность:** хранит drafts, immutable revisions, package locks, commit/operation records, snapshots, assets, migrations и selective export/import. Actual grants принадлежат отдельному host authorization store.
**Расположение:** `crates/deckweave-profiles/`.

## Публичный интерфейс

| Символ | Тип | Описание |
|---|---|---|
| `ProfileStore` | trait/service | Load/save/query revisions |
| `WorkspaceDraft` | mutable document | Undoable editing state |
| `WorkspaceRevision` | immutable document | Stored/applied config |
| `SnapshotRef` | struct | Named pointer/comment |
| `AssetId` | content hash | Deduplicated asset |
| `ExportSelection` | struct | Public/private content selection |
| `MigrationReport` | struct | Pure transform result |
| `ApplyCommitRecord`, `MutationReceipt` | documents | Crash recovery и durable deduplication |
| `PackageLock` | document | Exact versions/hashes/settings schemas |

## Зависимости

| Модуль | Что использует |
|---|---|
| Domain & Protocol | IDs/document schemas |
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
- Import не переносит grants и не сопоставляет CredentialRef локальному secret автоматически.
- GC учитывает revisions/snapshots/drafts/operations/preview; missing migration сохраняет opaque settings.
- Runtime artifacts jobs не относятся к profile assets и не участвуют в snapshot/export по умолчанию; promotion — отдельная явная операция через ArtifactBroker.

## Намеренно НЕ обрабатывает

- runtime apply;
- plugin execution;
- OS keychain implementation;
- event history как event sourcing.

## Заметки для агента

> JSON документы — source of truth. SQLite допустим только как rebuildable index/cache. Проверять hash assets и сохранять unknown optional fields. Нормативны [storage/apply](../../contracts/lifecycle.md#сохранение-и-apply) и [pure migrations](../../contracts/lifecycle.md#миграции-и-неизвестные-данные); fault tests охватывают каждую точку записи и disk full.
