# DDD и границы контекстов DeckWeave

Этот документ задаёт нормативные правила модульного монолита DeckWeave. DDD здесь означает не каталог с именем `domain`, а явное владение терминами, инвариантами и use cases. Размер файла является только сопровождающей метрикой.

## Карта контекстов

```text
Desktop / CLI / Codex
          |
          v
Application Gateway --------------------------+
  commands · queries · auth · operations      |
     |             |             |             |
     v             v             v             v
Workspace       Host          Automation   Interaction UI
Configuration   Lifecycle     Execution    Sessions
     |             |             |             |
     +-------------+-------------+-------------+
                   |
                   v
          Shared Kernel + ports
                   |
        +----------+-----------+
        v                      v
 Plugin runtimes          Infrastructure adapters
 Platform Input           files · OS · web · devices
```

Стрелка означает зависимость через публичный контракт или port. Контексты не импортируют чужую infrastructure-реализацию и не изменяют принадлежащие другому контексту агрегаты напрямую.

## Bounded contexts

| Контекст | Владеет | Не владеет |
|---|---|---|
| Workspace Configuration | draft, команды редактирования, versions, immutable revisions, snapshots, package lock, export/import policy | запуск runtime, authorization principal, OS-файлы |
| Host Lifecycle | startup/shutdown, выбранная active revision, apply barrier, activation epoch, recovery и health | редактирование draft, transport auth, automation rules |
| Application Gateway | command/query DTO, principal/scopes, use-case routing, stable errors, operation tracking | доменные инварианты workspace, filesystem и hardware |
| Automation Execution | graph validation/compile, run state, ordering, timers, results, error policies | окна UI, plugin process lifecycle, profile storage |
| Capability Registry | nodes, devices, sessions, elements, ports, descriptors и availability | platform-specific discovery и бизнес-сценарии |
| Plugin Runtime | package index, grants-to-session activation, process lifecycle, handshake и health | workspace semantics и пользовательские окна |
| Surface Composition | непрерывные scenes, hit regions, light scenes, frame arbitration | device transport и редактор workspace |
| Platform Input | наблюдение, interception и emission системного ввода, source identity/granularity | выбор automation action и общие host locks |
| Interaction UI | жизненный цикл modal/palette sessions, focus/cancel/timeout, view policy и typed result | поиск Zotero, произвольные Tauri-команды, automation continuation |

Module specifications уточняют ports каждого контекста. Если термин встречается в нескольких контекстах, его смысл и владелец определяются этой таблицей, а обмен идёт через ID, DTO или событие.

## Внутренние слои контекста

Каждый context может содержать четыре слоя. Наличие отдельной директории не обязательно для малого среза, но направление зависимостей обязательно.

1. **Domain** — aggregates, value objects, domain services и ошибки инвариантов; не знает transport, filesystem, process и UI.
2. **Application** — use cases, commands/queries и ports; координирует domain, но не реализует I/O.
3. **Infrastructure** — filesystem, OS, process, database и network adapters к ports.
4. **Interface** — HTTP/Tauri/CLI/plugin protocol translators; преобразует boundary DTO и не содержит business rules.

Допустимое направление: `interface/infrastructure -> application -> domain -> shared kernel`. Infrastructure подключается composition root и не становится зависимостью application.

## Shared Kernel

`deckweave-domain` и `deckweave-protocol` образуют минимальный Shared Kernel, а не общий склад типов.

Тип добавляется туда только если:

- его семантика стабильна и нужна как минимум двум bounded contexts;
- у него один документированный смысл и правила версионирования;
- он не содержит use case конкретного контекста;
- зависимость не тянет OS, UI, storage или runtime implementation.

`serde_json::Value` допустим на extension/document boundary и для сохранения неизвестных полей. Он не заменяет value objects и проверяемые инварианты внутри domain.

## Агрегаты и транзакционные границы

- `WorkspaceDraft` — aggregate Workspace Configuration; только его команды меняют draft и увеличивают `DraftVersion`.
- `WorkspaceRevision` неизменяем после публикации. Snapshot ссылается на revision, rollback создаёт новый draft/revision.
- Active selection и `ActivationEpoch` принадлежат Host Lifecycle. Workspace Configuration публикует candidate, но не объявляет его активным.
- `ExecutionPlan` принадлежит Automation Execution и строится только из согласованного revision snapshot.
- Device/session topology принадлежит Capability Registry; provider сообщает факты, но не редактирует registry storage напрямую.
- `InteractionSession` принадлежит Interaction UI. Provider возвращает typed items/results, а Automation связывает terminal result с конкретным invocation.

