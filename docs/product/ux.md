# UX-концепция DeckWeave

Desktop UI является редактором и наблюдателем long-lived host. Он не владеет device handles, не исполняет automation и может быть закрыт без остановки applied workspace.

Нормативные требования к визуальной плотности, осмысленности каждого элемента, empty/data states и приёмке экранов заданы в [стандарте качества интерфейса](ui-quality-standard.md). При конфликте декоративного решения с рабочими данными приоритет имеет стандарт.

Целевое поведение уточнено пользователем 2026-10-13: [live editing и взаимодействие](../decisions/0022-live-editing-and-interaction.md), [самостоятельные Windows plugins](../decisions/0023-composable-windows-plugins.md). Текущий UI Фазы 6 ещё использует ручной Apply; новые правила ниже являются задачами реализации, а не описанием уже готовых функций.

## Принципы этого интерфейса

- **Устройства — на главном экране.** Пользователь начинает с цифрового представления реального рабочего места, а не со списка плагинов.
- **Одно действие — одна модель.** Drag-and-drop, inspector и graph редактируют один subgraph, а не отдельные «простые» и «экспертные» конфигурации.
- **Частое — без отдельного подтверждения.** Выбрать control, назначить action, при необходимости заполнить параметры; валидная настройка начинает работать автоматически.
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
│ DeckWeave  Workspace: Основной  ● Applied  [Undo] [Redo] [Пауза live]    │
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

**Всегда видно:** workspace, applied/draft status, undo/redo, состояние live editing, общий health и selection inspector.

**Скрыто до запроса:** raw descriptors, protocol versions, stdout, USB traces, API tokens и media negotiation.

## Общее оформление и переопределения controls

Настройки устройства задают базовый фон; настройка конкретного control имеет приоритет только для переопределённого свойства. Изменение общей палитры сохраняет overrides. У кнопки показано «Наследуется от устройства» или «Собственное правило» и действие «Сбросить»; reset возвращает наследование, не подставляет копию прежнего цвета.

Градиент рассчитывается на общей плоскости устройства и вырезается по bounds каждого control. Отдельный одинаковый gradient на каждой кнопке не является общим фоном. Положение приходит из descriptor плагина; если оно неизвестно, UI показывает список элементов с явной пометкой и не предлагает пространственный градиент для оборудования. Произвольные images/assets и анимации добавляются через Surface/Lighting contracts, а не как path из WebView.

Первый срез Фазы 6 сохраняет оформление в desired editor layout и подписывает его «Макет». Для actual output необходим device renderer и подтверждённый output contract. Наличие красивого preview не означает отправку кадра или изменение RGB устройства.

## Готовые элементы и свободная композиция

Из библиотеки перетаскиваются actions, widgets и presets. Готовый элемент открывается как редактируемая композиция: фон, изображения, текстовые блоки с переменными, позиции/размеры, порядок слоёв, условия видимости и gestures. Настройки данных и действия отделены от оформления. «Готовый виджет» не означает закрытую картинку с единственным полем имени.

Кнопка с экраном и область общей панели используют одну визуальную модель в пределах capabilities. Несенсорная панель допускает отображение и управление с других controls; touch editor предлагается только при соответствующем input endpoint. Неподдерживаемые жесты/форматы объясняются, а не имитируются молча. Общие переменные задаются один раз и доступны нескольким виджетам через явные bindings.

Общий фон — одно изображение или композиция в координатах устройства; кнопки могут содержать только текст поверх него. Локальный override меняет только выбранные свойства. Фон обрезается и кодируется для отдельных экранов provider-ом; неизвестная геометрия требует настройки или честного ограничения. Изменение одного виджета не требует нового пользовательского «рабочего стола» и не сбрасывает остальные.

## Мгновенное редактирование

Оформление, triggers и действия применяются автоматически после валидного изменения. UI различает «работает», «обновляется», «не применено: причина»; прежняя рабочая версия остаётся активной, пока новая не принята. Частично введённый путь/число и несвязанный обязательный вход не становятся работающим binding. Одно перетаскивание — одна операция undo; hardware preview обновляется с bounded rate и latest-wins, независимо от durable save/compile.

Редактирование не запускает action для проверки и не воспроизводит старый press или retained state как новый trigger. Для временно незавершённой сложной правки доступна явная пауза live editing; это необязательный инструмент, не обязательная стадия каждой настройки. Вызов action для теста обозначается отдельно. Preview/simulation и production input сохраняют разные права и ownership.

## Ручная и адаптивная панель

В настройках устройства или связанной группы выбирается manual либо adaptive mode. Пользователь может назначить кнопку включения/выключения adaptive mode. В manual выбранная панель сохраняется при смене foreground. В adaptive сразу проверяется актуальное активное окно: совпавшее правило выбирает панель, отсутствие совпадения возвращает стандартную панель. Видимое неактивное окно не удерживает профиль.

Несколько совпавших правил имеют видимый порядок приоритетов. Автопереключение относится к выбранному scope; смена одного slot и его виджета остаётся локальной операцией. Включение режима и выбор существующей панели не создают новых workspace revisions.

## Модификаторы, запуск и ошибки

Кнопку можно назначить модификатором и задать solo action и сочетания. Solo выполняется при отпускании, если кнопка не участвовала в сочетании; использование chord не выполняет следом solo. Удержание, повтор, отмена и смена слоя обрабатываются одной gesture-моделью. Это не даёт права блокировать обычное действие клавиатуры Windows: observe/intercept выбираются отдельно по возможностям provider.

