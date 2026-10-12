# Gate расширяемости перед Фазой 6

Статус: **закрыт для начала Фазы 6** по итогам аудита и integration acceptance 2026-10-12. Проверена реализация до `328cebf` включительно: 219 Rust tests, `cargo clippy --workspace --all-targets -- -D warnings` и `npm run check` проходят. Исходные наблюдения относятся к `2d99c88`; текущие доказательства перечислены ниже. Закрытие gate означает готовность общих контрактов и control runtime к редактору; оно не объявляет реализованными все plugin adapters. Основание: [CG-01–15](../research/plugin-feasibility.md), сценарии: [каталог PLG](../plugins/README.md), сводный путь данных: [граница ядра и плагина](../contracts/plugin-boundary.md). Следующий срез — [6.0, application adapter до canvas](phase-6-desktop.md).

## Проверенный результат повторного аудита

| Область | Реализация и evidence | Что ещё нужно проверить или подключить |
|---|---|---|
| Storage | Checkpoint, cached quota accounting, retired claim GC с защитой pending apply, terminal apply archive вне startup scan. `compaction` проверяет faults, rollback clock, replay и pending/history; `growth` — 256 mutations/4 restarts, автоматический checkpoint, receipt quota и постоянный write budget | B2 закрыт. Power-loss и физическая latency не доказаны этими тестами; archive quota сохраняет историю и ограничивает новые apply при заполнении hot store |
| Instances | Settings/schema/revision/CredentialRefs, required features, несколько leases одной group, endpoint ownership по instance. `configured_instances` запускает два аккаунта в одном процессе и восстанавливает настройки после restart; `grouped_contributions` выполняет source/action/feedback разных contributions в одном процессе | Credential preflight/revoke доказаны отдельно; secret lease/use/renew не заменяется передачей CredentialRef. Разные artifacts одного package требуют расширения lock перед SDK freeze |
| Bindings | Structural schemas, wire SchemaRef, finite ranges, single-trigger named arguments, compiler field/type checks, run-local results. `argument_compiler` проверяет compiler → scheduler; `structured_arguments` — concurrent/malformed data; `action_results` передаёт task ID двум параллельным цепочкам с обратным порядком completion | Полные collection/join/parallel operators остаются Фазой 11; implicit unit conversion запрещён; Flow token отделён от result value |
| Live execution | Общий monotonic clock, timer pump, provider budget, ordered scheduler/dispatch, activation commit после routes, cleanup receipt/deadline и stop drain. `held_outputs` проверяет два живых owners, shutdown/apply/revoke/crash и mixed metrics/button/completion overflow через настоящие процессы; `cleanup_receipts` — negative/missing ACK | Cleanup admission error возвращается barrier; отказ одного release не останавливает deadlines других runs. Late session и несовместимая wire version отклоняются. Crash plugin-owned output даёт unconfirmed release, а не доказательство освобождения ОС; host-owned physical lease обязателен до input emission |
| Presentation | ItemRef/revision/hit capture, slot next/back/consumer count, DesktopViewSession lease model; InteractionSession query fence/schema/retention и owner/provider cancellation | `collection_presenter`, `plugin_data`, `plugin_assets`, `feed_metric_contracts` доказывают общую модель, scoped CAS/retention, независимые slots и partial/count units/corrected measurements. Это contract fakes: реальный renderer, durable data adapter и IANA/внешние API проверяются позже |
| Composition | Desktop: PackageGraphCompiler, static catalog по package lock, ScopedAuthorizationStore, `PluginCatalog` query через общий Gateway. Credential preflight подключён; default unresolved блокирует activation до запуска процесса | `credentials` доказывает unresolved до process и revoke между prepare/barrier. Реальный acquire/use/renew остаётся gate первого secret-using adapter. LocalePreferencesService + JSON CAS store подключены в Desktop; tests доказывают restart, delivery retry, catalog fallback/hash и authorized metadata query без исполнения кода |
| Services | Typed role bindings, transitive resolver, cycle/optional/missing/artifact conflicts; delegation intersection/deadline/fence и subscription gap rules с tests | `service_lifecycle` доказывает job/subscription results, cancel, provider restart, revoke и lease cleanup на выбранных windows/MQTT roles. Live broker ещё не подключён: serviceBindings сохраняются, но compiler явно блокирует execution |

Все срезы 6G.0–6G.5 имеют executable evidence. Реальные hardware, DSP, renderer и внешние services имеют отдельные gates ниже. Матрица различает production runtime, реальные изолированные test providers и contract fakes; результаты одной категории не подменяют другую.

## Порядок

Каждый срез оценить на 1–5 дней; при превышении разделить на ports/model, implementation и integration acceptance. До gate допустимы UI sketches и read-only shell; persistent inspector/binding schema и production canvas опираются на закрытый gate. Нельзя сохранять непроверенные extensions как будто они уже исполняются.

### 6G.0 — Хранение перед нагрузкой editor

