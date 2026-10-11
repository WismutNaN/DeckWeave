# UX-концепция DeckWeave

Desktop UI является редактором и наблюдателем long-lived host. Он не владеет device handles, не исполняет automation и может быть закрыт без остановки applied workspace.

## Принципы этого интерфейса

- **Устройства — на главном экране.** Пользователь начинает с цифрового представления реального рабочего места, а не со списка плагинов.
- **Одно действие — одна модель.** Drag-and-drop, inspector и graph редактируют один subgraph, а не отдельные «простые» и «экспертные» конфигурации.
- **Частое — за три действия.** Выбрать control, назначить action, применить.
- **Draft не маскируется под runtime.** Видно, что сохранено, применено, изменено и degraded.
- **Внешний вид редактируется на preview.** Visual nodes и touch regions перемещаются руками, имеют numeric inspector и snap templates.
- **Диагностика рядом с объектом.** Missing permission, conflict, stale state и crash видны у конкретного device/node.
- **Цвет не единственный сигнал.** Icon/shape/text/status дополняют цветовую кодировку.
- **Плотность CAD/GIS.** Компактные панели, dockable inspectors, минимум декоративного пустого пространства.
- **Опасное обратимо или отделено.** Preview имеет stop/timeout; provisioning не смешивается с apply.
- **RU/EN с начала.** Layout не рассчитывает на длину одного языка; stable IDs не зависят от текста.

## Главный экран

```text
┌──────────────────────────────────────────────────────────────────────────┐
│ DeckWeave  Workspace: Основной  ● Applied  [Undo] [Redo] [Применить]    │
├──────────────┬────────────────────────────────────────┬──────────────────┤
│ Library      │ Infinite workspace canvas              │ Inspector        │
│ Search       │                                        │ Selection        │
│ Actions      │   ┌──────── N4 Pro ────────────────┐   │ Binding          │
│ States       │   │ [10 keys] [touch surface]      │   │ Appearance       │
│ Templates    │   │ [encoder × 4 + rings]          │   │ Feedback         │
│ Plugins      │   └─────────────────────────────────┘   │ Advanced ▸       │
│              │        ┌── Gamepad ──┐                  │                  │
│              │        │ buttons/axes│                  │                  │
├──────────────┴────────────────────────────────────────┴──────────────────┤
│ Runtime: 2 devices · 3 packages · p95 3.2 ms · 0 edge drops · 1 warning │
└──────────────────────────────────────────────────────────────────────────┘
```

**Всегда видно:** workspace, applied/draft status, undo/redo, apply, общий health и selection inspector.

**Скрыто до запроса:** raw descriptors, protocol versions, stdout, USB traces, API tokens и media negotiation.

## Центральные редакторы

| Редактор | Основная задача | Вход |
|---|---|---|
| Workspace | Расстановка devices/groups и быстрые assignments | Главный экран |
| Graph | Conditions, fan-out, sequence/parallel, layers, state, errors | «Advanced» у binding или toolbar |
| Surface | Scene objects, widgets, hit regions, animations, preview | Выбор display/surface |
| Device Studio | Discovery, calibration, traces, descriptor project | Device menu/первый запуск |
| Runtime | Sessions, queues, conflicts, permissions, metrics | Status bar/diagnostics |
| Plugin Library | Install/update/rollback/catalog/compatibility | Library |

Редакторы могут открываться вкладками. Selection сохраняется при переходе Workspace → Graph → Surface.

## Progressive disclosure

### Уровень 1: быстрое назначение

1. Выбрать кнопку/энкодер/region.
2. Перетащить action или выбрать найденный.
3. Применить.

Создаётся именованный collapsed subgraph с default error/feedback policy.

### Уровень 2: inspector

Показывает trigger, action parameters, appearance и feedback. Advanced fields свернуты по секциям, но доступны без смены глобального режима.

### Уровень 3: graph

Тот же binding раскрывается в nodes. Изменения немедленно отражаются в inspector summary. Возврат к простому виду не удаляет сложные nodes; summary показывает «Сложный сценарий».

## Ключевые сценарии

### Сценарий: назначить действие control

**Частота:** высокая.

1. Пользователь выбирает control на device preview.
2. Перетаскивает action/preset.
3. Нажимает «Применить» или `Ctrl+Enter`.

**Количество действий:** 3.

### Сценарий: изменить параметры готового binding

**Частота:** высокая.

1. Выбрать control.
2. Изменить значение в inspector.
3. Применить.

**Количество действий:** 3; validation inline, без modal.

### Сценарий: собрать N4 Pro surface

**Частота:** высокая на этапе настройки.