Пользователь подтвердил выбор для каждого назначения обычной клавиатуры: «Пропускать в приложение» по умолчанию, «Перехватывать» — явная настройка. Пока provider не поддерживает interception, второй вариант недоступен с объяснением; сохранённое требование interception не понижается молча до pass-through.

Готовая кнопка приложения переключается на уже открытое приложение; запуск выполняется при его отсутствии. Если найдено несколько окон, исходный вариант — последнее использованное подходящее; конкретное окно можно выбрать отдельным действием. Это правило выбора среди окон пока является implementation default, а не отдельно подтверждённым предпочтением пользователя.

Ошибка automation даёт короткое всплывающее сообщение с названием действия и причиной, останавливая зависимую цепочку. Автоматического повтора нет. Details доступны по запросу; popup не требует разбора графа и не исчезает бесследно из diagnostics. Повторяющиеся одинаковые сообщения группируются. Уже выполненные внешние действия не откатываются от закрытия popup или undo настройки.

## Центральные редакторы

| Редактор | Основная задача | Вход |
|---|---|---|
| Workspace | Расстановка devices/groups и быстрые assignments | Главный экран |
| Graph | Conditions, fan-out, sequence/parallel, layers, state, errors | «Advanced» у binding или toolbar |
| Surface | Scene objects, widgets, hit regions, animations, preview | Выбор display/surface |
| Device Studio | Discovery, calibration, traces, descriptor project | Device menu/первый запуск |
| Runtime | Sessions, queues, conflicts, permissions, metrics | Status bar/diagnostics |
| Plugin Library | Install/update/rollback/catalog/compatibility | Library |
| Interaction overlay | Поиск, picker, quick form и подтверждение во время automation | Горячая клавиша/action |

Редакторы могут открываться вкладками. Selection сохраняется при переходе Workspace → Graph → Surface.

## Progressive disclosure

### Уровень 1: быстрое назначение

1. Выбрать кнопку/энкодер/region.
2. Перетащить action или выбрать найденный.
3. Заполнить обязательные параметры, если они есть; валидное изменение применяется автоматически.

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
3. При необходимости заполняет обязательные параметры. Дополнительного «Применить» нет.

**Количество действий:** 2 для готового preset, ещё настройка параметров при необходимости.

### Сценарий: изменить параметры готового binding

**Частота:** высокая.

1. Выбрать control.
2. Изменить значение в inspector; валидное изменение применяется автоматически.

**Количество действий:** 2; validation inline, без modal и отдельного подтверждения.

### Сценарий: собрать N4 Pro surface

**Частота:** высокая на этапе настройки.

1. Открыть display в Surface editor.
2. Добавить/переместить text, icon, image, group или widget.
3. Отдельно нарисовать hit regions либо применить template.
4. Уточнить положение численно или сдвигом на пиксель; подключённый поддерживаемый output показывает изменения сразу.

Grid является snap overlay и может быть отключён. Между regions нет принудительных пустых pixels.

### Сценарий: добавить сложную логику

**Частота:** средняя.

1. Открыть binding в Graph.
2. Добавить condition/layer и parallel/sequence group.
3. При необходимости задать явную fallback/continue ветвь; default — остановка с сообщением об ошибке, без автоматического retry.
4. Для сборки незавершённого сценария можно приостановить live editing и проверить simulation.
5. После возобновления валидный граф применяется автоматически.

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

### Сценарий: найти материал Zotero горячей клавишей

**Частота:** высокая для исследовательской работы.

1. Пользователь нажимает назначенную комбинацию на штатной клавиатуре.
2. Компактный host-owned overlay получает focus; ввод сразу попадает в поиск.
3. Стрелки перемещают selection, `Enter` открывает PDF, `Escape` отменяет session.

Overlay показывает provider/status, поддерживает pagination и не открывает результат до явного выбора. При повторном запуске sessions не перезаписывают друг другу query/result. Недоступный Zotero оставляет понятную ошибку в overlay и trace, не блокируя остальные bindings.

## Interaction overlays

- Окно, focus, placement, timeout и accessibility принадлежат DeckWeave.
- Базовые list/search/form/confirm views рендерятся host без custom web runtime.
- Plugin передаёт schema, bounded data pages и typed result; custom view запускается только при необходимости в sandboxed runtime.
- Overlay полностью управляется клавиатурой, имеет видимый focus и не перехватывает глобальный ввод после закрытия.
- Закрытие и `Escape` дают explicit cancel; пустой selection не считается success.
- Это runtime interaction, а не редактор workspace: результат не записывается в profile без отдельного действия.

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
- Автоматический Apply создаёт immutable revision; ручное подтверждение не требуется для обычной настройки.
- Named snapshot может ссылаться на любую сохранённую revision и иметь comment.
- Rollback создаёт новую revision на основе старой; история не переписывается.
- External API mutation появляется как отдельная undoable command с actor/source.
- Export/import не заменяет undo.

Подтверждённые draft edits сохраняются до ответа API; после restart последняя draft version восстанавливается, а отсутствие in-memory undo history показывается явно. Apply progress различает prepare, возможный downtime/handoff, commit и health. До commit доступен cancel; после commit — новый rollback apply. Export/rollback не выдаёт grants и не переносит secrets по совпадению имени.

Отдельный тестовый hardware preview имеет TTL/stop и lease. Apply отзывает конфликтующий preview, а stop восстанавливает текущую applied scene. Touch во время preview тестирует hit regions в editor и не запускает production actions. Simulation использует fake targets.

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
| Применить вручную | `Ctrl+Enter` — текущий UI Фазы 6; в live editing не требуется |
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
