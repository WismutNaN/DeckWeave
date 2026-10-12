# Фаза 6: application adapter и Desktop editor

Входной [gate 6G.0–6G.5](pre-phase-6.md) закрыт на `328cebf`. Фаза 6 начата: реализован первый рабочий вертикальный срез desktop editor. Общий gate фазы ещё открыт; подтверждённые результаты и оставшиеся проверки перечислены ниже. Её результат — пользователь связывает virtual control с action через общий API, видит ошибки и применяет draft; закрытие окна не останавливает runtime. Один срез занимает ориентировочно 1–5 дней; если acceptance не помещается, отдельно выделяются backend query, adapter и UI. Нормативные правила остаются в [Application API](../contracts/application-api.md), [границе plugin](../contracts/plugin-boundary.md) и [стандарте UI](../product/ui-quality-standard.md).

## Состояние первого среза, 2026-10-12

- **Реализовано:** общий Tauri adapter с host-derived principal и blocking pool; installed/registry/runtime projections, открытие текущего desktop draft, атомарная замена document + package lock. UI получает стабильные DTO и локализует ошибки по code.
- **Реализовано:** canvas с pan/zoom/fit и отсечением устройств за viewport; selection по полному ID, библиотека/search/drag-and-drop, последовательные action bindings в общем GraphDocument, record/unit actions и рекурсивные schema fields. Неоднозначный input выбирается явно; boolean input имеет режимы true/false/каждое событие. Переходы между шагами используют реальные result schemas; UI не конкатенирует JSON для передачи данных.
- **Реализовано:** общий solid/gradient фон в editor preview, геометрическое распределение градиента, подпись/цвет текста/фон отдельной кнопки, явное наследование и reset. Это desired layout, без вывода кадров или RGB на hardware.
- **Реализовано:** локальные undo/redo с CAS при сохранении, save/plan/apply, отдельные draft/active/runtime states, журнал восстановления WebView по data-root. Неотправленные изменения переживают restart. Неизвестный outcome удерживает исходные epoch/key/payload; replay не превращается в новый Apply. Отказ авторизации при повторе не доказывает отсутствие прежнего эффекта. Трёхстороннее объединение независимых полей не заменяет конфликтующие graph arrays молча.
- **Реализовано:** RU/EN preference, per-key fallback, locale reconcile, keyboard arrows по geometry, Ctrl+S/Ctrl+Enter, локальные Ctrl+Z/Ctrl+Shift+Z. Сохранённые настройки existing configured instances показываются раздельно по instance ID.
- **Проверено:** 13 frontend logic tests, fixture от настоящего TS builder → Rust compiler/scheduler → первый action → result binding следующего action. Нативный smoke через WebView выполняет create/open/save/replay/stale conflict, invalid catalog/locale, plan/apply пустого workspace и receipt replay; затем close-to-tray/reopen/Exit. Browser acceptance проверяет цепочку, выбор переменной, override/reset, RU/EN и 760×520 без горизонтального переполнения. Browser example изолирован и не исполняет плагины.

**До закрытия полного gate остаются отдельные срезы:** UI создания configured instances/аккаунтов и выбора instance для action; review/diff и consent для disruptive plan; отображение compiler diagnostics у конкретных полей; компонентная drag/drop automation и единый native E2E от назначения через UI до signal/action/result/feedback настоящего virtual process. Нагрузочную приёмку тысяч controls и virtualized graph нельзя считать закрытой отсечением device cards. Визуальный graph editor остаётся Фазой 11; сложный существующий graph сохраняется и не перезаписывается простым назначением. Hardware rendering остаётся Фазой 9.

Текущие ограничения: каталог пакетов сканируется при старте host; установка и grants не входят в этот экран. После добавления пакета вне UI нужен перезапуск host. Внутренний учебный пример доступен отдельной кнопкой, хранится только в памяти и не доказывает работу production runtime. Сохранение layout не заявляет применение подсветки к оборудованию.

## 6.0 — Доступ к уже работающему ядру

Сначала Tauri adapter и недостающие read projections; canvas ещё не нужен. Desktop composition уже подключает настоящий Gateway/compiler/runtime, locale service и static `PluginCatalog`. Это не означает, что все целевые API groups уже доступны из WebView.

