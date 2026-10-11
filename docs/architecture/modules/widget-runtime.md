# Модуль: Widget Runtime

**Ответственность:** lazily исполняет custom web property editors и surface widgets в изолированном контексте и возвращает scoped messages/frames. Desktop palettes/dialogs с typed user result принадлежат отдельному [Interaction Runtime](interaction-runtime.md).
**Расположение:** `runtimes/widget-host/`.

## Публичный интерфейс

| Символ | Тип | Описание |
|---|---|---|
| `WidgetHost` | process/service | Isolated web runtime |
| `WidgetInstance` | handle | Package/context-scoped instance |
| `WidgetInput` | message | Allowed state/pointer/settings |
| `WidgetFrame` | bitmap/layer | Bounded visual output |
| `WidgetHealth` | snapshot | FPS, memory, errors |

## Зависимости

| Модуль | Что использует |
|---|---|
| Plugin Supervisor | Package/session/grants |
| Protocol/frame port | Выдаёт WidgetFrame; consumer compositor подключён composition root, compile dependency на него отсутствует |
| Local API broker | Только scoped settings operations |

## Инварианты

- Никаких privileged Tauri commands.
- Origin/token scoped package+instance.
- Random URL path не является origin isolation; storage partition и bridge authorization проверяются отдельно.
- Network/filesystem только через granted broker.
- Frame/input rate и memory bounded.
- Closing Desktop WebView не останавливает active widget host.

## Намеренно НЕ обрабатывает

- standard text/icon/date widgets;
- hardware output;
- automation execution;
- arbitrary browser navigation.
- host-owned interaction windows и user-selection lifecycle.

## Заметки для агента

> Не запускать widget host, если active scenes используют только built-in scene nodes. Точный embeddable browser backend выбирается отдельным spike с Windows/Linux tests.
