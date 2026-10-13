# Фаза 6: application adapter и Desktop editor

**Статус: завершена, 2026-10-13.** Входной [gate 6G.0–6G.5](pre-phase-6.md) закрыт на `328cebf`; desktop gate 6.0–6.4 подтверждён настоящим Tauri/WebView2 и отдельным процессом bundled virtual provider. Пользователь добавляет virtual device, назначает и настраивает действия, проверяет изменения, применяет их и запускает цепочку нажатием виртуальной кнопки.

Нормативные правила: [Application API](../contracts/application-api.md), [граница plugin](../contracts/plugin-boundary.md), [стандарт UI](../product/ui-quality-standard.md). Команды воспроизведения и упаковка provider описаны в [разработке](development.md).

## Итог и доказательства

| Срез | Реализация и приёмка |
|---|---|
| 6.0 — adapter | Host-derived principal; blocking IPC; общий Gateway/compiler/runtime. Native smoke проверяет CAS, replay, locale, plan/apply, active document и receipt identity. In-process tests проверяют scopes и ошибки. |
| 6.1 — workspace | Pan пустого поля без Space, zoom/fit, spatial keyboard selection, отсечение устройств и отдельных controls за viewport. Два различимых virtual templates доступны в обычной сборке. |
| 6.2 — inspector | Static action catalog, schema forms, typed result bindings, независимые configured instances и выбор подключения у каждого действия. Native E2E проводит текст через Work → Home и получает `H:W:Hello` от plugin process. |
| 6.3 — apply | Review показывает добавленные/изменённые/удалённые действия и их кнопки, изменения связей, подключений и оформления. Disruptive consent требует нового plan; diagnostics сохраняют node/field без значений. CAS/recovery не заменяют неизвестный запрос новым. |
| 6.4 — E2E | Назначение → review → apply → live virtual input → action → typed result → feedback → provider readback → текст на кнопке. Проверены stale epoch/generation/endpoint, redacted trace, close-to-tray/reopen/Exit и отсутствие orphan WebView2. |

Код закрывающего среза: `bac7be9` (virtual runtime) и `2c34d76` (desktop, recovery и native acceptance); ранее `f1efd8f` и `740314a` добавили темы, viewport, accounts и review contract. Локально прошли `npm run check`, 259 Rust tests всего workspace, `cargo fmt`, clippy со всеми features без warnings и release build. CI workflow воспроизводит проверки; удалённый запуск CI этим отчётом не заявляется.

28 frontend tests покрывают, в частности, перенос/переупорядочивание цепочек, remapping ссылок на результаты, сохранность чужого graph, instances, consent, diagnostics, recovery и тему. Отдельные Rust tests проверяют отсутствие повторного live input после restart, scope Apply вместо Simulation, dry-run rejection, изоляцию accounts, UTF-8 bounds и отказ большой записи без изменения сохранённой версии.

**Настоящие жесты в Windows:** library → кнопка, копирование кнопки и pan пустого поля проверены мышью в WebView2. Внутренние переносы используют pointer capture с подсветкой цели, ghost и отменой по Esc: быстрый Windows HTML5/OLE drag терял drop, хотя синтетический тест проходил. HTML5 DataTransfer handlers сохраняют совместимость и используются также в повторяемом component smoke. Shift переносит привязку, обычный drag копирует; ссылки результатов переназначаются на новые IDs. Зависимый шаг нельзя переместить перед источником результата.

**Внешний вид:** system/light/dark и собственный accent сохраняются между запусками; цвет текста accent адаптируется до AA-контраста. Review визуально проверен в светлой и тёмной темах; native smoke целиком работает в окне 760×520 и проверяет доступность primary action без горизонтального overflow. Общий gradient использует геометрию plugin; overrides кнопок сохраняются. Ctrl+S/Ctrl+Z учитывают физический key code при русской раскладке.

**Нагрузка:** draft-only fixture с 4096 controls и 4096 action nodes проходит реальный WebView, прокрутку, spatial selection, изменение подписи, durable recovery и undo. Последний локальный замер: 84 mounted controls, 118 мс на прокрутку/выбор. Для этой проверки geometry/capabilities упрощены; настоящий wire/feedback проверяется отдельным 15-key provider, fixture не применяется к runtime. Это evidence редактора, а не hardware FPS benchmark. История отмены ограничена 64 snapshots и бюджетом 32 MiB оценочного JSON-объёма с сохранением последнего снимка.

**Восстановление:** IndexedDB хранит recovery journal отдельно для WebView data-root, с миграцией прежнего localStorage record. Mutation не пересекает IPC до complete durable transaction. Ошибка journal блокирует отправку; исходный envelope сохраняется для retry. Лимит Profile Store возвращается как `capacity_exceeded`, а не потеря связи; recovery не заменяет persistent draft store.

## Границы завершённой фазы

- Virtual Deck A/B входят в штатный bundle; это обычный versioned package, запускаемый supervisor. Кнопка «Проверить кнопки» явно запускает **применённые** действия, имеет scope Apply и отличается от dry-run. Любые внешние actions, назначенные на эту кнопку, сохраняют собственные permissions.
- Учебный browser example хранится в памяти и не доказывает production runtime. Он остаётся явно отдельным режимом.
- Каталог packages сканируется при старте. Install/update/grants UI и замена pinned package относятся к следующим фазам; существующие bytes bundled package не перезаписываются.
- Solid/gradient/individual background сейчас являются desired editor layout. Изображения, hardware frames/RGB и Surface editor — Фаза 9; сложный Graph editor/layers/parallel — Фаза 11. Existing advanced graph сохраняется без переписывания простым inspector.
- Реальные Windows/provider, hardware, media, services и overlays имеют собственные gates. Закрытие Фазы 6 не закрывает [Фазу 7](phase-7-windows.md) или её ручную Windows 10/11 матрицу.

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

Gate Фазы 6 закрыт перечисленной выше adapter/component/E2E приёмкой работающей desktop сборки. Hardware frame rate, audio DSP, live shared services, durable notes и external API integration не выдаются за часть этого результата; их владельцы и обязательные gates перечислены в [pre-6 evidence](pre-phase-6.md#границы-готовности-и-следующие-обязательства).
