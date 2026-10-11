# Документация DeckWeave

DeckWeave объединяет устройства и приложения через плагины: ввод, действия с результатами, визуальные поверхности, подсветку и будущие media streams. Текущий статус — **архитектурная спецификация; исполняемый код не создан**. Контракты уточнены сравнением исходников 2026-10-12; работа начинается с [Фазы 1](implementation/roadmap.md#фаза-1--скелет-репозитория).

## Порядок чтения

1. [Замысел](product/vision.md), [сценарии](product/scenarios.md) и [требования](product/requirements.md): что должно получиться и как это проверить.
2. [Архитектура](architecture/overview.md): владельцы состояния, процессы и зависимости.
3. Нужный контракт из таблицы ниже и соответствующая [граница модуля](architecture/modules/README.md).
4. [План реализации](implementation/roadmap.md): ближайший рабочий срез и acceptance gate.
5. [Решения](decisions/README.md) и [исследование исходников](research/implementation-review.md): почему выбраны эти границы.

## Карта разделов

| Каталог | Что хранится | Точка входа |
|---|---|---|
| `product/` | Назначение, требования, сценарии, режимы и UX | [Замысел](product/vision.md), [режимы](product/modes.md), [UX](product/ux.md) |
| `architecture/` | Устройство системы и границы модулей | [Обзор](architecture/overview.md), [модули](architecture/modules/README.md) |
| `contracts/` | Нормативное поведение на границах | Таблица ниже |
| `implementation/` | Порядок реализации, срезы и проверки | [План](implementation/roadmap.md) |
| `decisions/` | Нумерованные ADR с причиной выбора | [Реестр ADR](decisions/README.md) |
| `research/` | Первичные источники, наблюдения, сравнение и reuse-кандидаты | [Референсы](research/REFERENCES.md), [анализ реализаций](research/implementation-review.md) |
| `archive/` | Исторические материалы, не текущие требования | [Исходный стек](archive/initial-stack.md), [первый аудит](archive/2026-10-12-initial-audit.md) |

## Контракты

| Документ | Единственный владелец подробных правил |
|---|---|
| [Capabilities](contracts/capabilities.md) | Identity, descriptors, port semantics, dynamic topology и negotiation |
| [Plugins](contracts/plugins.md) | Package/manifest, runtime classes, handshake, feature/limit negotiation и SDK |
| [Lifecycle](contracts/lifecycle.md) | Revisions/apply/recovery, epochs, ingress order, leases и frame delivery |
| [Automation](contracts/automation.md) | Typed results/jobs, run scope, artifacts, collections, joins и layers |
| [Lighting](contracts/lighting.md) | Physical geometry, key/LED mapping, effects, output ownership и onboard границы |
| [Media](contracts/media.md) | Будущие negotiated streams, buffer ownership, clock и RT lifecycle; ABI ещё не выбран |
| [Application API](contracts/application-api.md) | Commands/queries/subscriptions, authorization, dedup и adapters |
| [Security](contracts/security.md) | Trust, grants, private data, privileged operations и web isolation |
| [Compatibility](contracts/compatibility.md) | OpenAction/Elgato levels, context/settings mapping и ограниченные legacy results |

## Как расширять документацию

- Новая пользовательская способность начинается со сценария и требования с постоянным ID. Статус design/fixture/real integration различается явно.
- Правило поведения записывается в один тематический contract. Обзор и module file кратко описывают границу и ссылаются на правило; plan задаёт проверку, не пересказывает protocol.
- Изменение общего решения получает следующий ADR; небольшой provider-specific detail остаётся в модуле/исследовании. Новый документ создаётся при отдельной ответственности, а не для каждой сессии обсуждения.
- Исследование разделяет source observation, вывод для DeckWeave и непроверенную гипотезу. Репозитории/файлы имеют pinned commit; hardware и бинарные releases требуют отдельного evidence.
- Исторические исследования и отчёты уходят в `archive/` с явным статусом. Они не изменяют актуальные контракты. Исходный `initial-stack.md` сохранён без переписывания.
- При переносе файла обновляются все локальные links/anchors и корневые индексы. Redirect-копии и параллельные версии одной спецификации не создаются.
- Перед commit проверяются local links/anchors, requirement IDs, task→contract/module links, индексы, JSON examples и отсутствие противоречий между requirement, contract и plan.

При расхождении product определяет желаемый результат, contracts — точное поведение, ADR — причину выбора, plan — сроки проверки. Несовместимые утверждения исправляются вместе; исследование или старый audit не имеют права молча переопределить contract. Принятое решение ещё не является runtime-гарантией.

## Перенос прежней структуры

Корневые `docs/PREPROJECT.md`, `REQUIREMENTS.md`, `MODES.md`, `UI.md` перенесены в `product/`; `ARCHITECTURE.md` — в `architecture/overview.md`; модели/API/security/lifecycle — в `contracts/`; `PLAN.md` — в `implementation/roadmap.md`. Прежний корневой `modules/` теперь находится в `docs/architecture/modules/`, `docs/adr/` — в `docs/decisions/`. Путь [research/REFERENCES.md](research/REFERENCES.md) сохранён как стабильная точка входа в референсы.
