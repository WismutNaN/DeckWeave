# Общие ленты, даты и персональные показатели

Статус — проектирование. Добавлено по уточнению пользователя 2026-10-12. [Реестр](README.md), [общее представление](../contracts/presentation.md), [проверенные API](../research/plugin-feasibility.md#external-evidence). Connector получает данные, aggregation/calculation формирует общий результат, presenter отображает его на любой surface или в host-owned desktop view.

<a id="plg-037"></a>
## PLG-037 — Общий список непрочитанного

- **Сценарии:** Telegram/почта/Mattermost/другие accounts; точка «есть непрочитанное», число по сервису/аккаунту, общее число, общий лист с ручным или timed перелистыванием. Пользователь выбирает filters, muted channels и показываемые private previews.
- **Contributions:** connectors PLG-016/017/038 выдают normalized read-only feed и unread summary; отдельный aggregation provider/recipe объединяет разрешённые sources. PLG-022/043 предоставляет list/badge/counter/carousel, не знает API сервиса.
- **Данные:** `(source, account, item ID)`, item revision, timestamps, title/snippet, target reference, declared read/ack operations. Message count, thread count, chat count и mentions — разные units. Единого точного total не бывает при смешанных units или unavailable source: UI показывает breakdown/partial/stale, а не ложный ноль.
- **Семантика:** render, auto-rotation и open preview не помечают remote message прочитанным. Mark-read/ack — отдельная scoped operation с тем смыслом, который допускает сервис (item/thread/channel/read-through cursor). «Скрыть локально» отличается от «прочитать в сервисе». OS notification stream не считается полной unread history.
- **Приёмка:** сообщение прочитано с телефона, удалено/отредактировано, reconnect gap, duplicate item, два accounts, один source offline/revoked и read race. Курсор общего списка хранит source cursors/watermarks с bounded expiry; нельзя загружать всю историю для каждого repaint. Дедупликация одинакового текста из разных сервисов по умолчанию запрещена.

<a id="plg-038"></a>
## PLG-038 — Mattermost и другие messaging connectors

Mattermost connector хранит server/account instance, выдаёт разрешённые channel/post data, unread/mention summary и отдельные read-state operations, если API/permissions это поддерживают. REST snapshot + events/polling выбираются по версии сервера; webhook новых сообщений сам по себе не доказывает unread state пользователя. Bot identity и user account имеют разную доступность; не обещать чужие private channels.

Новые сервисы реализуют тот же feed contract независимо, без branch по provider name в renderer или core. Приёмка: server offline, удалённый channel, thread/channel count semantics, revoke, resync. Проверяется против version-pinned instance; [официальный API](https://docs.mattermost.com/api) — исходная точка, real integration ещё не выполнена.

<a id="plg-039"></a>
## PLG-039 — Health metrics и дневная цель

- **Сценарии:** пройденные шаги, осталось до выбранной цели, прогресс, требуемый темп до конца дня; сон и доступные scores/другие показатели. Источник может быть Samsung Health или другой provider.
- **Разделение:** source adapter/phone bridge → normalized personal metric series → bounded calculation → PLG-022 gauge/chart/number. Прогноз не привязан к Samsung schema. Identity включает человека/источник/устройство/time interval; перекрывающиеся телефон/часы не суммируются без выбранной aggregation policy.
- **Расчёт:** `remaining = max(goal - observed_steps, 0)`; требуемый темп = remaining / оставшееся выбранное время, если оно положительно. Оценка «успею» требует явно заданного или измеренного темпа и freshness; выводится как оценка с исходными допущениями. После конца дня — отдельное состояние. Это не медицинский вывод из sleep/energy score.
- **Границы Samsung:** официальный Data SDK перечисляет steps/sleep/Energy score, работает на Android; для desktop нужен отдельный мобильный adapter и authenticated pairing. Production access имеет registration/permission gate. Health Connect — альтернативный путь для доступных ему records, без обещания всех vendor scores. Не называть Energy score универсальным «качеством жизни»; неизвестный показатель требует точного API mapping. [Источники](../research/plugin-feasibility.md#external-evidence).
- **Приёмка:** sync delay, смена timezone/дня, revoked phone permission, user switch, corrected history, duplicate interval, нет цели/score, телефон offline. Health content private: minimised read scopes, bounded encrypted transport/storage по выбранному adapter, no default trace/export, screen-lock redaction. Raw phone data не отправляются в сторонний облачный сервис без отдельной настройки.

<a id="plg-040"></a>
## PLG-040 — Countdown до даты

Target date/time + timezone, optional start date и выбранные units. Number показывает оставшиеся calendar days либо duration — это разные режимы, особенно при DST. Fill/block view требует start/end interval и direction (`elapsed`/`remaining`); без start показывается только доступная метрика. Past target получает expired/completed/negative режим по настройке, не деление на ноль или вечный переполненный progress.

Общий clock PLG-008 и date math переиспользуются Pomodoro/important dates, PLG-022 рисует number/gauge/blocks. Приёмка: today, past date, start=end, leap day, timezone/clock jump; обновление раз в нужную границу времени, а не каждый frame. Переключение clock↔countdown на одной кнопке — PLG-043.

<a id="plg-041"></a>
## PLG-041 — Ближайшая важная дата

Sources: вручную заданные dates, read-only contacts birthdays, calendar events/anniversaries. Contacts и calendar — отдельные adapters/permissions; общий occurrence query выдаёт ближайшую дату/несколько совпавших с stable person/event reference и timezone. Birth year может отсутствовать: возраст не выдумывается. Recurrence, Feb 29, all-day event и timezone policy явны.

PLG-040 рассчитывает расстояние до occurrence, PLG-022 показывает карточку/список. Query «следующая» не требует копировать всю адресную книгу в профиль; индекс/cache bounded/private. Приёмка: несколько sources одного контакта, deleted/updated birthday, нет разрешения, нет дат, одинаковые даты, recurring event exception. Выбор Apple/Google/Microsoft/local provider и реального API — отдельный package spike.

<a id="plg-042"></a>
## PLG-042 — Новости и заголовки

Независимые RSS/Atom/JSON feed adapters, explicit site adapter и Telegram-channel source преобразуют записи в общую NewsItem collection: source/item ID, title, короткое описание, publication/update time, safe link и optional scoped image. Website scraping не является универсальным API и не обещается для любого сайта; политика источника и формат проверяются отдельно.

Reused feed query/cache/paging, PLG-037 aggregation и PLG-022/043 presentation; news viewed state локальный и не равен remote messenger read receipt. HTML/remote markup санитизируется в text/разрешённый model, scripts не исполняются. Auto-scroll/rotation останавливается при взаимодействии, скрытии или выборе item. Приёмка: duplicate/update, malformed feed, oversized description/image, unavailable source, сортировка с неполным временем и link открывается только отдельным действием. Full article text не копируется по умолчанию.