- Host создаёт principal из доверенного контекста окна, а не принимает `principalId`/scopes от JavaScript. `Read`, `Draft`, `Simulation`, `Apply`, `Preferences` проверяются application use cases.
- Обернуть общие queries/commands/mutation window и locale get/change/reconcile. Error DTO сохраняет code, message key, trace ID, revision conflict и retryability; frontend не разбирает Rust error strings.
- Добавить bounded installed-package summaries и registry/runtime-health snapshots через application ports. Catalog выбранного package читать существующим query с точными version/hash. Registry snapshot включает revision/generation и availability; path/handle/process/token не передаются UI. Ошибка metadata не маскируется пустым успешным списком.
- Draft возвращается вместе с version; plan связывается с draft/active revisions и plan hash. Старый ответ не заменяет новую локальную selection. Для первого client достаточно последовательных snapshot queries; подписки потребуют cursor/gap/resnapshot и bounded queues до своего подключения.
- Reconnect сначала восстанавливает snapshot/operation по ID. Потеря ответа mutation повторяет тот же key/payload; новый key не применяется для автоматического повтора неизвестного эффекта. Ошибка locale notification показывает `deliveryPending` и вызывает reconcile текущей revision.

Acceptance: один сценарий выполняется через in-process client и Tauri adapter с одинаковыми DTO/outcomes. Проверить denied scope, invalid catalog/hash, stale draft, replay receipt после потери ответа, недоступный runtime и reconnect. Frontend не импортирует storage/runtime implementation. Это последний backend/transport срез перед визуализацией; он не создаёт второй compiler или scheduler.

## 6.1 — Workspace и выбор control

Read-only virtual device cards строятся по registry descriptors: geometry, elements, ports и availability. Не использовать enum моделей клавиатур/Stream Deck в UI. Keyboard navigation и выбор control используют устойчивый ID; disconnect снимает live availability, сохраняя draft binding. Zoom/scroll/selection — editor state, не workspace apply на каждый жест. Показывать различие между desired graph и active revision.

Acceptance: два одинаковых virtual devices остаются разными, selected control не перескакивает после reorder/reconnect. Неподдержанная geometry отображается явно; frontend не придумывает layout или pressure.

## 6.2 — Каталог и инспектор конфигурации

Action library использует static metadata без запуска плагина. Настройки привязаны к configured instance, а не ко всему package: два аккаунта имеют независимые revisions/credentials. Schema-form поддерживает только доказанный structural vocabulary; opaque `Json` и неизвестная required feature дают unsupported state. UI позволяет выбрать explicit trigger и named value bindings/constants, показывает result/state schema и units. Сложные join/parallel/collections остаются Фазой 11.

Все изменения проходят DraftCommand и compiler diagnostics. Новая форма не получает права запускать процесс, выдавать grant или читать secret. Service bindings можно показывать как недоступную возможность, но нельзя предлагать их как исполняемый node до подключения broker.

Acceptance: transcript record → task create → task ID binding редактируется без ручной JSON-конкатенации. Invalid field/range/units отклоняются; два instances не делят настройки. Выбор ru/en обновляет host preference, plugin labels используют per-key fallback; machine IDs и пользовательский текст не переводятся.

## 6.3 — Draft, plan, apply и восстановление

Dirty/valid/needs-attention/applied/degraded — разные состояния. Сначала validate/plan, затем ApplyDraft с теми же preconditions; успешный receipt не считается здоровым runtime. Показать operation progress, revision conflicts и trace summaries без приватных payload. Undo/redo оформляются workspace use cases с CAS; нельзя откатывать plugin-owned notes или runtime slot selection вместе с layout.

Acceptance: stale plan/revoke между prepare/barrier не применяется; потеря ответа и restart не создают второй action; ошибка cleanup видна как unconfirmed, а не «всё остановлено». Закрытие окна и повторное открытие сохраняют применённую revision и живой host.

## 6.4 — Сквозная приёмка

Virtual device → выбор control → assignment → validation → apply → signal/action/result/feedback. Проверить keyboard-only путь, ru/en fallback, resize, offline provider, unsupported feature и redacted diagnostics. Для нового назначения сохранить цель roadmap «не более трёх действий» на подготовленном virtual fixture; сложная первоначальная настройка аккаунта оценивается отдельно. Убрать элементы, которые дублируют данные или создают декоративную пустоту.

Gate Фазы 6 закрывается после adapter parity, component/E2E acceptance и проверки работающей desktop сборки. Hardware frame rate, audio DSP, live shared services, durable notes и external API integration не выдаются за часть этого результата; их владельцы и обязательные gates перечислены в [pre-6 evidence](pre-phase-6.md#границы-готовности-и-следующие-обязательства).
