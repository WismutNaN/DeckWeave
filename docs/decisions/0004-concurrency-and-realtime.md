# ADR-0004: Конкурентность и разделение real-time

**Дата:** 2026-10-11  
**Статус:** Принято

## Контекст

Кнопки, state updates и USB-команды терпят планирование async runtime. Аудио, видео и некоторые haptic/force-feedback paths требуют предсказуемых buffers и запрещают allocation/locks в callback.

## Рассматриваемые варианты

1. **Один общий Tokio event bus** — просто, но не даёт real-time guarantees.
2. **Отдельный thread на каждый объект** — понятное владение, но неограниченное число потоков и сложная координация.
3. **Compiled deterministic control plane + отдельный media data plane** — разные QoS и явная граница.

## Решение

Выбрали **Tokio control plane с сериализованными owners, immutable ExecutionPlan и bounded channels**. Edge events и continuous values имеют разные очереди/backpressure semantics; async actions не блокируют workspace scheduler. Media buffers передаются внутри отдельного media runtime через профильные APIs/shared buffers. UI получает snapshots и никогда не блокирует engine.

## Последствия

Уточнение 2026-10-12: [контракт сигналов](../contracts/lifecycle.md#порядок-сигналов-и-время) задаёт общий ingress order и ограничивает coalescing границами edge/timer/barrier. Encoder deltas не являются latest values; clocks разных источников напрямую не сравниваются.

**Становится проще:** измерять latency, применять backpressure и не смешивать аудио с automation events.  
**Становится сложнее:** control parameters и media graph должны иметь отдельный bridge.  
**Закрывает дверь на:** обработку микрофонных buffers обычным ActionProvider или WebSocket handler.