- [x] Закрыть B2 checkpoint/compaction/quota accounting с recovery/retention tests; не смешивать plugin data с workspace history (→ [Profile Store](../architecture/modules/profile-store.md), [plugin data](../contracts/plugin-composition.md#plugin-data)).

Выход: растущая история не превращает обычную mutation в scan всех receipts; crash при compaction сохраняет pending/claims/receipts и выбранные revisions.

### 6G.1 — Instances, settings и зависимости

- [x] Ввести versioned configured-instance/settings/binding model в Workspace/Plugin Runtime ports и protocol; package/group/session/physical target остаются разными IDs (→ [композиция](../contracts/plugin-composition.md), CG-01/06).
- [x] Реализовать activation нескольких contributions/instances одной group с их settings и scoped CredentialRefs; добавить required-feature compatibility и import preserving unknown optional data (→ [Plugins](../contracts/plugins.md), [Lifecycle migrations](../contracts/lifecycle.md#миграции-и-неизвестные-данные)).
- [x] Описать/проверить fake resolver transitive requirements, explicit provider selection, missing/optional/cyclic/version-conflict и locked artifacts. Зафиксировать service-call/delegation/subscription port до UI; полноценный shared service broker допускается позже до первого зависимого plugin (→ [ADR-0019](../decisions/0019-plugin-composition.md)).

Выход: два аккаунта и state+action contributions в одной group не смешивают настройки/данные; restart/rollback сохраняет выбранные bindings; provider не выбирается случайно. SDK library/recipe reuse не требует лишнего runtime dependency.

### 6G.2 — Сложные данные и аргументы actions

- [x] Проверять wire schema/encoding, scalar range, bounded nested records/arrays, enum/optional/null; forbidden/unknown required structure не проходит как общий Json (→ [Automation](../contracts/automation.md), CG-02).
- [x] Ввести minimal typed value binding: constants, result/state field references, named inputs и явный trigger. Ошибка missing field/schema не вызывает action; сбор arguments не превращается в отдельный invocation на каждый data edge (→ [Automation](../contracts/automation.md#structured-bindings), CG-03).

Выход: fake transcript → task `{title, description}` и task ID → complete проходят через общий compiler/scheduler; две concurrent chains изолированы. Неверная вложенная структура, чужой SchemaRef и несовместимые units отклоняются. Полные sequence/parallel/join/collections operators остаются Фазой 11; editor не имитирует их JSON-конкатенацией.

### 6G.3 — Live clock, cleanup и ingress

- [x] Подключить monotonic clock и bounded timer pump к LiveExecutionOwner, согласовать provider deadlines; проверить timeout без дополнительных событий (→ [time contract](../contracts/automation.md#time-and-schedules), CG-04).
- [x] Реализовать owner-scoped cleanup через настоящие host/provider ports, ACK/timeout/fault path; success не возвращается за пропущенный release. Для outputs определить lease/cleanup после crash процесса (→ [Lifecycle effects](../contracts/lifecycle.md#executions-cancellation-и-смена-плана), CG-05).
- [x] В live bridge брать QoS из descriptor: reliable edge/delta/terminal отдельно от coalescible state/value; test mixed metrics + key release + completion при overflow (→ [Lifecycle ordering](../contracts/lifecycle.md#порядок-сигналов-и-время)).

Выход: fake provider удерживает output до release своего owner; завершение одного run сохраняет чужой owner, apply/revoke/shutdown освобождают оставшиеся. При crash/disconnect подтверждения release нет: внешний след удержания в fixture сохраняется, pending routes снимаются, shutdown возвращает ошибку. Это проверка обнаружения неизвестного исхода; физическое восстановление принадлежит обязательному platform lease adapter. Pending job заканчивается по deadline при тишине, 10 минут host uptime не обнуляют время; отказ cleanup не останавливает остальные deadlines. OS hooks и Pomodoro UI в этот gate не входят.

### 6G.4 — Представления, коллекции и данные plugin

- [x] Зафиксировать ItemRef/revision и presentation hit binding, navigation/back, scene selection без workspace apply. Fake presenter: task list, windows и notes используют один contract (→ [presentation](../contracts/presentation.md), CG-08).
- [x] Развести InteractionSession и DesktopViewSession; добавить query-generation/owner/deadline/retention модель и fake lifecycle (→ [Interaction Runtime](../architecture/modules/interaction-runtime.md), CG-07).
- [x] Определить scoped document/asset storage ports и retention/export/revision policy; fake private notes/palette survive view close, workspace rollback не откатывает их молча (→ [plugin data](../contracts/plugin-composition.md#plugin-data), CG-09).
- [x] Зафиксировать общий presenter service и slot-local widget selection; два slots меняются независимо, hidden widgets не продолжают render/tick без consumer (→ [slot contract](../contracts/presentation.md#slot-switching), CG-14).
- [x] Добавить fake mixed unread/feed и personal metric/date fixtures: разные count units/partial source, no read-on-render, corrected/overlapping measurements и timezone boundaries (→ [личные данные и ленты](../plugins/feeds-and-personal.md), CG-13/15).

Выход: перестановка списка между render/click не меняет target операции; два displays независимы; close edit не удаляет note/view, late search не подменяет актуальный query. Contracts/fixtures готовы до canvas; настоящий renderer/always-on-top и durable plugin store adapter реализуются перед соответствующими packages в Фазах 9/13 либо отдельном срезе. Не вводить новую базу данных только ради этих mocks.

### 6G.5 — Production composition

- [x] Подключить catalog/graph compiler и scoped authorization/credential ports в desktop composition вместо fixture-only planning; тот же application service работает без editor (→ [Host Runtime](../architecture/modules/host-runtime.md), CG-10).
- [x] Проверить static package/schema metadata → configured instances → plan/apply → real isolated virtual providers → trace → restart. Metadata не появляется магически из уже активной session; import/index не запускают code (→ [Automation Engine](../architecture/modules/automation-engine.md)).

Выход: production composition применяет настоящий graph, видит denied/unresolved permissions и показывает diagnostics через API. Нельзя заменять DenyAll на AllowAll ради успешного demo. Persistent grant management требует отдельного review; fake authorization допустима только в harness.

## Финальная демонстрация и разрешение перехода

- [x] Один reusable collection presenter получает fake tasks/windows/notes; item actions адресуют конкретную revision (→ [PLG-022](../plugins/productivity.md#plg-022)).
- [x] Fake layout consumer использует выбранный fake window service; fake hub использует выбранный fake MQTT role. Lifecycle/delegation можно доказать test port без сетевых credentials (→ [композиция](../contracts/plugin-composition.md)).
- [x] Fake transcript/HTTP record → task create показывает structured bindings; two instances не смешиваются (→ [PLG-009](../plugins/integrations.md#plg-009)).
- [x] Fake clock/held-output/telemetry fixture доказывает order, deadlines и cleanup. Сам game plugin и physical frame latency проверяются с renderer; быстрый headless сценарий не доказывает частоту обновления Mirabox (→ [PLG-021](../plugins/productivity.md#plg-021)).
- [x] Зафиксировать результаты, оставшиеся platform limits и backward compatibility. B2 и live-path fixes закрыты; прочие целевые runtime features имеют назначенный этап и не выдаются за готовые (→ [исследование](../research/plugin-feasibility.md)).

Только после этих доказательств gate меняется на closed. Локально запускать новые отрицательные tests и затронутые suites; не повторять старый набор ради количества. На интеграционном выходе — общий core/compatibility/docs gate. Внешние Singularity/Zotero/Handy/Sprut, hardware и real overlays проходят собственную приёмку позже.

## Границы готовности и следующие обязательства

| Возможность | Следующий gate и проверяемый результат |
|---|---|
| UI transport и read models | Фаза 6.0: Tauri principal, одинаковые DTO/errors, catalog + draft + registry/health snapshots, reconnect и locale API. Не добавлять execution rules в frontend |
| Физические held outputs | До первого input-emission adapter в Фазе 7: host-owned lease с owner/refcount, expiry/revoke/crash recovery; два owners одной клавиши не отпускают её друг у друга. Неподтверждённый OS release виден пользователю |
| Secrets и permissions | До первого privileged/secret-using package: persistent grant management, scoped acquire/use/renew/revoke, редактирование CredentialRef и secret-free traces. Текущие default resource/credential ports закрыты |
| Nested services | До window-layout/Sprut/другого зависимого plugin: live broker по доказанным fake ports, authority intersection, cancellation, provider restart и subscription resnapshot. Пока compiler запрещает execution `serviceBindings` |
| Frames, images, sound, DSP | Фазы 8/9: hardware read/write evidence, frame endpoint negotiation, renderer/asset delivery. Отдельный media gate: stream transport, clocks и realtime latency; control JSON не заменяет его |
| Documents, overlays и presenters | Фазы 9/13 либо до первого notes/palette package: durable scoped adapter, quotas/export/GC/crash tests, renderer и DesktopView leases. CAS/retention fakes не доказывают disk persistence |
| Несколько artifacts одного package | До Фазы 12 SDK/installer freeze: versioned lock, связывающий manifest digest и каждый выбранный platform/runtime artifact; миграция/recovery/hash-change tests. Сейчас lock содержит один artifact SHA-256 на package; разные contributions одного binary работают, разные binary внутри одного package не обещаны |
| Personal feeds/calendar | В gates конкретных packages: доступность внешнего API/consent, реальные timezone/DST и reconciliation; fake fixtures сохраняют count units/partial quality, но не доказывают интеграцию Samsung/Telegram/mail |

Compatibility acceptance: отсутствующие новые optional поля старых документов читаются с defaults; пустые additions не меняют старый serialized plan без необходимости. Unknown optional extensions сохраняются, required features проверяются до исполнения. Protobuf additions и generated schemas проходят общий gate. Это совместимость с поддержанными fixtures, а не обещание, что старый binary исполнит новый structured graph. Upgrade/downgrade SDK, multi-artifact lock и migrations получают свои fixtures при реализации соответствующих features.

Samsung Health mobile adapter/registration, реальные mail/Mattermost/contacts/news adapters и renderer performance не блокируют проектирование canvas после закрытия общих контрактов. Они не считаются реализованными fake feed/metric demo и получают отдельные package gates.
