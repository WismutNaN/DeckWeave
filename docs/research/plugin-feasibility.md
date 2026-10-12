# Реализуемость каталога плагинов перед Фазой 6

Дата: 2026-10-12. Проверен DeckWeave `2d99c88`, текущее дерево без исходных изменений пользователя. Метод: чтение реализованных ports, compiler/scheduler, wire translation, runtime activation, desktop composition, contracts и [каталога идей](../plugins/README.md); локальный Zotst и официальные API sources. Existing tests повторно не запускались. Ниже **статические наблюдения и design gaps**, не результаты новых executable probes или подтверждение работы внешних integrations.

## Вывод

Все перечисленные идеи имеют подходящий путь в существующих DDD contexts: endpoint/providers, typed jobs/results, Automation, scene/presentation и специализированный media runtime. Оснований заменять всё ядро нет. Однако M1 Фазы 5 проверяет узкий graph/virtual runtime; он ещё не доказывает configured accounts, сложные records, постоянные views, service dependencies и реальное удержание input. До закрепления UI-модели Фазы 6 нужны [дополнительные срезы](../implementation/pre-phase-6.md).

Нельзя гарантировать «любое приложение/любой tray/любое устройство»: ограничения ОС, API и firmware остаются в adapters и отражаются capabilities/availability. Архитектурное доказательство требует независимых fake providers, а физическая/API совместимость — отдельного evidence. Полная реализация каталога до canvas не нужна.

## Проверка функций ядра по коду

