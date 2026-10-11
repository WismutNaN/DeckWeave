# Аудит архитектуры от 2026-10-12

Исторический отчёт первого аудита (commit `ee4b3de`); числа проверок и выводы относятся к той версии. Актуальное сравнение референсов: [исследование реализаций](../research/implementation-review.md), нормативная карта: [документация](../README.md).

**Область:** README, спецификации, ADR, план и все модульные документы в исходном commit `0bcc274`. Репозиторий содержит документацию, но не исполняемый код, schemas или тестовую инфраструктуру. Исправления ниже устраняют противоречия спецификации; они не являются доказательством работающего runtime.

**Результат:** сохранены Rust modular core, per-user desktop host, isolated plugins, capability model и API-first. Уточнены общие failure contracts и зависимости, которые иначе пришлось бы менять после появления hardware/plugins.

## Найденные проблемы и исправления

| № | Приоритет | Проблема и возможное последствие | Исправление / проверка при реализации |
|---:|---|---|---|
| 1 | P1 | Architecture переключала runtime раньше сохранения revision; crash мог вернуть другую конфигурацию, а hardware rollback обещался слишком широко | [Durable commit и crash matrix](../contracts/lifecycle.md#сохранение-и-apply), disk-full/kill fixtures, явный disruptive exclusive handoff |
| 2 | P1 | Revision/session не ограждали pending executions при apply; старый completion мог продолжить action в новой конфигурации | [Activation epochs, drain/cancel и cleanup](../contracts/lifecycle.md#executions-cancellation-и-смена-плана); late-completion и held-input fixtures |
| 3 | P1 | Раздельные edge/value lanes не определяли взаимный порядок; relative encoder мог потерять шаги, touch — release, condition — прочитать будущее value | [Ingress order и типы сигналов](../contracts/lifecycle.md#порядок-сигналов-и-время); value flood/encoder/edge-overflow fixtures |
| 4 | P1 | Monotonic timestamps разных узлов считались общими; replay обещал одинаковый результат при перестановке внешних completions | Clock domains, host receive order и полный replay fixture с outcomes/coalescing; reorder не объявлен эквивалентным |
| 5 | P1 | Idempotency key без durable receipts/retention/payload comparison и один expected revision не определяли retry/apply concurrency | [API preconditions и dedup](../contracts/application-api.md#идемпотентность-и-dry-run), повтор после restart, conflict/expired window, отсутствие exactly-once обещаний для внешних effects |
| 6 | P1 | Auth был отнесён к transports, scopes отложены; grant/profile/confirmation ownership не определён | [ApplicationService authorization](../contracts/security.md#local-api), отдельный grant store, revoke/revalidation, operation-bound confirmation, unresolved imported credentials |
| 7 | P1 | Latest-frame-wins для цепочки partial diffs мог повредить framebuffer; preview мог вернуть старую сцену после apply | [Frame/base IDs, full resync и preview lease](../contracts/lifecycle.md#frames-и-preview); loss/ACK/expiry/apply fixtures |
| 8 | P1 | Random localhost path описывался как origin isolation; соседние PI могли оказаться в общей web authority/storage | [Отдельные origin и storage partition](../contracts/security.md#web-property-editors-и-widgets), cross-context bridge/storage tests |
| 9 | P1 | Module dependencies создавали runtime ↔ diagnostics/testkit, Surface ↔ Widget циклы; lazy platform plugin владел bootstrap credentials/autostart | [ADR-0013](../decisions/0013-dependency-and-host-service-boundaries.md), отдельные host platform services, application ports, production diagnostics и testkit |
| 10 | P1 | Один host не имел enforceable ownership для portable roots; exclusive leases были локальными к provider/port | Single-instance/root locks и canonical resource keys, общий arbiter для active/probe, одна active package version |
| 11 | P1 | Lazy activation требовала binding до обнаружения первого device; index при этом не мог запускать plugin | [Scoped discovery lease](../contracts/lifecycle.md#resources-discovery-и-версии-packages) до первого binding, без implicit output permission |
| 12 | P1 | Rollback/restart не фиксировал artifact/settings version; pure migration могла подразумевать запуск package code | Exact package lock, declarative bounded migrations, opaque preservation и reachability GC; import не запускает executable |
| 13 | P2 | Unknown-field preservation приписывалось Protobuf без учёта SDK и JSON conversion | [Раздельные document/wire правила](../contracts/lifecycle.md#миграции-и-неизвестные-данные), cross-language conformance, opaque relay и required-feature negotiation |
| 14 | P1 | Legacy setSettings конфликтовал с immutable revision/draft; владение runtime settings отсутствовало | [Scoped settings overlay](../contracts/compatibility.md#property-inspector), явный merge в draft и versioned restart fixtures |
| 15 | P1 | State/latch автоматически разрешал цикл, а bounds очереди не предотвращали бесконечный loop | Положительный delay/новое внешнее событие и execution/rate budgets; explicit parallel join/priority и no-blind-retry policy |
| 16 | P2 | M2 после Фазы 9 не закрывал P0 layers/parallel/errors и recording; export/portable откладывались до 17; L-фазы оценивались в 1–5 дней целиком | [План](../implementation/roadmap.md): preview после 9, daily после 11; roots/export/recorder foundation раньше; L-фазы разбиты на срезы, compatibility/backend spikes и gates |

P1 здесь означает риск потери данных/неверного исполнения/нарушения границы или крупного архитектурного переделывания; P2 — существенная неполнота контрактов/плана. Это приоритеты аудита, не продуктовые P0/P1 из требований.

## Проверка технических оснований

- Path не входит в origin tuple: [HTML Standard, Origins](https://html.spec.whatwg.org/multipage/browsers.html#origin). Отсюда требование отдельной web authority; isolation storage дополнительно проверяется выбранным backend.
- Protobuf предупреждает о потере unknown fields при JSON conversion и field-by-field copy: [Proto3 guide](https://protobuf.dev/programming-guides/proto3/#unknowns). В исследованном `prost` derive неизвестный tag обрабатывается `skip_field`: [исходный код генератора](https://github.com/tokio-rs/prost/blob/master/prost-derive/src/lib.rs). Поэтому conformance привязан к выбранной версии SDK, а не названию формата. Версии библиотек фиксируются в Фазе 2; текущие ссылки на upstream не являются dependency lock.

Исходное исследование [stack.md](initial-stack.md) сохранено без переписывания и отделено от нормативных контрактов. Аудит не переутверждает все ранее перечисленные hardware/license claims: они требуют pinned provenance перед заимствованием и hardware fixture перед реализацией.

## Выполненные проверки документации

Автоматическая проверка: 49 Markdown-файлов, 299 локальных ссылок, 106 задач PLAN, 19 модулей и 71 уникальный requirement ID; ошибок paths/anchors/JSON/fences/навигации не обнаружено. Проверка выполнена временным Python-скриптом без добавления runtime/toolchain в репозиторий; постоянный docs-check входит в CI Фазы 1.

- Локальные Markdown paths/anchors, включая новые ADR и runtime contracts.
- Каждая задача PLAN имеет ссылку на спецификацию/модуль; каждый модуль имеет путь из PLAN и docs index.
- Уникальность requirement IDs и наличие адресатов requirement references.
- JSON examples, balanced code fences и `git diff --check`.
- Повторный смысловой проход по apply/cancellation, storage/auth, signal/frame ordering и зависимостям модулей.

Build, runtime tests, hardware conformance и performance benchmarks не запускались: их код ещё отсутствует. В PLAN и таблицу [обязательных fixtures](../contracts/lifecycle.md#обязательные-проверки-реализации) добавлены конкретные условия проверки будущей реализации.

## Что остаётся проверить реализацией

| Вопрос | Где закрывается | Последствие неудачи |
|---|---|---|
| Windows/Linux durability и locks | Фазы 1, 4; storage fault injection | Root/backend не объявляется поддерживаемым |
| N4 Pro firmware, rings/haptic/readback/frame capabilities | Фаза 8, реальное устройство и pinned SDK fixtures | Descriptor честно объявляет unsupported/degraded |
| Renderer throughput и bulk control latency | Фаза 9, benchmark ADR | Выбор другого backend/bulk channel, без изменения scene semantics |
| Browser isolation и offscreen rendering | Фаза 13, Windows/Linux spike | Пересмотр backend/объёма, без ослабления privileged boundary |
| Legacy registration/settings/lifecycle | Ранний spike Фазы 3, production Фаза 14 | Ограниченная exact-version compatibility matrix |
| Media ABI, remote clock/capture, privileged intercept | Post-v1 ADR и benchmarks | Не включаются в стабильный v1 contract преждевременно |

Для перехода к коду достаточно начать Фазу 1. Дальнейшее увеличение спецификации не заменяет проверку этих гипотез рабочими срезами.
