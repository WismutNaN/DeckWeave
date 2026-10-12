# База идей плагинов

Это проектный каталог: пользовательские сценарии, границы переиспользования и условия реализации. Это не runtime registry, магазин packages или обещание включить все integrations в v1. Идентификатор `PLG-NNN` сохраняется при разделении/переименовании идеи; окончательные package IDs назначаются при реализации. Один package может содержать несколько contributions, один сценарий — использовать несколько packages и возможностей ядра.

Источник: идеи пользователя 2026-10-12 и ранее принятые [сценарии](../product/scenarios.md), [требования](../product/requirements.md), [roadmap](../implementation/roadmap.md). Во всех карточках ниже статус — **проектирование**, если явно не указано иное. Существующие virtual fixtures не являются реализацией перечисленных integrations. Техническая оценка ядра: [проверка реализуемости](../research/plugin-feasibility.md); обязательный следующий шаг: [gate до Фазы 6](../implementation/pre-phase-6.md).

## Реестр

«Ранний» означает кандидат для первых providers после gate, а не изменение согласованного P0/v1 scope. «Контракт» означает доказательство общей платформы до реальной интеграции. Последовательность реализации задаёт roadmap, детализацию — карточка.

| ID | Идея | Форма и очередь |
|---|---|---|
| [PLG-001](platform.md#plg-001) | Приложения, окна, foreground, иконки, переключение, tray | Windows provider; ранний, tray отдельно |
| [PLG-002](platform.md#plg-002) | Пресеты расстановки окон на мониторах | Layout provider + PLG-001; после window API |
| [PLG-003](platform.md#plg-003) | CPU/RAM/GPU и другие показатели | Metrics provider + общие presenters; ранний |
| [PLG-004](platform.md#plg-004) | Виртуальные клавиши, сочетания, Unicode, вставка | Platform output provider; ранний |
| [PLG-005](platform.md#plg-005) | PowerShell и другие scripts/process jobs | Isolated action provider; ранний |
| [PLG-006](platform.md#plg-006) | Настраиваемые HTTP-запросы | Action provider + общий SDK transport; ранний |
| [PLG-007](platform.md#plg-007) | MQTT publish/subscribe | Shared connection provider; ранний после service binding |
| [PLG-008](platform.md#plg-008) | Дата, часы, таймеры | Host clock + time provider/templates; контракт до 6 |
| [PLG-009](integrations.md#plg-009) | SingularityApp: задачи сегодня, действия, голосовое создание | Connector + collection view + capture/transcription |
| [PLG-010](integrations.md#plg-010) | Погода сейчас и прогноз | Weather connector + shared presenters |
| [PLG-011](productivity.md#plg-011) | Pomodoro и интервалы работы/отдыха | Timer policy provider + PLG-008 |
| [PLG-012](integrations.md#plg-012) | Zotero: поиск и открытие PDF с прогревом приложения | Connector + host interaction |
| [PLG-013](integrations.md#plg-013) | Запись голоса в ограниченный audio artifact | Capture job provider |
| [PLG-014](integrations.md#plg-014) | Handy: внешний connector, позднее native transcription | Transcription provider; version gate |
| [PLG-015](integrations.md#plg-015) | Браузер: вкладки, ввод результата, ChatGPT-сценарий | Browser adapter; отдельный integration gate |
| [PLG-016](integrations.md#plg-016) | Почта: draft/send/receipt | Mail connector |
| [PLG-017](integrations.md#plg-017) | Telegram: события и доступная история уведомлений | Feed connector + collection view |
| [PLG-018](integrations.md#plg-018) | Sprut.hub: свет, яркость, устройства | Domain connector поверх PLG-007 |
| [PLG-019](productivity.md#plg-019) | Заметки, редактор, постоянные desktop overlays | Notes provider + host views |
| [PLG-020](productivity.md#plg-020) | Пипетка → палитра на кнопках | Capture interaction + palette storage; conformance |
| [PLG-021](productivity.md#plg-021) | Игра «поймай бобра» | Game provider + scene/input; conformance |
| [PLG-022](productivity.md#plg-022) | Общие карточки, шкалы, графики, списки и подменю | Scene templates/presenters; не API-specific UI |
| [PLG-023](productivity.md#plg-023) | Одно устройство переключает режим/экран другого | Automation recipe + scoped scene/layer state |
| [PLG-024](platform.md#plg-024) | Штатная клавиатура как trigger, observe/intercept | Bundled Platform Input; Фаза 7 и поздние extensions |
| [PLG-025](platform.md#plg-025) | Windows volume/mute/media/application launch | First-party platform contributions; Фаза 7 |
| [PLG-026](devices-and-bridges.md#plg-026) | Mirabox N4 Pro и Stream Deck-подобные поверхности | Device provider; Фаза 8, model evidence |
| [PLG-027](devices-and-bridges.md#plg-027) | Gamepad/HID, позже рули и force feedback | Второй hardware family; Фаза 10/позже |
| [PLG-028](devices-and-bridges.md#plg-028) | RGB/аналоговые клавиатуры, OpenRGB, geometry | Providers + lighting engine |
| [PLG-029](devices-and-bridges.md#plg-029) | Smart keyboard: onboard profiles/programming | Provisioning provider; после v1 |
| [PLG-030](devices-and-bridges.md#plg-030) | OpenAction/Elgato совместимость | Compatibility host; Фаза 14 |
| [PLG-031](devices-and-bridges.md#plg-031) | Companion actions/variables/feedback | Protocol bridge; после v1 |
| [PLG-032](devices-and-bridges.md#plg-032) | Remote panels, Steam Deck/Android, remote inputs | Remote agent; после v1 |
| [PLG-033](devices-and-bridges.md#plg-033) | DSP/виртуальный микрофон, CLAP/GStreamer/VST | Specialized media; отдельная программа |
| [PLG-034](devices-and-bridges.md#plg-034) | OBS | Action/state connector; ранее пример extension, без срока |
| [PLG-035](devices-and-bridges.md#plg-035) | MIDI | Protocol bridge; ранее кандидат, без срока |
| [PLG-036](devices-and-bridges.md#plg-036) | OSC | Protocol bridge; ранее кандидат, без срока |
| [PLG-037](feeds-and-personal.md#plg-037) | Общий unread inbox: индикатор, counts, список, autorotation | Aggregation provider + shared presenter |
| [PLG-038](feeds-and-personal.md#plg-038) | Mattermost и другие messaging services | Независимые feed connectors |
| [PLG-039](feeds-and-personal.md#plg-039) | Шаги, дневная цель, сон и health metrics | Health adapters + mobile bridge + shared metrics |
| [PLG-040](feeds-and-personal.md#plg-040) | Дни до даты, число/шкала/блоки | Date calculation provider/recipe + presenters |
| [PLG-041](feeds-and-personal.md#plg-041) | Ближайшие даты, дни рождения, контакты/календарь | Date-source adapters + next occurrence query |
| [PLG-042](feeds-and-personal.md#plg-042) | Новости: RSS/сайт/Telegram, заголовки и описания | Feed adapters + common collection presenter |
| [PLG-043](productivity.md#plg-043) | Переключение виджетов одной кнопки по кругу | Slot-scoped presentation state/recipe |

## Правила расширения

Карточка содержит пользовательский результат, contributions/данные, используемые общие части, ошибки/ограничения и приёмку. Не добавлять API доменного сервиса в core. Повторяемую бизнес-логику можно вынести в plugin, библиотеку SDK либо recipe: отдельный процесс нужен только при самостоятельной ответственности/connection/lifecycle. Зависимости и bindings определяет [контракт композиции](../contracts/plugin-composition.md).

Результат исследования добавляется в существующую карточку; новый файл заводится для самостоятельного семейства либо достаточно большой реализации. Когда package появляется, карточка получает ссылки на source, manifest, conformance report и проверенные platform/version. Идея, fixture и готовая integration — разные статусы. В реестре не хранят credentials, user account IDs или пользовательские заметки.