| ID | Граница и evidence на `2d99c88` | Почему важно для идей | Что закрыть |
|---|---|---|---|
| CG-01 | [RuntimeRequirement](../../crates/deckweave-automation/src/plan.rs) не содержит configured instance/settings; [activation](../../crates/deckweave-plugin-supervisor/src/host/instance.rs) передаёт `settings: None`; [group validation](../../crates/deckweave-plugin-supervisor/src/host/coordinator/plan.rs) запрещает несколько contributions одной группы | Два accounts Singularity, два MQTT brokers, weather locations, action+state одного package | Versioned configured instance/binding/settings model и multiplexed activation; не закреплять один process на каждый account |
| CG-02 | [CatalogSchema](../../crates/deckweave-automation/src/compile/catalog.rs) содержит общий ValueKind; [value_matches_schema](../../crates/deckweave-automation/src/scheduler/routing.rs) принимает любой JSON для `Json`; [wire decode](../../crates/deckweave-plugin-supervisor/src/host/instance/wire.rs) читает bytes без сверки payload schema/encoding с ожидаемыми | Task/WindowRef/Weather/Color records могут пройти с неверными полями или schema envelope | Structural schema validation, bounded arrays/depth, enum/range/unit и schema/encoding check на входе; scalar fixture недостаточна |
| CG-03 | [Graph model](../../crates/deckweave-automation/src/graph/model.rs) описывает nodes/ports/edges, но не field projection, constants и named argument assembly; [routing](../../crates/deckweave-automation/src/scheduler/routing.rs) запускает action на каждом arrival, trigger даёт null | Взять `transcript.text` как task.title, добавить description/account, использовать selected task ID; разные inputs не должны вызвать несколько writes | Minimal typed binding model: constant/source result/state reference, field path, required/optional inputs и один trigger. Полные joins/loops остаются Фазой 11 |
| CG-04 | [LiveExecutionOwner::ingest](../../crates/deckweave-host/src/execution/owner.rs) передаёт `now_ticks = 0`; обычный live ingress не получает текущий clock. Есть scheduler deadline transition, но production timer pump в изученной composition не подключён | Pending HTTP/script/Handy без ответа; Pomodoro; game rounds; deadline после долгого uptime | Injected monotonic clock + bounded wakeup pump в host owner, явный time domain между host/provider; timeout без новых input events |
| CG-05 | [RuntimeDispatchHub](../../crates/deckweave-plugin-supervisor/src/host/dispatch.rs) возвращает success для CleanupRun/CleanupAll без route; [wire](../../crates/deckweave-plugin-supervisor/src/host/instance/wire.rs) не кодирует их. Raw signals в live bridge всегда получают Reliable | Held synthetic modifier, layer owner, game/view leases; высокочастотные metrics не должны занять edge queue | Host/provider output owner registry и реальное cleanup/ACK/fault path; descriptor-driven QoS со сохранением encoder edges. Не считать trace cleanup доказательством внешнего release |
| CG-06 | [Manifest schema](../../protocol/schema/plugin-manifest.schema.json) разрешает произвольные dependency objects; [IndexedPackage](../../crates/deckweave-plugin-supervisor/src/package.rs) не разрешает service requirements/transitive closure | PLG-002→001, PLG-018→007, замена providers | Declared requirements, explicit binding, locks/DAG, scoped delegation и lease model; prototype resolver до editor, полный installer позже |
| CG-07 | [Interaction model](../../crates/deckweave-interaction/src/model.rs) и [ports](../../crates/deckweave-interaction/src/ports.rs) описывают короткую session. Модуль уже честно отмечает отсутствие query generation/schema/retention guarantees. Persistent view owner не определён кодом | Zotero с вводом во время startup; edit note отдельно от always-on-top display | Interaction vs DesktopViewSession, query generation, placement/lease/grants, bounded pages и schema validation; fake presenter до UI, real windows в Фазе 13 |
| CG-08 | [Surface module](../architecture/modules/surface-compositor.md) и collection semantics пока целевые; общий item-bound navigation/presentation revision не реализован | Task board, Alt+Tab, notes, палитра и game | Render/hit revision, stable item binding, selected action/back, scene/layer owner; смена режима без полного workspace apply |
| CG-09 | [Profile Store](../architecture/modules/profile-store.md) хранит workspace/drafts/assets; отдельного broker для durable plugin documents и dynamic asset lifetimes ещё нет | Notes/palettes/layout presets, private task cache, window icons, WAV | Разделить configuration/private durable/cache/artifact/credentials; namespace/quotas/export/migration/retention, scoped asset leases |
| CG-10 | [Desktop composition](../../apps/desktop/src-tauri/src/composition.rs) всё ещё выбирает RestrictedFixtureCompiler и DenyAllAuthorizationStore | Canvas не сможет применять настоящий graph с permissions только за счёт Tauri adapter | Composition получает настоящий catalog/compiler и authorization/credential ports; fail-closed сохраняется. Production UI не строится на test-only catalog или allow-all |
| CG-11 | Pure DAG/scheduler, stable/runtime endpoints, epochs, typed terminal outcomes, limits и replay уже есть; новые tests/core gate Фазы 5 записаны в [baseline](../implementation/phase-5-latency-baseline.md) | Хорошая основа для всех families без vendor branches | Сохранить regression acceptance при расширении; не выдавать virtual dispatch latency за display/audio/API latency |
| CG-12 | B2 checkpoint/compaction остаётся открытой в roadmap | Editor mutations и profile history | Закрыть существующий storage gate до интенсивной нагрузки Фазы 6; notes/telemetry не записывать через workspace patch |
| CG-13 | Collection contract есть, но normalized cross-service count/read-state и aggregation policy ещё не реализованы | Unread inbox Telegram/mail/Mattermost, news | Source/account identity, count units/completeness, отдельные read/ack/viewed states, bounded aggregate cursors; fake mixed feed до editor |
| CG-14 | В текущем graph нет slot/widget selection; scene/layers ещё запланированы | Карусель clock/countdown/unread одной кнопки без замены экрана | Slot/widget IDs, active selection, gesture owner/revision, hidden-view leases и shared presenter port до UI; реальный rendering позже |
| CG-15 | Remote/media boundaries запланированы, generic provider может отдавать metrics; Samsung-specific mobile integration отсутствует | Steps/sleep/score и contacts/dates | Personal metric time/source/provenance/privacy schema; отдельный authenticated mobile bridge и API approval gate, не новый health engine в core |

