# ADR-0010: Компилируемый automation graph

**Дата:** 2026-10-11  
**Статус:** Принято

## Контекст

Пользователь хочет простые bindings, сложные conditions, layers, parallel/sequence actions, configurable errors и replay. Исполнение mutable UI graph напрямую усложняет latency, validation и rollback.

## Рассматриваемые варианты

1. **Цепочка async callbacks** — легко начать, трудно обеспечить порядок и replay.
2. **Event sourcing/CQRS** — полная история, но чрезмерно для локального продукта.
3. **Draft graph → immutable ExecutionPlan** — validation/optimization до apply и deterministic scheduler.

## Решение

Выбрали **компиляцию validated draft graph в immutable ExecutionPlan**. Workspace scheduler сериализует state transitions, async effects возвращаются continuations, а записанные envelopes воспроизводятся с fake clock.

## Последствия

**Становится проще:** predictable hot path, simulation, trace, rollback и cycle detection.  
**Становится сложнее:** нужен compiler и явная семантика каждого node.  
**Закрывает дверь на:** произвольную мутацию active graph plugin-ом и скрытые async side effects.

