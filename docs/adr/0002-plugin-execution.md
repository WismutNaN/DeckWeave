# ADR-0002: Модель исполнения плагинов

**Дата:** 2026-10-11  
**Статус:** Принято

## Контекст

Один package может работать с HID, предоставлять действия, отображать property inspector или обрабатывать аудиобуферы. Один IPC/ABI не способен одновременно дать языковую независимость, crash isolation и hard real-time guarantees.

## Рассматриваемые варианты

1. **Все плагины как DLL в host** — низкий overhead, но любой crash повреждает приложение и ABI трудно версионировать.
2. **Все плагины как процессы с JSON/WebSocket** — хорошая изоляция, но непригодно для audio/video buffers.
3. **Все плагины как WASM** — сильная sandbox, но ограниченный hardware/native ecosystem и real-time интеграции.
4. **Общий package format + несколько runtime-классов** — сложнее host, но требования разделены явно.

## Решение

Выбрали **единый manifest и contributions с несколькими runtime-классами**: isolated control process, OpenAction compatibility process, widget host, trusted media host, remote agent и минимальный набор audited built-ins. Default granularity — один process на активную runtime group package, а не на каждую action instance. Native control protocol использует length-delimited Protocol Buffers over stdio. WASM остаётся возможным дополнительным runtime.

## Последствия

**Становится проще:** crash isolation, lazy activation, многоязычные plugins, корректный media path.  
**Становится сложнее:** SDK и conformance должны проверять несколько lifecycle.  
**Закрывает дверь на:** передачу raw media buffers через обычный JSON control channel и загрузку произвольной DLL в главный host.
