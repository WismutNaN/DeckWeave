# Модуль: Desktop Shell

**Ответственность:** предоставляет Tauri tray application и Svelte editors поверх ApplicationService без владения runtime resources.
**Расположение:** `apps/desktop/`, `ui/desktop/`.

Текущий shell подключает production Gateway/runtime и locale service; canvas и базовый inspector реализованы в первом срезе Фазы 6; Graph/Surface editors и полный RuntimePanel остаются целевыми компонентами. Порядок реализации: [adapter и snapshots → selection → inspector → apply](../../implementation/phase-6-desktop.md). UI получает static catalog через `PluginCatalog`, configured instances через draft, live availability через registry projection; direct plugin/filesystem access не требуется.

## Публичный интерфейс

| Символ | Тип | Описание |
|---|---|---|
| `DesktopAdapter` | client adapter | Typed ApplicationService access |
| `WorkspaceCanvas` | UI component | Infinite device canvas |
| `Inspector` | UI component | Progressive settings |
| `GraphEditor` | UI component | Advanced automation |
| `SurfaceEditor` | UI component | Scene/hit-region editing |
| `RuntimePanel` | UI component | Health/permissions/conflicts |

## Зависимости

| Модуль | Что использует |
|---|---|
| Local API/ApplicationService | Все data/mutations |
| Domain generated UI types | Snapshots/commands |

## Инварианты

- UI не открывает devices и plugin pipes.
- Closing window не вызывает host shutdown.
- Каждая mutation имеет expected revision.
- Long operation отображается по OperationId.
- ru/en text не используется как identity.
- Основной сценарий assignment ≤ 3 действия.
- Apply progress различает prepare/barrier/commit и degraded health; после commit доступен новый rollback, а не фиктивный cancel.

## Намеренно НЕ обрабатывает

- graph execution;
- profile file IO;
- plugin permissions enforcement;
- actual rendering hardware frame.

## Заметки для агента

> Canvas и graph virtualized. Runtime events не должны попадать в глобальное Svelte state без throttle. External API changes отображаются diff, а не silently overwrite current draft.