CG-02/04/05 — конкретные незавершённые live paths, требующие исправления/отрицательных regression tests, а не только новых абзацев. CG-01/03/06/07/08/09 — форма будущих контрактов, которую опасно определять после editor и первых монолитных plugins. CG-10 — обязательная production composition перед canvas. Старые F5-R1–R3 из [apply-аудита](pre-apply-review.md) уже исправлены срезом 5.0; они здесь не открываются заново.

## Покрытие пользовательских сценариев

| Сценарии каталога | Общие части | Основные gaps |
|---|---|---|
| Окна/иконки/Alt+Tab/раскладка, PLG-001/002 | Collections + service binding + runtime refs + artifacts | CG-01/06/08/09 и OS focus/tray limits |
| Метрики/погода, PLG-003/010 | Typed measurements + time + bounded charts | CG-02/04/05/08 |
| Input/scripts, PLG-004/005/024/025 | OS providers + args/jobs + output owners | CG-03/04/05/10 |
| HTTP/MQTT/Sprut, PLG-006/007/018 | Transport helpers/services + grants + state | CG-01/02/04/06 |
| Singularity/voice, PLG-009/013/014 | Typed chain + artifact + collection presenter | CG-01/02/03/07/08/09 |
| Clock/Pomodoro, PLG-008/011 | Host time domain + timer policy + reliable elapsed | CG-04/05/09 |
| Zotero, PLG-012 | Concurrent startup/search + picker + scoped attachment | CG-03/07/09/10 |
| Browser/mail/Telegram, PLG-015/016/017 | Typed outcome + handle identity + collection | CG-02/03/06/08/09 |
| Notes/пипетка, PLG-019/020 | Durable data + short edit/picker + persistent views | CG-02/07/08/09 |
| Игра/представления/межустройственное переключение, PLG-021/022/023 | Scene/input revision + clock + owners | CG-04/05/08; hardware latency позже |
| Hardware/bridges/remote/media, PLG-026–036 | Existing control identity и specialized plane boundaries | Model/API gates, CG-05/06/08/11; real-time audio не переносить в control IPC |
| Unread aggregator/Mattermost/news, PLG-037/038/042 | Feed adapters → normalized aggregation → shared presenter | CG-02/06/08/13; exact/partial counters и no read-on-render |
| Health/countdown/important dates, PLG-039/040/041 | Metrics/date source → pure calculation → templates | CG-02/04/09/15; mobile API gate, timezone/privacy и недоступные scores |
| Slot carousel, PLG-043 | Host presenter + slot-local state + shared resources | CG-05/08/14; текущий press не запускает action нового widget |

<a id="zotst"></a>
## Локальный референс Zotst

Путь `G:/TYPST/zotst`, HEAD `1091ffb2a55aab16c0db6eb655fa0beab29a208f`. Working tree **изменён пользователем**: изучены текущие файлы, их содержимое не следует приписывать целиком commit. Никаких файлов в этом проекте не изменено. `package.json` указывает MIT; перед копированием проверяются license/notices каждого выбранного fragment.

| Файл | SHA-256 изученного содержимого |
|---|---|
| `src/zoteroClient.ts` | `C4793FEB35F4B83C548DAD1149480FF93250A29CB028F8E80710E9F449D1DE41` |
| `src/citationPicker.ts` | `FBEFFFF0B99BBD5960A07EBE31BA07954249F2029BB6DE71F66170580EEC4BF9` |
| `src/extension.ts` | `DCE13B532922CA523A7A8EAE856BD43A22795B6594E78695570765A2C8E9163F` |

Наблюдения: local BBT JSON-RPC `item.search`/`item.attachments`, readiness probe; picker debounce и search generation защищают от старого результата; extension выбирает `zotero://open-pdf/` attachment. Это подходящие reuse-кандидаты. Цитирование, `.bib` sync и VS Code UI не нужны PDF launcher. Transport сейчас собирает response string без byte bound, использует time-based RPC ID; search dedup по citekey и выбор первого PDF нужно заменить library/item/attachment identity и явной selection policy. При переносе нужны cancellation, byte/page limits, correlation, redaction и readiness, совместимые с DeckWeave. Код не скопирован.

<a id="external-evidence"></a>
## Проверенные внешние границы

