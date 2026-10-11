# ADR-0013: Направление зависимостей и системные сервисы host

**Дата:** 2026-10-12

**Статус:** Принято; уточняет ADR-0001, ADR-0007, ADR-0008 и ADR-0011

## Контекст

Module tables смешивали compile dependencies и consumers. Diagnostics зависел от всех runtimes, которые сами зависели от Diagnostics; Surface и Widget ссылались друг на друга. Autostart/credential access были отнесены к lazy plugin, хотя нужны для запуска и авторизации самого plugin. Permissive SDK не должен транзитивно импортировать GPL application domain.

## Рассматриваемые варианты

1. **Связать компоненты напрямую** — быстрый старт, затем циклы crates и bootstrap.
2. **Вынести всё в процессы** — избыточный IPC и сложный lifecycle.
3. **Контракты внутрь, реализации снаружи, composition в desktop host** — явный ациклический граф без новых служб.

## Решение

`deckweave-protocol` содержит permissive wire types/SDK contracts и не импортирует GPL crates. `deckweave-domain` содержит application domain; `deckweave-application` — commands/queries и service ports без implementations. Host реализует их, API adapters зависят от contracts, desktop composition root соединяет реализации.

`deckweave-diagnostics` содержит production instrumentation/sinks и зависит только от domain/protocol и logging libraries. Testkit зависит от тестируемых компонентов через dev/test paths; production crates не импортируют testkit. Device Studio application service находится в `deckweave-device-studio`, а не в testkit. Surface принимает `WidgetFrame` через protocol/trait без зависимости от widget-host.

Production simulation/replay находится в Automation Engine как `SimulationRunner` с isolated state/fixture outcomes и не подключает live dispatcher. Testkit строит поверх него test harness; API/Device Studio не зависят от testkit ради replay.

`deckweave-platform-host` реализует минимальные injected OS services: data roots, instance locks, autostart, credentials, child-process cleanup и metadata notifications. Он не публикует automation actions и не содержит vendor logic. Volume/input/window/application actions остаются isolated platform plugins. Profile Store не импортирует OS keychain; authorization/credential broker принадлежит host и передаёт только scoped references/results.

В v1 один host на user session и один active workspace; несколько portable roots не могут одновременно владеть hardware. Подробнее — [контракты владения](../contracts/lifecycle.md#владение-host-и-workspace).

## Последствия

**Становится проще:** компиляция без циклов, headless reuse, независимые SDK и запуск без secrets-plugin bootstrap.

**Становится сложнее:** composition root явно передаёт contracts/adapters и проверяет dependency graph в CI.

**Закрывает дверь на:** production dependency от testkit и зависимость SDK от GPL host/domain implementation.
