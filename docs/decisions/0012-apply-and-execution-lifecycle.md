# ADR-0012: Фиксация apply и жизненный цикл effects

**Дата:** 2026-10-12

**Статус:** Принято; уточняет ADR-0003, ADR-0004, ADR-0005 и ADR-0010

## Контекст

Логическое переключение плана, файловое сохранение и внешний side effect нельзя сделать одной аппаратной транзакцией. Прежнее описание сохраняло revision после cutover и не определяло судьбу pending actions, exclusive leases и held outputs. Это допускало потерю применённой конфигурации и выполнение старых continuations в новом плане.

## Рассматриваемые варианты

1. **Сначала runtime, затем сохранение** — короткий путь, но crash возвращает другую конфигурацию.
2. **Обещать общую ACID-транзакцию** — неприменимо к USB, процессам и внешним API.
3. **Durable commit record и execution epochs** — явное recovery и ограниченная остановка на cutover.

## Решение

Сначала сохраняются immutable revision/assets/package lock, затем готовятся resources. На barrier закрывается dispatcher, выполняется drain/cancel и атомарно публикуется commit record. После commit новый plan становится выбранным даже при degraded activation; возврат — отдельный apply. Executions/frames/timers ограждены activation epoch, отмена не обещает отмену внешнего эффекта. Exclusive handoff требует объявленного downtime; безопасный retry зависит от provider idempotency.

Нормативный порядок, crash matrix, сигналы и frame semantics определены в [контрактах исполнения](../contracts/lifecycle.md).

## Последствия

**Становится проще:** определить выбранную revision после crash, отсечь late effects, проверить failure stages.

**Становится сложнее:** нужны durable receipts, resource reservations, barrier и cleanup held outputs.

**Закрывает дверь на:** silent partial graph apply, blind retry unknown effects и гарантию бесшовной замены exclusive provider.