1. Открыть display в Surface editor.
2. Добавить/переместить text, icon, image, group или widget.
3. Отдельно нарисовать hit regions либо применить template.
4. Включить preview и применить.

Grid является snap overlay и может быть отключён. Между regions нет принудительных пустых pixels.

### Сценарий: добавить сложную логику

**Частота:** средняя.

1. Открыть binding в Graph.
2. Добавить condition/layer и parallel/sequence group.
3. Задать error policy.
4. Запустить simulation по recorded fixture.
5. Применить.

Graph показывает logical order и pending async boundaries.

### Сценарий: установить plugin

**Частота:** низкая.

1. Найти catalog entry или выбрать local artifact.
2. Просмотреть source, license, runtimes, permissions и conformance.
3. Установить.

Package остаётся Indexed до runtime/discovery lease. Для поиска первого устройства пользователь включает scoped discovery; появление candidate не открывает exclusive/output доступ автоматически.

### Сценарий: восстановиться после ошибки

**Частота:** низкая, критичная.

1. Status bar показывает degraded item.
2. Клик открывает Runtime с причиной и затронутыми routes.
3. Пользователь retry/disable/rollback package или возвращает workspace snapshot.

Остальные routes продолжают работать, если их dependencies доступны.

### Сценарий: управление через Codex

**Частота:** средняя для владельца проекта.

1. Codex создаёт draft через API.
2. UI показывает external changes и diff.
3. Пользователь или authorized workflow запускает simulation/apply.

Файлы не меняются за спиной runtime; revision conflict видим обеим сторонам.

## Infinite workspace canvas

- Device cards используют provider-defined outline/layout metadata или generic geometry.
- Zoom/pan, minimap и fit selection обязательны.
- Физический scale не обязан совпадать между devices; optional real-size mode доступен позже.
- Groups задают логическую композицию без слияния device identity.
- Routes могут отображаться только для selection, чтобы не превращать canvas в «лапшу».
- Presence/status overlays не меняют device geometry.
- Keyboard navigation перемещает selection между elements.

## Surface editor

```text
┌──────── Assets/Widgets ───────┬──────────── Canvas ───────────┬─ Inspector ─┐
│ Text                          │  visual scene                 │ Position    │
│ Icon/Image                    │  ┌─────────────────────────┐  │ Size        │
│ Shape/Group                   │  │ arbitrary composition   │  │ Binding     │
│ Standard widgets             │  │ ┈ independent hit zones │  │ States      │
│ Plugin widgets               │  └─────────────────────────┘  │ Animation   │
├───────────────────────────────┴───────────────────────────────┴─────────────┤
│ [Visual] [Hit regions] [Touch trace] [Preview ▶] [Stop ■]  FPS/dirty stats │
└─────────────────────────────────────────────────────────────────────────────┘
```

Visual and interaction layers can be viewed together or separately. Touch trace shows raw point, transformed point and matched region.

## Graph editor

Result pins подписаны типом и именем поля; соединение результата transcription с browser input видно отдельно от trigger edge. Inspector показывает input/result/error schema, run scope, timeout, reentrancy и cancellation policy. Run timeline различает accepted, running, succeeded, failed, cancelled, skipped и unknown. Приватные поля скрыты по умолчанию; expired artifact нельзя открыть как живой результат. Настройка arbitrary code предлагает isolated script node с input/output schema, не скрытое исполнение выражения внутри UI.

Notification view позволяет выбрать account/filter и листать bounded pages, показывает freshness/разрыв истории. Acknowledge/send являются отдельными действиями. Полные примеры: [сценарии](scenarios.md).

Visual roles:

```text
[Source] → [Transform] → [Condition / Layer] → [Sequence / Parallel] → [Target]
     └───────────────────────────────────────→ [Feedback] ────────────────┘
```

- Port shape/icon and label supplement color.
- Структурно несовместимый edge отклоняется с объяснением schema/unit mismatch. Offline device/missing grant сохраняет draft edge и показывает readiness issue до apply.
- Auto converter is inserted as visible node.
- Cycle требует положительного delay либо нового внешнего события и execution budget; один latch не разрешает loop.
- Group can be collapsed and named as reusable template.
- Error policy and async boundary are visible.
- Simulation timeline highlights executed/skipped/failed nodes.
- Recorded event can be dragged onto source test input.

## Device Studio

### Геометрия и свет

Редактор различает input elements, физические позиции, адресуемые LEDs и UI layout. Пользователь выбирает группу keys, preset wave и правило dim/cooldown/fade; duration и retrigger policy видны в inspector. Unknown geometry предлагает calibration/mapping, а missing LED показывает отсутствие возможности. Preview имеет Stop/TTL. Host rendering, аппаратный preset и persistent programming отображаются раздельно; запись памяти не запускается редактором animation. Подробные правила: [lighting contract](../contracts/lighting.md).

