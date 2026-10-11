# ADR-0014: Общие границы и специализированные пути исполнения

**Статус:** принято для спецификации, требуется проверка fixtures.
**Дата:** 2026-10-12.

## Контекст

Команда без типизированного результата не позволяет надёжно связать транскрибацию с обработкой текста и browser action. Попытка передать тем же механизмом каждый LED tick и audio buffer создаёт очереди и неопределённые lifetimes. Сравнение [референсов](../research/implementation-review.md) подтверждает полезность отдельных control, workflow, visual и media semantics.

## Решение

Сохраняем общий package format, identity, grants, revisions, resource ownership и lifecycle. Engine обрабатывает дискретные events/state и координирует jobs с typed input/results и run-local DataEdges. Host предоставляет bounded runtime artifact broker. Surface Compositor владеет display/light output arbitration; media runtime согласует отдельный buffer data plane.

Workflow не выделяется в отдельный процесс/базу данных ради терминологии: это часть Automation Engine с async dispatcher. Artifact storage отделён от profile assets. Native descriptors различают templates, runtime instances и atomic descriptor revisions. Unknown required semantics блокируют route, unsupported media feature не мешает использованию control contribution того же package.

Нормативные детали: [automation](../contracts/automation.md), [lighting](../contracts/lighting.md), [media](../contracts/media.md), [plugins](../contracts/plugins.md).

## Альтернативы

- Один generic message bus: прост в начале, оставляет ordering, completion и buffer lifetime каждому plugin; отклонён.
- Node-RED как обязательный engine: приносит ecosystem, но также отдельные state/runtime semantics; допускается будущим adapter, не core dependency.
- Полностью собственные DSP/ASR/device stacks: слишком большой объём; предпочитаем существующие libraries/processes за adapters.

## Последствия и проверки

Публичный контракт становится немного шире до SDK freeze, зато избегает поздней замены CommandIn/state semantics. Фазы 2/5/11 проверяют mixed descriptors, concurrent results, artifacts и failure policies. Production audio и конкретные внешние integrations остаются отдельными gates. Общий typed contract не обещает поддержку произвольного API без provider.
