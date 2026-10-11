# Модуль: Profile Store

**Ответственность:** хранит drafts, immutable revisions, named snapshots, assets, migrations, grants references и selective export/import.
**Расположение:** `crates/deckweave-profiles/`.

## Публичный интерфейс

| Символ | Тип | Описание |
|---|---|---|
| `ProfileStore` | trait/service | Load/save/query revisions |
| `WorkspaceDraft` | mutable document | Undoable editing state |
| `ProfileRevision` | immutable document | Stored/applied config |
| `SnapshotRef` | struct | Named pointer/comment |
| `AssetId` | content hash | Deduplicated asset |
| `ExportSelection` | struct | Public/private content selection |
| `MigrationReport` | struct | Pure transform result |

## Зависимости

| Модуль | Что использует |
|---|---|
| Domain & Protocol | IDs/document schemas |
| OS credential adapter | Secret references only |

## Инварианты

- Applied revision не изменяется in place.
- Atomic replace + backup защищают от partial write.
- Undo/redo относится к draft commands.
- Rollback создаёт новую revision.
- Secret values отсутствуют в profile/assets/default export.
- Migration pure и сохраняет pre-migration backup/report.

## Намеренно НЕ обрабатывает

- runtime apply;
- plugin execution;
- OS keychain implementation;
- event history как event sourcing.

## Заметки для агента

> JSON документы — source of truth. SQLite допустим только как rebuildable index/cache. Content-addressed assets проверяются hash при load/import. Unknown optional fields нельзя молча удалять.