Touch inspector показывает, предоставлен ли только coordinate sample или подтверждённый contact lifecycle. Hold/drag недоступны для sample-only provider; synthetic approximation помечается отдельно. Screen target и raw→logical transform видны до live preview.

### Standard discovery

Показывает known HID/gamepad inputs, live values, calibration и generic descriptor. Пользователь может сохранить declarative mapping без Rust code, если standard protocol достаточен.

### Protocol lab

Записывает input reports и результаты только безопасных, явно выбранных commands. Unknown output reports не отправляются автоматически. Session имеет stop, rate limit и diagnostic warning.

```text
┌──────── Interfaces ──────┬──────── Live events ───────┬──── Descriptor ───┐
│ HID collection 01       │ Button 4: pressed          │ Element: key-4    │
│ Bluetooth gamepad       │ Gyro Z: 0.18 rad/s         │ motion.angular   │
│ Vendor interface        │ Report 03: …               │ schema/range     │
└──────────────────────────┴─────────────────────────────┴───────────────────┘
```

## Plugin Library

Package card показывает:

- publisher/source/repository/license;
- installed/available version и update channel;
- runtime groups и external runtimes;
- contributions;
- permissions diff;
- supported OS/architecture;
- conformance/compatibility level;
- active instances, process/memory/health;
- rollback version.

Отдельную contribution можно disable без удаления package.

## Draft, undo и snapshots

- `Ctrl+Z/Ctrl+Shift+Z` работают внутри current draft.
- Apply создаёт immutable revision.
- Named snapshot может ссылаться на любую сохранённую revision и иметь comment.
- Rollback создаёт новую revision на основе старой; история не переписывается.
- External API mutation появляется как отдельная undoable command с actor/source.
- Export/import не заменяет undo.

Подтверждённые draft edits сохраняются до ответа API; после restart последняя draft version восстанавливается, а отсутствие in-memory undo history показывается явно. Apply progress различает prepare, возможный downtime/handoff, commit и health. До commit доступен cancel; после commit — новый rollback apply. Export/rollback не выдаёт grants и не переносит secrets по совпадению имени.

Hardware preview имеет TTL/stop и отдельный lease. Apply отзывает конфликтующий preview, а stop восстанавливает текущую applied scene. Touch во время preview тестирует hit regions в editor и не запускает production actions. Simulation использует fake targets.

## Состояния интерфейса

| Состояние | Что показываем |
|---|---|
| Первый запуск | Virtual demo, «подключить устройство», «установить plugin» |
| Нет hardware | Virtual nodes и Device Studio, не пустой canvas |
| Discovery | Incremental candidates без блокировки UI |
| Indexed plugin | Contributions видны, runtime = not running |
| Missing permission | Resource, reason, requested scope, grant/deny |
| Draft invalid | Inline issue + переход к node/field |
| Applying | Prepare/barrier/commit progress, affected routes и объявленный downtime; прежняя revision работает до barrier |
| Applied | Ненавязчивое confirmation + revision |
| Degraded | Затронутые ports/routes и продолжающие работу части |
| Device disconnected | Card остаётся, ports серые, reconnect status |
| Stale state | Last value + age + warning |
| Plugin crash | Last error, restart count, retry/disable/logs |
| Revision conflict | Diff/base/current и варианты reopen/reapply |
| Empty search | Причина filters и clear action |

## Клавиатурные сокращения

| Действие | Сочетание |
|---|---|
| Глобальный поиск | `Ctrl+K` |
| Сохранить draft | `Ctrl+S` |
| Применить | `Ctrl+Enter` |
| Undo/redo | `Ctrl+Z` / `Ctrl+Shift+Z` |
| Открыть Graph выбранного binding | `G` |
| Открыть Surface выбранного display | `V` |
| Simulation start/stop | `F5` / `Shift+F5` |
| Stop preview/dangerous session | `Esc`, затем явная toolbar action при focus conflict |
| Runtime | `Ctrl+Shift+R` |

Сочетания локализуются только в label, не в storage. Global hotkeys не включаются без отдельного OS permission.

## Производительность UI

- Device/event updates поступают snapshots/deltas с throttle.
- Canvas virtualizes offscreen cards/nodes.
- Graph layout не пересчитывается на каждое runtime event.
- Preview frames не проходят через Svelte state tree.
- WebView closure не влияет на host.
- Large trace открывается paginated/streamed, а не целиком.
