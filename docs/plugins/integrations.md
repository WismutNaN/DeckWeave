# Интеграции приложений и сервисов

Статус карточек — проектирование; реальные account/device/version проверки ещё не выполнены. [Реестр](README.md), [композиция](../contracts/plugin-composition.md), [исследование API](../research/plugin-feasibility.md#external-evidence). Данные сервиса предоставляет connector; общий presenter, picker, capture и input output переиспользуются отдельно. Сроки новых integrations не подменяют v1 roadmap.

<a id="plg-009"></a>
## PLG-009 — SingularityApp

- **Сценарий:** задачи сегодня на кнопках, сортировка по времени, короткое время/название; нажатие открывает подменю «выполнить / удалить / завтра / назад». Quick create принимает один заголовок, а голосовой вариант получает title/description из PLG-013/014.
- **Contributions:** bounded task query, connection/freshness state, create/complete/remove/reschedule operations. UI использует общую collection navigation PLG-022 и stable task ID, не порядковый номер кнопки.
- **Политика:** «сегодня/завтра» определяется timezone аккаунта; задачи без времени имеют явное место сортировки. Date/time, длительность и deadline — разные поля. Recurring instance/series различаются. Delete содержит выбранный target и policy подтверждения, возврат не применяется к уже другому item после обновления списка.
- **Границы:** прямой REST connector использует общий HTTP SDK, установка PLG-006 не обязательна. При read-only token mutation unavailable. Polling с backoff/freshness нужен, пока API не предоставляет change subscriptions. Completion/remove и recurrence проверяются по закреплённой версии API; uncertain create нельзя повторять как новый task.
- **Приёмка:** более одной страницы, два аккаунта, midnight/timezone change, task удалён извне, список пересортирован перед click, потерян ответ после create, offline. Прототип fake tasks до Фазы 6 доказывает identity/binding; реальный connector приходит после collection/interaction foundations. [Официальный API](https://singularity-app.com/wiki/api/).

<a id="plg-010"></a>
## PLG-010 — Погода

- **Сценарий:** выбранный регион; текущая и ощущаемая температура, текущие осадки, температура и осадки через заданный интервал.
- **Contributions:** location instance → normalized weather state/forecast query. Provider mapping отдельно от PLG-022 number/icon/chart templates и от PLG-006 transport.
- **Данные:** координаты/region label, timezone, observation/forecast timestamp, units, freshness, valid interval. Вероятность осадков (%), количество (mm) и интенсивность (mm/h) — разные показатели; отсутствующее поле не превращается в ноль. Прогноз на `now + offset` использует объявленную temporal resolution.
- **Приёмка:** несколько мест, stale/offline, смена единиц, недоступный forecast horizon, rate limit, ошибочная geocoding selection. Конкретный API/лицензия и attribution выбираются в package spike; [Open-Meteo](https://open-meteo.com/en/docs) — исследуемый кандидат, не уже выбранная обязательная зависимость.

<a id="plg-012"></a>
## PLG-012 — Zotero → PDF

- **Сценарий:** сразу открыть search overlay и принимать текст, одновременно запуская/прогревая Zotero; после выбора/Enter открыть конкретный PDF. Не терять уже введённый запрос, пока приложение ещё не готово.
- **Contributions:** readiness/probe, search page, attachments query, open attachment. Host Interaction Runtime владеет окном; provider — Zotero/Better BibTeX API. Query generation заменяет устаревший поиск; loading/error не блокирует ввод.
- **Переиспользование:** keyboard trigger PLG-024, app launch PLG-025, generic picker, typed ItemRef/AttachmentRef и icon/asset broker. Item identity — library + item key, не только citekey; несколько PDF требуют выбора/явной политики. Отсутствующий локальный PDF, external reader и Zotero reader имеют отдельный availability report.
- **Референс:** изучен локальный `G:/TYPST/zotst` — BBT JSON-RPC, debounce, generation guard, attachment open URI. Брать transport/search mapping после license/provenance проверки, не переносить VS Code QuickPick и citation/BibTeX workflow. [Точная версия и ограничения](../research/plugin-feasibility.md#zotst).
- **Приёмка:** медленный startup, query до ready, новый query до старого ответа, Enter без валидного selection, два sessions, several attachments, cancel при startup. Открытие разрешено лишь для выбранного актуального attachment; никакого «последнего результата» по глобальной переменной.

<a id="plg-013"></a>
## PLG-013 — Запись голоса

Ранее запланированный bounded capture job: start/stop с выбранной политикой release/toggle/silence/limit, progress/state и конечный WAV artifact. Микрофон выбирается отдельно; hotplug, cancel, privacy, size/duration quota и формат — явные. Artifact получает read lease для transcription provider; запись не попадает в profile/export по умолчанию. Это конечная запись, не virtual microphone/DSP PLG-033. Приёмка: два run, stop не своего job, тишина, device lost, quota и cleanup.

<a id="plg-014"></a>
## PLG-014 — Handy / transcription

Пользователь ранее уточнил: Handly/Hendly означает **Handy**. Первый путь — connector к отдельно установленному, version-pinned Handy с проверенным file → typed text результатом. Capture PLG-013 и downstream task/browser не встраиваются внутрь connector. Interactive toggle остаётся best-effort до доказанного механизма correlation/result; ожидание clipboard по таймеру не подходит.

Второй путь — native transcription package с переиспользованием ASR libraries, если он даст измеримую пользу. Это отдельный вариант той же transcription capability, не загрузка всего desktop приложения Handy в host. Проверяются licenses code/models/backend, model availability и resource budgets. Замена реализации сохраняет declared input/result schema. [Уже изученный pinned source](../research/implementation-review.md#handy-и-результаты-действий). Приёмка: отсутствующая модель/CLI, empty text, cancel, corrupt WAV, две записи и bounded stderr. Secret/private transcript не логируется.

<a id="plg-015"></a>
## PLG-015 — Browser automation

Ранее описанная цепочка Handy → transform → открыть вкладки → заполнить ChatGPT composer. Provider возвращает scoped tab handles, проверяет origin/target и отдельные permissions. Fill и send — разные operations; неизвестная страница/DOM возвращает unsupported. PLG-004 clipboard/input fallback допускается только с проверенным foreground и отдельным degraded report. Приёмка: две вкладки/два run, закрытие target, смена focus, cancel. [Сквозной сценарий](../product/scenarios.md#кнопка--handy--обработка-текста--вкладки-браузера).

<a id="plg-016"></a>
## PLG-016 — Почта

Typed recipient/subject/body/attachments, draft/send, provider receipt; account и allowed recipients входят в scope. Использует HTTP/SDK, artifacts и общий form при необходимости, не вводит email-специфичные nodes в ядро. Приёмка: lost response без повторной отправки, revoke, invalid attachment и provider-defined idempotency. Доставка HTTP не считается доставкой письма. [Сценарий](../product/scenarios.md#кнопка--письмо-уведомления--stream-deck).

Отдельная read-only contribution предоставляет unread messages/threads, counts, bounded previews и deep links для [общего inbox](feeds-and-personal.md#plg-037). Mail read, send и mark-read scopes независимы. Count unit и semantics archive/delete/read соответствуют конкретному provider; просмотр карточки не меняет mailbox state.

<a id="plg-017"></a>
## PLG-017 — Telegram

Arrival/edit/delete events, unread/freshness state и bounded history query; acknowledge — отдельный action. Connector выбирает доступный account/API mode, не обещает все сообщения любого аккаунта. PLG-022 отображает feed и navigation; чтение/рендеринг не меняет read status само. Приёмка: reconnect gap, deleted item, cursor expiry, rate limit и privacy. [Общий collection contract](../contracts/automation.md#уведомления-и-коллекции).

Unread lists/counters и channel news используют тот же connector с разными filters; агрегация находится в [PLG-037](feeds-and-personal.md#plg-037), news presentation — в [PLG-042](feeds-and-personal.md#plg-042). User read state требует подтверждённого account API; bot event stream не подменяет пользовательский inbox.

<a id="plg-018"></a>
## PLG-018 — Sprut.hub

- **Сценарий:** кнопка включает/выключает свет, encoder меняет яркость; другие characteristics добавляются schemas/mappings.
- **Contributions:** discovery/collection доступных accessories, normalized state и domain commands. Service binding к PLG-007 MQTT использует выбранную broker instance; topics/prefix/characteristics остаются конфигурацией adapter.
- **Переиспользование:** transport не дублируется. UI отображает declared unit/range; encoder delta преобразуется в абсолютную brightness с clamp/rate policy. Commanded value и reported device state различаются, timeout оставляет pending/unknown.
- **Приёмка:** reconnect/retained state, duplicate publish, device offline, изменение capability, две лампы и два brokers. Все доступные устройства не получают права на запись автоматически. Официальный MQTT bridge и `/set` подтверждены, точные topic/payload mapping ещё требуют sample конкретного hub: [источник](https://wiki.spruthub.ru/Создание_моста_MQTT).