Межконтекстная атомарность не имитируется общей mutable transaction. Она выражается application use case, durable record, idempotency key и компенсацией/recovery, как в apply protocol.

## Плагины как граница расширения

Плагин публикует contributions через versioned protocol. Он не становится частью доменной модели host и не получает ссылку на внутренние services.

- device/input provider объявляет capabilities и события;
- action/query provider исполняет внешнюю операцию и возвращает typed outcome;
- interaction provider поставляет query/action schema и, при необходимости, ограниченное представление;
- surface widget поставляет визуальный слой для device surface;
- host проверяет permissions, limits, session generation и lifecycle для каждого contribution.

Поставляемые с приложением first-party packages проходят тот же contract. Они могут выглядеть встроенными в UI, но не получают скрытый privileged API.

## Правила кода и автоматические границы

Обязательные правила:

1. Crate root (`lib.rs`, `main.rs`) экспортирует modules и composition wiring, но не накапливает use cases.
2. Контекст не импортирует infrastructure другого контекста. Обмен — через application port или stable event/DTO.
3. Transport, Tauri command, plugin process adapter и filesystem adapter не содержат business logic.
4. Каждый инвариант имеет одного владельца; повторная проверка на boundary может защищать вход, но не определяет вторую семантику.
5. Новый Shared Kernel type требует двух потребителей и ссылки на contract/ADR.
6. Production module целится в `<= 400` строк, crate root — в `<= 150`. Это ratchet: существующий превышающий файл не растёт и получает migration owner. Исключение документирует причину, владельца и срок пересмотра.
7. Архитектурная проверка запрещает обратные dependencies и не допускает `testkit` в production graph.

Целевые размеры не требуют механического дробления связного aggregate. Если модуль остаётся крупным из-за единой модели, решение фиксируется рядом с проверкой; смешение domain, persistence и adapter code исключением не считается.

## Стратегия тестирования

| Уровень | Что проверяет | Допустимые зависимости |
|---|---|---|
| Domain unit | инварианты aggregate/value objects | fake clock/IDs без I/O |
| Application use case | orchestration, authorization decision, conflict/error mapping | in-memory ports |
| Adapter contract | atomic storage, OS/provider translation, protocol limits | real adapter или fault fixture |
| Scenario | цепочку между contexts | public application API, controlled fakes |
| Architecture | dependency direction, crate-root/module ratchet | source/Cargo graph |

Новая функция не считается покрытой одним end-to-end тестом: владелец инварианта получает unit test, port — contract test, а критический межконтекстный путь — scenario/fault test.

## Текущая миграция

| Сейчас | Целевая ответственность |
|---|---|
| Workspace Configuration | `deckweave-workspace` владеет aggregate, commands, snapshot/export policy и repository ports; `deckweave-profiles` — append-only/filesystem adapter |
| Host и API | `deckweave-host` владеет lifecycle; `deckweave-application::ApplicationGateway` — dispatch/auth/errors |
| OS bootstrap | ports находятся в `deckweave-platform`, реализации разделены на launch/storage/process adapters в `deckweave-platform-host` |
| новые расширения | `deckweave-platform-input` и `deckweave-interaction` фиксируют контракты/fixtures без преждевременных OS/web integrations |

Миграция R1–R4 выполнена strangler-подходом без изменения persisted/wire schema. Compatibility re-export OS ports сохранён в `deckweave-application` только для внешних потребителей; новый код импортирует `deckweave-platform` напрямую. Полный порядок и gate продолжения описаны в [roadmap](../implementation/roadmap.md#ddd-migration).

## Антипаттерны

- универсальный `Manager`/`Service`, который меняет несколько aggregates;
- импорт Profile Store из automation или UI;
- Tauri command, напрямую меняющая JSON draft;
- plugin-specific поле в общем domain без namespaced extension;
- один enum ошибок для всех контекстов с потерей их семантики;
- произвольное окно/HTML плагина с доступом к host API;
- объявление обычной клавиатуры «особым встроенным устройством» в core вместо first-party provider.
