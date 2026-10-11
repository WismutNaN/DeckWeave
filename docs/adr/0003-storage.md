# ADR-0003: Хранение профилей и состояния

**Дата:** 2026-10-11  
**Статус:** Принято

## Контекст

Пользовательские workspaces невелики, должны экспортироваться, сравниваться и мигрироваться. Обязательная БД усложнит восстановление и не решит задач, которых нет в первой версии.

## Рассматриваемые варианты

1. **SQLite для всех данных** — транзакции и запросы, но непрозрачный экспорт и лишняя схема на старте.
2. **Только один JSON-файл** — простота, но неудобные assets, backup и крупные документы.
3. **Версионированные JSON-документы + immutable revisions + content-addressed assets + atomic replace** — прозрачность и контролируемое recovery.
4. **Event sourcing** — полная история, но высокая сложность без продуктовой необходимости.

## Решение

Выбрали **версионированные JSON-документы**, immutable applied revisions, in-memory undo/redo command log для draft, named snapshots, отдельное asset storage, temporary file + fsync/rename + backup и migration reports. Rollback создаёт новую revision на основе старой. Секреты находятся в OS credential store. SQLite может быть добавлен только как производный cache/index после измерения потребности.

## Последствия

Уточнение 2026-10-12: [ADR-0012](0012-apply-and-execution-lifecycle.md) задаёт durable commit record до runtime cutover. `fsync/rename` выше обозначает требование к storage adapter, а не готовую одинаковую реализацию для всех ОС. Revision/assets/package lock/receipt публикуются в заданном порядке; undo stack может быть in-memory, подтверждённый draft document — durable. Grants не входят в переносимую revision.

**Становится проще:** экспорт, diff, backup, ручная диагностика и перенос профилей.  
**Становится сложнее:** cross-document references и migrations нужно проектировать явно.  
**Закрывает дверь на:** хранение telemetry/event history без отдельного bounded журнала или будущего index store.
