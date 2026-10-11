# ADR-0008: Application API и optimistic revisions

**Дата:** 2026-10-11  
**Статус:** Принято

## Контекст

Desktop UI, CLI, Codex и future MCP должны выполнять одинаковые операции. Прямое редактирование profile files или отдельные Tauri-only commands создадут несовместимые пути и потерю изменений при параллельном редактировании.

## Рассматриваемые варианты

1. **UI вызывает внутренние функции, внешние clients меняют файлы** — быстро, но семантика расходится.
2. **REST является доменной моделью** — внешне удобно, но связывает core с transport.
3. **Application commands/queries + transport adapters** — единая семантика, optimistic concurrency и тестируемость.

## Решение

Выбрали **единый ApplicationService** с commands, queries, subscriptions, expected revision, idempotency key, validate/simulate/plan/apply. Desktop использует direct adapter; CLI/Codex — authenticated loopback HTTP/WebSocket; MCP является optional adapter.

## Последствия

Уточнение 2026-10-12: [API.md](../API.md) задаёт command-specific preconditions, persisted dedup receipts и principal authorization внутри service. Apply проверяет draft version и active revision раздельно. Contracts находятся в `deckweave-application`, adapters не импортируют Host implementation ([ADR-0013](0013-dependency-and-host-service-boundaries.md)).

**Становится проще:** автоматизация через Codex, contract tests, conflict handling и смена transport.  
**Становится сложнее:** каждый UI action сначала оформляется application command.  
**Закрывает дверь на:** приватные возможности UI и direct file mutation как поддерживаемый workflow.
