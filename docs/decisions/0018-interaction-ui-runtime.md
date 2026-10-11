# ADR-0018: Host-owned Interaction UI runtime

**Статус:** принято  
**Дата:** 2026-10-12

## Контекст

Автоматизации иногда требуют короткого диалога с пользователем: поиск Zotero, выбор файла или приложения, ввод значения, подтверждение. Surface widgets решают вывод на устройство, а property editors — настройку plugin; ни один из них не должен открывать произвольные desktop windows или смешивать результат двух одновременных запусков.

## Решение

Ввести отдельный Interaction UI context. Automation открывает `InteractionSession` с typed request, timeout/cancel policy и invocation identity. Host владеет окном, focus, placement, Escape, lifecycle и accessibility. Provider поставляет schema-native модель данных либо sandboxed web view со scoped bridge. Terminal typed result возвращается только исходному invocation.

Plugin не получает произвольный Tauri/window API. Interaction view не является surface widget и не продолжает automation самостоятельно.

## Последствия

- один механизм покрывает command palette, Zotero search, picker, quick form и подтверждение;
- закрытие/timeout/cancel имеют одинаковую семантику;
- custom UI требует отдельной web isolation policy и запускается лениво;
- desktop shell получает новый вид host-owned окна, но domain providers не зависят от Tauri;
- конкурентные sessions изолируются по identity и result schema.

Модуль: [Interaction Runtime](../architecture/modules/interaction-runtime.md).