Sources просмотрены 2026-10-12; реальные credentials, приложения и устройства не подключались. API version/payload fixtures фиксируются при начале package implementation.

| Источник | Наблюдение → решение DeckWeave |
|---|---|
| [Microsoft EnumWindows](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-enumwindows), [SetForegroundWindow](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-setforegroundwindow) | Перечень top-level windows и право на foreground различаются. Полный список всех приложений/окон не следует из одного API; activation может быть отклонена. Provider сообщает granularity и результат |
| [Microsoft Shell_NotifyIconGetRect](https://learn.microsoft.com/en-us/windows/win32/api/shellapi/nf-shellapi-shell_notifyicongetrect) | API требует identifier уже известной tray icon. Вывод: это не доказательство полного tray enumeration; отдельный adapter/spike, никаких обещаний через скрытые Explorer internals |
| [Microsoft SendInput](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-sendinput) | UIPI ограничивает injection. Output capability возвращает реальные limitations; нельзя обещать любой privileged target |
| [Singularity REST API](https://singularity-app.com/wiki/api/) | Документирован v2, tokens, CRUD и offset pagination; webhooks/subscriptions не заявлены. Нужны polling/freshness, adapter consistency и version-pinned task semantics; никаких вызовов API в ходе этого аудита |
| [Sprut.hub MQTT bridge](https://wiki.spruthub.ru/Создание_моста_MQTT) | Есть настраиваемый MQTT bridge, visibility устройств и `/set`; это основание transport/domain разделения, не универсальная схема payload всех accessories |
| [MQTT 5 OASIS](https://docs.oasis-open.org/mqtt/mqtt/v5.0/mqtt-v5.0.html) | QoS и retained delivery имеют собственную transport семантику. Вывод: receipt MQTT не доказывает physical effect и retained snapshot не равен новому пользовательскому trigger |
| [Better BibTeX JSON-RPC](https://retorque.re/zotero-better-bibtex/exporting/json-rpc/) | Поддерживает search/attachments; локальный Zotst — adapter-кандидат, а не API самого DeckWeave |
| [Handy upstream](https://github.com/cjpais/Handy), [прежнее pinned исследование](implementation-review.md#handy-и-результаты-действий) | File transcription и interactive UX нужно различать. Проверить конкретный executable/model; extraction native libraries — альтернативная отдельная работа |
| [Open-Meteo documentation](https://open-meteo.com/en/docs) | Current/hourly поля имеют units и temporal meaning. Вывод: нормализовать weather schema, не смешивать precipitation probability с количеством/интенсивностью |
| [Gmail labels](https://developers.google.com/workspace/gmail/api/reference/rest/v1/users.labels), [TDLib chat](https://core.telegram.org/tdlib/docs/classtd_1_1td__api_1_1chat.html), [Mattermost API](https://docs.mattermost.com/api) | Gmail различает unread messages/threads, TDLib имеет chat unread count/read position. Вывод: normalized feed сохраняет count unit и read scope; конкретные endpoints Mattermost/version/permissions ещё требуют package spike |
| [Samsung Data SDK](https://developer.samsung.com/health/data/overview.html), [data types](https://developer.samsung.com/health/data/guide/features/data-types.html), [production registration](https://developer.samsung.com/health/data/faq.html) | Android SDK объявляет Steps/Sleep/Energy score; production app требует registration и соответствующих разрешений. Вывод: mobile adapter + authenticated bridge, не предполагаемый desktop REST API. «Качество жизни» не отождествляется с названием vendor score |
| [Android Health Connect](https://developer.android.com/health-and-fitness/health-connect), [permissions/background example](https://developer.android.com/codelabs/health-connect) | Доступ и background reads зависят от permissions/availability. Это альтернативный adapter для поддерживаемых records, не подтверждение наличия всех Samsung scores |

## Что изменено этим аудитом

Создана [база идей](../plugins/README.md), уточнены композиция, presentation/data/time contracts, добавлен ADR и [pre-6 gate](../implementation/pre-phase-6.md). Production Rust/TS code и существующие tests не изменены. Новые design contracts ещё требуют реализации и fixtures; документация не закрывает gaps из таблицы.
