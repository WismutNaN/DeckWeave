# Сквозные сценарии и проверки расширяемости

Сценарии проверяют архитектуру на разнородных устройствах. Это acceptance specifications, не описание работающих integrations. Общий граф и fixtures входят в v1; реальные Handy/browser/mail/Telegram packages и production DSP имеют отдельные gates в [плане](../implementation/roadmap.md#дальнейшие-программы-после-v1). Детали wire и runtime принадлежат [контрактам](../README.md#контракты).

## Кнопка → Handy → обработка текста → вкладки браузера

Пользователь подтвердил, что под «Hendly» имелся в виду Handy. В изученном source revision есть headless `--transcribe-file … --json` для WAV и отдельный интерактивный toggle. Это не утверждение о наличии флагов в установленном у пользователя release; provider проверяет поддержку версии ([источники и ограничения](../research/implementation-review.md#handy-и-результаты-действий)).

```text
Нажатие → capture job → WAV artifact → Handy transcription job
                                      ↓ typed text result
                        optional text processor / script
                                      ↓ validated text
                  create browser tabs → scoped tab handles → fill composer
                                      └→ optional clipboard copy
```

Пользователь выбирает способ окончания записи (release, повторное нажатие, silence policy либо лимит); событие stop относится к конкретному capture job. Headless Handy получает конечный 16 kHz mono 16-bit PCM WAV и заранее установленную модель. Result stdout парсится по schema, stderr используется как bounded redacted diagnostics. Завершение процесса и valid result необходимы перед следующим шагом. Неподдерживаемый CLI или отсутствующая модель останавливают ветвь с понятным report; скрытой загрузки модели нет.

Interactive toggle запускает/останавливает состояние Handy, но не предоставляет correlation/result contract для DeckWeave run. Такой adapter отдельно маркируется best-effort, требует подтверждённого механизма получения результата и не выдается за надёжный job. Чтение чужой history DB или «sleep → взять clipboard» не являются default интеграцией.

Browser provider открывает выбранные URL и возвращает session-scoped tab handles. Следующий шаг проверяет нужный origin, tab identity и доступность поля ввода; затем заполняет текст. Отправка сообщения — отдельный action. Реальная поддержка ChatGPT web зависит от browser extension/automation adapter и актуальной страницы; она не является обещанием универсального DOM API. Если доступна только OS-вставка, нужен проверяемый foreground target и явный degraded mode; при смене фокуса ветвь прекращается. При использовании clipboard restore возможен только если clipboard всё ещё содержит записанное этим run значение.

Проверки: два нажатия не смешивают transcript и tab handles; пустой transcript не вызывает дальнейший action без выбранной политики; cancel/revoke/timeout останавливает continuation; private audio/text отсутствуют в profile/export/log. Transcription result не интерполируется в shell command: executable и argv передаются отдельно. Пользовательский code исполняется isolated provider с объявленными inputs/outputs и grants.

## Кнопка → письмо, уведомления → Stream Deck

Mail action принимает типизированные recipient/subject/body/attachments и возвращает provider receipt, а не просто успешную HTTP-доставку запроса. Если подтверждение потеряно, состояние unknown не вызывает повторную отправку без provider idempotency. Режим draft/send выбирается в конфигурации. Получатели и account входят в scope; credentials остаются у host broker/provider.

Telegram integration отдельно предоставляет arrival events, paginated history, unread state и acknowledge action. Пользователь выбирает аккаунт/чаты, доступные данному API. Scene показывает карточку, scroll/page controls и stale/gap marker. Открытие/рендеринг карточки не означает «прочитано». Неполная доступность истории явно видна; поток уведомлений не перегружает очередь нажатий. Fixtures: reconnect с пропуском событий, edit/delete, cursor expiry, rate limit и confidential data redaction. Реальный connector выбирается отдельным spike; общий контракт не обещает доступ ко всем сообщениям любого аккаунта.

## Клавиатура → волна света, затемнение и восстановление

Provider объявляет позиции keys/LEDs и их mapping. Пользователь выделяет keys, назначает wave по координатам, добавляет правило press → dim → cooldown → fade. Без geometry предлагается пользовательская раскладка либо static effect. Длительности задаются явно; повторное нажатие перезапускает выбранный envelope. Смена слоя меняет базовую palette, не создаёт конкурирующего USB writer. Правила shared LED, frame drop и preview описаны в [lighting contract](../contracts/lighting.md).

Fixtures используют «обычную» observe-only клавиатуру, аналоговую клавиатуру с travel sensor, RGB array с несовпадающей input matrix и smart device с аппаратными presets. Отсутствие firmware API не мешает host automation; unsupported interception/onboard programming отображается отдельно. Физическая input key без RGB не получает фиктивную light capability.

## Удержание → временный слой на другом устройстве

Нажатие keyboard key создаёт layer token для группы устройств; энкодер Stream Deck временно меняет назначение, а его light/display показывает активный слой. Release исходной key освобождает тот же token. Disconnect клавиатуры, change mapping во время hold и два одновременных owners не оставляют stuck layer/output. Observe mode не подавляет ввод в ОС; selective suppression требует отдельной actual capability.

## Клавиатура ноутбука → поиск Zotero → PDF

Bundled Platform Input provider наблюдает назначенную комбинацию в pass-through mode и создаёт нормализованное edge event. Automation вызывает `zotero.search`, затем открывает host-owned `InteractionSession`. Desktop overlay получает focus, показывает строку поиска и bounded страницы `ItemRef`; Zotero provider отвечает за запрос, metadata и attachment references, но не за окно.

После выбора terminal typed result возвращается только исходному invocation. Следующий step вызывает `zotero.open_attachment` и открывает/focuses конкретный PDF. `Escape`, закрытие окна, timeout, потеря provider или удалённый item завершают session явным cancel/error и не открывают случайный последний результат. Две быстрые горячие клавиши имеют разные session IDs и не смешивают query, selection и continuation. Если OS hook не различает физические клавиатуры, UI честно показывает source как «Системная клавиатура».

Reference fixture использует fake search provider, проверяет keyboard navigation, pagination, cancel и concurrent sessions. Реальный Zotero adapter получает отдельный version/API gate; private query/history не попадают в profile или default logs.

## Микрофон → обработка → виртуальный endpoint

Media graph согласует rate/channel layout, выбирает filter и virtual sink. Control graph меняет gain/bypass и читает meters. Audio buffers остаются в media plane. Fixtures: plugin crash, hotplug, sample-rate change, grant revoke и unavailable virtual endpoint. До отдельного backend/driver gate это P2; запись файла для Handy может появиться раньше и не считается low-latency microphone replacement.

## Матрица доказательств

| Сценарий | Требования | Доказательство общего контракта | Gate реальной интеграции |
|---|---|---|---|
| Typed chain и Handy | DW-AU-011, DW-AU-012, DW-AU-014 | Два isolated fake jobs, artifacts, bounded result, cancel в Фазах 5/11 | Version-pinned Handy + capture + browser adapter после v1 |
| Mail и Telegram | DW-AU-011, DW-AU-013, DW-AU-014 | Unknown mail outcome и paginated fake feed в Фазе 11 | Account/API limits и privacy fixtures отдельного package |
| RGB wave/cooldown | DW-SF-009, DW-SF-010 | Irregular virtual keyboard и fake clock в Фазах 9/11 | RGB provider с подтверждённой geometry/transport |
| Слои и разные inputs | DW-AU-003, DW-AU-010, DW-WS-007 | Owner cleanup, mixed descriptor fixtures в Фазах 2/3/11 | Suppression/onboard features имеют отдельный platform gate |
| Keyboard → Zotero interaction | DW-AU-015, DW-PL-008, DW-IN-007, DW-IN-008 | Bundled fake input + concurrent search sessions в Фазах 7/13 | Version-pinned Zotero provider и OS keyboard evidence |
| Audio graph | DW-MD-001, DW-MD-004 | Negative negotiation/descriptor fixtures в Фазе 2 | Backend/latency/virtual endpoint ADR после v1 |

До стабилизации SDK нужны несколько независимых fake providers, а не один универсальный fixture с закодированными знаниями о всех устройствах. Они проверяют возможность добавить capability без special-case в core; hardware tests позже подтверждают физическую реализацию.
