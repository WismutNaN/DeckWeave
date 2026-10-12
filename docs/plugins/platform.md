# Платформенные и транспортные плагины

Все карточки — проектирование. Реестр и стадии: [база идей](README.md). Общие правила: [композиция](../contracts/plugin-composition.md), [lifecycle](../contracts/lifecycle.md), [security](../contracts/security.md). Platform adapters имеют общий смысл операций, но объявляют фактически доступные возможности каждой ОС.

<a id="plg-001"></a>
## PLG-001 — Приложения, окна и иконки

- **Сценарии:** показать текущую программу, список окон и доступные tray-приложения с иконками; адаптировать назначения под foreground; переключиться на конкретное окно как в Alt+Tab.
- **Contributions:** query коллекции окон, state foreground, lifecycle events, commands activate/minimize/move/resize и read icon. Process, app, window и tray entry — разные сущности. App identity пригодна для persisted selector; WindowRef/TrayItemRef — scoped к session/generation, raw HWND не сохраняется как identity.
- **Переиспользование:** PLG-002 использует window operations; PLG-023 — foreground state; PLG-022 — коллекцию и icon references. Capability движения окон отделена от права наблюдения.
- **Границы:** фильтры workspace/virtual desktop/visibility явно задаются. Foreground activation может быть запрещена ОС. Tray enumeration — отдельный optional adapter/spike, не обещание полного списка через EnumWindows. Icon fallback, размер, MIME и cache lifetime определены; чужой icon handle не пересекает IPC.
- **Приёмка:** два окна одного приложения, закрытие между render/click, reused handle, недоступная иконка, отказ focus. Профиль не зависит от заголовка окна как единственного ключа. Ограничения Win32 подтверждены в [исследовании](../research/plugin-feasibility.md#external-evidence).

<a id="plg-002"></a>
## PLG-002 — Расстановка окон

- **Сценарий:** сохранить preset выбранных приложений на выбранных мониторах и восстановить по кнопке.
- **Contributions:** capture layout → private versioned preset, preview restore plan, apply restore job → per-window outcomes. Использует service binding PLG-001; не содержит второй enumerator окон.
- **Данные:** stable app selectors и политика matching нескольких окон, monitor identity/geometry, logical units/DPI, work area, minimized/maximized state. Отсутствующий монитор требует выбранного fallback; отсутствующее приложение запускается только по явной опции.
- **Приёмка:** mixed DPI, отключённый монитор, частичный отказ, закрытие окна, cancel. Это набор внешних действий с partial report, не атомарный workspace apply. Ручное перемещение пользователем после начала операции не перезаписывается бесконечным retry.

<a id="plg-003"></a>
## PLG-003 — Метрики системы

- **Сценарий:** CPU, память, GPU и доступные дополнительные sensors представлены числами, шкалами и графиками с условными цветами.
- **Contributions:** typed sampled values с unit/range, timestamp, validity/freshness, hardware identity и negotiated sampling interval. Отсутствующий GPU counter — unavailable, не нулевая нагрузка.
- **Переиспользование:** сбор метрик отделён от PLG-022; историю хранит bounded ring/window, ширина графика не создаёт бесконечный log. Sampling и rendering имеют разные частоты; скрытые views освобождают subscription.
- **Приёмка:** медленный consumer, hotplug, несколько GPU, reset counters и stale samples; load не задерживает reliable input. Доступность counters проверяется для конкретных ОС/драйверов, универсальность не обещана.

<a id="plg-004"></a>
## PLG-004 — Виртуальный ввод

- **Сценарий:** нажать/отпустить клавишу, chord с modifiers, ввести Unicode-текст или вставить его через clipboard.
- **Contributions:** раздельные key/chord/text/clipboard operations; physical scan code, logical key и Unicode text не смешиваются. Clipboard fallback требует своего grant и conditional restore. Это output provider, PLG-024 остаётся input source.
- **Переиспользование:** shortcuts приложений, текст Handy, scripts и browser fallback используют одинаковые owner/cancel semantics. Выбранная ОС сообщает свои методы и ограничения, а не эмулирует отсутствующий universal keycode.
- **Приёмка:** emoji/combining characters, разные layouts, два владельца modifier, потеря focus, revoke/disconnect/apply во время hold; release относится к прежнему owner. Windows UIPI может отклонить ввод; unsupported target не считается успехом. Никаких обещаний ввода «любой сложности» в любое приложение без platform evidence.

<a id="plg-005"></a>
## PLG-005 — Scripts и процессы

- **Сценарий:** PowerShell на Windows, выбранный доступный shell/executable на Linux/macOS; результат передаётся следующему action.
- **Contributions:** job с явными executable, argv, cwd, разрешёнными env/paths, stdin и output schema; bounded stdout/stderr, exit status, deadline и kill process tree. Shell source — отдельный явно выбранный режим.
- **Переиспользование:** SDK process helper, artifact input/output, typed results; credentials передаются scoped способом и не попадают в command log. Dynamic text не интерполируется в shell source.
- **Приёмка:** argv с кавычками/Unicode, non-zero exit, неверный JSON, переполнение stdout, child descendants, timeout/cancel, отсутствующий interpreter. Provider нельзя использовать как обход ограничений custom web view; native code имеет user-trusted модель, process isolation не является sandbox.

<a id="plg-006"></a>
## PLG-006 — HTTP-запросы

- **Сценарий:** настроить URL/method/query/headers/body, выбрать ожидаемый ответ и использовать result в workflow; bundled templates ускоряют типовые запросы.
- **Contributions:** request/job → status, выбранные headers и bounded typed body/artifact. Declared destinations, redirect policy, timeout, response/decompression limits, cancellation, rate limits и credential references обязательны.
- **Переиспользование:** общий HTTP helper SDK полезен каждому connector. Singularity/weather не обязаны зависеть от runtime HTTP plugin: доменную семантику они держат у себя. Runtime service binding нужен только при осознанном использовании общей connection/policy, без передачи чужих credentials произвольному caller.
- **Приёмка:** 429/Retry-After, redirect к неразрешённому origin, truncated body, offline, uncertain POST. Автоматический retry write разрешается только при доказанной API idempotency. Погодная карточка принадлежит PLG-010/022, не HTTP engine.

<a id="plg-007"></a>
## PLG-007 — MQTT

- **Сценарий:** publish по нажатию и subscribe на states; несколько consumers переиспользуют одну настроенную broker connection.
- **Contributions:** connection instance, topic-scoped publish/query/subscription, reconnect/freshness/gap state. TLS/account/topic grants; limits payload/rate/subscriptions/offline queue. Зависимости живут через lease, не через бесконечный pending invocation.
- **Переиспользование:** Sprut.hub и другие IoT mappings используют этот transport, но сохраняют domain IDs/units/conversions у себя.
- **Приёмка:** retained snapshot не запускает command повторно; duplicate/reconnect не повторяет toggle; QoS receipt отделён от подтверждения физического света. Предпочтительны абсолютные set-state операции, write-retain не включается молча. [Основание](../research/plugin-feasibility.md#external-evidence).

<a id="plg-008"></a>
## PLG-008 — Время и таймеры

- **Сценарий:** часы/дата в заданном locale/format/timezone, countdown, timer elapsed event для другой автоматизации.
- **Разделение:** host владеет monotonic clock, deadline queue и порядком timer events. Provider/template отвечает за clock/calendar presentation и прикладные timer commands; Pomodoro — отдельная политика PLG-011. Нельзя требовать установки clock plugin для безопасности host deadlines.
- **Данные:** TimerId/owner/epoch, duration или calendar target, pause/resume и sleep/restart policy. Elapsed — reliable event с occurrence ID, remaining — coalescible state; пропущенные ticks не вызывают burst effects.
- **Приёмка:** смена системных часов, DST/timezone, suspend/resume, restart, cancel одновременно с deadline. Monotonic время разных process/boot не сравнивается без согласованного time domain. [Нормативная семантика](../contracts/automation.md#time-and-schedules).

<a id="plg-024"></a>
## PLG-024 — Platform Input

Ранее запланированный bundled provider наблюдает обычную клавиатуру в pass-through mode. Observe, selective intercept и emit — разные capabilities/grants; aggregate hook не выдаёт физическую identity. Shared parts: normalized input, gesture/owner semantics, PLG-023. Приёмка: injected-event loop guard, два keyboards при поддерживаемой granularity, disconnect и neutral rearm. Реальный Windows observe — Фаза 7; privileged suppression/virtual devices — отдельный последующий gate. [Модуль](../architecture/modules/platform-input.md).

<a id="plg-025"></a>
## PLG-025 — Windows media и запуск приложений

Ранее запланированные first-party contributions: volume/mute, media transport, audio endpoint state, app launch/open URL. Launch возвращает факт запуска и доступную app reference; готовность окна/документа — отдельный predicate. Window enumeration принадлежит PLG-001. Приёмка: endpoint hotplug, unsupported media session, launch failure и смена default device. Общие OS adapters не обязаны иметь один package/process или одинаковые permissions. [Фаза 7](../implementation/roadmap.md#фаза-7--windows-base-provider-и-platform-input).
