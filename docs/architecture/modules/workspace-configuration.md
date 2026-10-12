# Модуль: Workspace Configuration

**Контекст:** bounded context редактируемой и версионируемой конфигурации.  
**Расположение:** `crates/deckweave-workspace/`; filesystem adapter остаётся в `crates/deckweave-profiles/`.

## Ответственность

- `WorkspaceDraft` aggregate и его commands;
- optimistic `DraftVersion`, undo/redo session и validation delta;
- создание immutable `WorkspaceRevision` и package lock;
- named snapshots, rollback-as-new-draft;
- политика selective export/import, private placeholders и migrations;
- ports для durable draft/revision/receipt repositories.

## Публичная граница

| Символ | Роль |
|---|---|
| `WorkspaceDraft` | aggregate root |
| `DraftCommand` | единственный путь изменения draft |
| `DraftVersion` | optimistic concurrency value |
| `WorkspaceRevision` | immutable published document |
| `DraftRepository` | application port для draft persistence/dedup |
| `ExportPolicy` / `ImportReport` | переносимая конфигурация без grants/secrets |

## Инварианты

- acknowledged command увеличивает version ровно один раз и имеет durable receipt;
- revision не меняется после публикации;
- rollback создаёт новый draft и затем новую revision;
- unknown optional extension fields сохраняются round-trip;
- secret values и actual grants не входят в aggregate;
- storage failure не превращает частично записанный документ в опубликованную revision.

## Зависимости

Контекст зависит от Shared Kernel types и собственных repository ports. Он не импортирует host lifecycle, filesystem adapter, plugin supervisor или UI.

R2 перенёс модели draft/revision/snapshot, validation, pure mutation, правила export selection и `DraftRepository`.
Profiles сохраняет только archive encoding, checksums, append-only receipts, recovery и filesystem layout; persisted JSON не изменён.

## Намеренно не обрабатывает

- выбор active revision и apply barrier;
- авторизацию principal;
- запуск/остановку plugins;
- filesystem layout и atomic replace implementation.
