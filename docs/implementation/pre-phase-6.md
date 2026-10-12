# Gate расширяемости перед Фазой 6

Статус: **открыт; backend foundations расширены** после повторного аудита 2026-10-12. Исходный аудит относится к `2d99c88`; таблица ниже фиксирует последующие изменения реализации. Модели instances/arguments уже исполняются, но весь gate ещё не доказан. Основание: [CG-01–15](../research/plugin-feasibility.md), сценарии: [каталог PLG](../plugins/README.md), сводный путь данных: [граница ядра и плагина](../contracts/plugin-boundary.md).

## Проверенный результат повторного аудита

| Область | Реализация и evidence | Что ещё нужно проверить или подключить |
|---|---|---|
| Storage | Checkpoint, cached quota accounting, retired claim GC с защитой pending apply, terminal apply archive вне startup scan. Три tests в `profiles/tests/support/compaction.rs`: два fault points, rollback wall clock, restart/replay и pending/history | Автоматический budget/schedule checkpoint для длительной editor session, growth/IO benchmark и policy размера terminal archive; power-loss не доказан fault injection |
| Instances | Settings/schema/revision/CredentialRefs, required features, несколько leases одной group, endpoint ownership по instance. `configured_instances` запускает два аккаунта в одном процессе и восстанавливает настройки после restart | Разные contributions с held effects и unresolved/revoked credentials; secret lease/use/renew не заменяется передачей CredentialRef |
| Bindings | Structural schemas, wire SchemaRef, finite ranges, single-trigger named arguments, compiler field/type checks, run-local results. `argument_compiler` проверяет compiler → scheduler; `structured_arguments` — concurrent/malformed data | Полные collection/join/parallel operators остаются Фазой 11; implicit unit conversion запрещён |
| Live execution | Общий monotonic clock, timer pump, provider budget, ordered scheduler/dispatch, activation commit после routes, cleanup receipt/deadline и stop drain | Negative/missing ACK при shutdown доказаны новым real-process test. Остаются held-output fixture с другим живым owner, apply/revoke/crash и mixed ingress/overflow в настоящем bridge; descriptor QoS проверен отдельно |
| Presentation | ItemRef/revision/hit capture, slot next/back/consumer count, DesktopViewSession lease model; InteractionSession query fence/schema/retention и owner/provider cancellation | Persistent-view lifecycle и scoped document CAS/export fake проверены. Остаются общий presenter + tasks/windows/notes, asset fake и mixed unread/metric/date fixtures |
| Composition | Desktop: PackageGraphCompiler, static catalog по package lock, ScopedAuthorizationStore. Credential preflight подключён; default unresolved блокирует activation до запуска процесса | End-to-end credential-resolution/revoke fixture, persistent locale preference/catalog loading application use case |
| Services | Typed role bindings, transitive resolver, cycle/optional/missing/artifact conflicts; delegation intersection/deadline/fence и subscription gap rules с tests | Live broker ещё не подключён: serviceBindings сохраняются, но compiler явно блокирует execution; нужна fake nested call/subscription lifecycle acceptance |

Следующие обязательные срезы: (1) cleanup/ingress conformance; (2) presentation/data/collection fakes; (3) credential + locale application use cases; (4) storage growth/maintenance acceptance и integrated gate. Реальные hardware, DSP, renderer и внешние services имеют собственные более поздние этапы. Наличие DTO и успешный M1 не закрывают перечисленные проверки.

## Порядок

Каждый срез оценить на 1–5 дней; при превышении разделить на ports/model, implementation и integration acceptance. До gate допустимы UI sketches и read-only shell; persistent inspector/binding schema и production canvas опираются на закрытый gate. Нельзя сохранять непроверенные extensions как будто они уже исполняются.

### 6G.0 — Хранение перед нагрузкой editor

- [ ] Закрыть B2 checkpoint/compaction/quota accounting с recovery/retention tests; не смешивать plugin data с workspace history (→ [Profile Store](../architecture/modules/profile-store.md), [plugin data](../contracts/plugin-composition.md#plugin-data)).

Выход: растущая история не превращает обычную mutation в scan всех receipts; crash при compaction сохраняет pending/claims/receipts и выбранные revisions.

### 6G.1 — Instances, settings и зависимости

- [x] Ввести versioned configured-instance/settings/binding model в Workspace/Plugin Runtime ports и protocol; package/group/session/physical target остаются разными IDs (→ [композиция](../contracts/plugin-composition.md), CG-01/06).
- [x] Реализовать activation нескольких contributions/instances одной group с их settings и scoped CredentialRefs; добавить required-feature compatibility и import preserving unknown optional data (→ [Plugins](../contracts/plugins.md), [Lifecycle migrations](../contracts/lifecycle.md#миграции-и-неизвестные-данные)).
- [ ] Описать/проверить fake resolver transitive requirements, explicit provider selection, missing/optional/cyclic/version-conflict и locked artifacts. Зафиксировать service-call/delegation/subscription port до UI; полноценный shared service broker допускается позже до первого зависимого plugin (→ [ADR-0019](../decisions/0019-plugin-composition.md)).

Выход: два аккаунта и state+action contributions в одной group не смешивают настройки/данные; restart/rollback сохраняет выбранные bindings; provider не выбирается случайно. SDK library/recipe reuse не требует лишнего runtime dependency.

### 6G.2 — Сложные данные и аргументы actions

- [x] Проверять wire schema/encoding, scalar range, bounded nested records/arrays, enum/optional/null; forbidden/unknown required structure не проходит как общий Json (→ [Automation](../contracts/automation.md), CG-02).
- [x] Ввести minimal typed value binding: constants, result/state field references, named inputs и явный trigger. Ошибка missing field/schema не вызывает action; сбор arguments не превращается в отдельный invocation на каждый data edge (→ [Automation](../contracts/automation.md#structured-bindings), CG-03).

Выход: fake transcript → task `{title, description}` и task ID → complete проходят через общий compiler/scheduler; две concurrent chains изолированы. Неверная вложенная структура, чужой SchemaRef и несовместимые units отклоняются. Полные sequence/parallel/join/collections operators остаются Фазой 11; editor не имитирует их JSON-конкатенацией.

### 6G.3 — Live clock, cleanup и ingress

- [x] Подключить monotonic clock и bounded timer pump к LiveExecutionOwner, согласовать provider deadlines; проверить timeout без дополнительных событий (→ [time contract](../contracts/automation.md#time-and-schedules), CG-04).
- [ ] Реализовать owner-scoped cleanup через настоящие host/provider ports, ACK/timeout/fault path; success не возвращается за пропущенный release. Для outputs определить lease/cleanup после crash процесса (→ [Lifecycle effects](../contracts/lifecycle.md#executions-cancellation-и-смена-плана), CG-05).
- [ ] В live bridge брать QoS из descriptor: reliable edge/delta/terminal отдельно от coalescible state/value; test mixed metrics + key release + completion при overflow (→ [Lifecycle ordering](../contracts/lifecycle.md#порядок-сигналов-и-время)).

Выход: fake provider удерживает output до release своего owner; apply/revoke/disconnect очищают его, чужой owner жив. Pending job заканчивается по deadline при тишине, 10 минут host uptime не обнуляют время. Не нужно внедрять OS hooks или Pomodoro UI для этих проверок.

### 6G.4 — Представления, коллекции и данные plugin

- [ ] Зафиксировать ItemRef/revision и presentation hit binding, navigation/back, scene selection без workspace apply. Fake presenter: task list, windows и notes используют один contract (→ [presentation](../contracts/presentation.md), CG-08).
- [x] Развести InteractionSession и DesktopViewSession; добавить query-generation/owner/deadline/retention модель и fake lifecycle (→ [Interaction Runtime](../architecture/modules/interaction-runtime.md), CG-07).
- [ ] Определить scoped document/asset storage ports и retention/export/revision policy; fake private notes/palette survive view close, workspace rollback не откатывает их молча (→ [plugin data](../contracts/plugin-composition.md#plugin-data), CG-09).
- [ ] Зафиксировать общий presenter service и slot-local widget selection; два slots меняются независимо, hidden widgets не продолжают render/tick без consumer (→ [slot contract](../contracts/presentation.md#slot-switching), CG-14).
- [ ] Добавить fake mixed unread/feed и personal metric/date fixtures: разные count units/partial source, no read-on-render, corrected/overlapping measurements и timezone boundaries (→ [личные данные и ленты](../plugins/feeds-and-personal.md), CG-13/15).

Выход: перестановка списка между render/click не меняет target операции; два displays независимы; close edit не удаляет note/view, late search не подменяет актуальный query. Contracts/fixtures готовы до canvas; настоящий renderer/always-on-top и durable plugin store adapter реализуются перед соответствующими packages в Фазах 9/13 либо отдельном срезе. Не вводить новую базу данных только ради этих mocks.

### 6G.5 — Production composition

- [ ] Подключить catalog/graph compiler и scoped authorization/credential ports в desktop composition вместо fixture-only planning; тот же application service работает без editor (→ [Host Runtime](../architecture/modules/host-runtime.md), CG-10).
- [ ] Проверить static package/schema metadata → configured instances → plan/apply → real isolated virtual providers → trace → restart. Metadata не появляется магически из уже активной session; import/index не запускают code (→ [Automation Engine](../architecture/modules/automation-engine.md)).

Выход: production composition применяет настоящий graph, видит denied/unresolved permissions и показывает diagnostics через API. Нельзя заменять DenyAll на AllowAll ради успешного demo. Persistent grant management требует отдельного review; fake authorization допустима только в harness.

## Финальная демонстрация и разрешение перехода

- [ ] Один reusable collection presenter получает fake tasks/windows/notes; item actions адресуют конкретную revision (→ [PLG-022](../plugins/productivity.md#plg-022)).
- [ ] Fake layout consumer использует выбранный fake window service; fake hub использует выбранный fake MQTT role. Lifecycle/delegation можно доказать test port без сетевых credentials (→ [композиция](../contracts/plugin-composition.md)).
- [ ] Fake transcript/HTTP record → task create показывает structured bindings; two instances не смешиваются (→ [PLG-009](../plugins/integrations.md#plg-009)).
- [ ] Fake clock/game/held-output/telemetry fixture доказывает order, deadlines и cleanup; physical frame latency измеряется позже (→ [PLG-021](../plugins/productivity.md#plg-021)).
- [ ] Зафиксировать результаты, оставшиеся platform limits и backward compatibility. B2 и live-path fixes обязательны; прочие целевые runtime features имеют назначенный этап и не выдаются за готовые (→ [исследование](../research/plugin-feasibility.md)).

Только после этих доказательств gate меняется на closed. Локально запускать новые отрицательные tests и затронутые suites; не повторять старый набор ради количества. На интеграционном выходе — общий core/compatibility/docs gate. Внешние Singularity/Zotero/Handy/Sprut, hardware и real overlays проходят собственную приёмку позже.

Samsung Health mobile adapter/registration, реальные mail/Mattermost/contacts/news adapters и renderer performance не блокируют проектирование canvas после закрытия общих контрактов. Они не считаются реализованными fake feed/metric demo и получают отдельные package gates.
