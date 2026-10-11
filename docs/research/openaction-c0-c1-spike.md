# OpenAction C0/C1 spike

Дата: 2026-10-12. Это ограниченный ранний spike, а не заявление о готовой
совместимости.

## Зафиксированная fixture

Использован открытый MIT plugin
[OpenAction Counter](https://github.com/OpenActionPlugins/counter) на revision
`1b8df9d2b729eded831f91550c5098c6a908c9b6`. Он присутствует в
[официальном каталоге OpenAction](https://github.com/OpenActionAPI/plugins/blob/main/catalogue.json)
как `me.amankhanna.oacounter`. В репозитории хранится только точный manifest и
provenance; executable, PI и source не vendored.

## Результат C0

`deckweave-openaction-host::CompatibilityScanner` без запуска кода:

- читает bounded `manifest.json`;
- индексирует обе actions и контроллеры `Keypad`/`Encoder`;
- выбирает platform entrypoint;
- отклоняет path traversal и дубли UUID;
- сохраняет явный `NeedsAttention`, если build artifact отсутствует;
- отмечает Property Inspector как C2, а не открывает его при индексации.

C0 подтверждён автоматическим тестом pinned fixture.

## Результат C1

C1 сознательно **не объявлен готовым**. Плагин использует опубликованный
OpenAction/Stream Deck WebSocket lifecycle. OpenDeck запускает такие плагины с
аргументами `-port`, `-pluginUUID`, `-registerEvent registerPlugin`, `-info`, а
затем переводит `willAppear`, key/dial events, settings и feedback через JSON
WebSocket. Это другой runtime protocol, не нативный DeckWeave Protobuf stdio.

Следовательно, production C1 требует отдельного lazy compatibility process:

1. loopback WebSocket listener с session token и bounded JSON;
2. exact `LegacyContextId ↔ BindingId` map;
3. `willAppear`/`willDisappear`, key down/up, dial rotate/press;
4. settings overlay с revision/package hash, без прямой мутации profile;
5. перевод `setTitle`/`setImage`/`setState` в scoped visual feedback;
6. запуск через общий process-tree guard, lease и circuit breaker;
7. conformance build pinned Counter artifact на Windows и Linux.

Property Inspector остаётся C2: для него нужны отдельный origin/storage
partition и broker. Наличие `pi.html` в manifest не повышает уровень C1.

## Решение

Нативный ABI не следует подгонять под legacy JSON. C0 scanner остаётся в
`runtimes/openaction-host`, а C1 WebSocket translator будет отдельным adapter
поверх общих supervisor/registry contracts. Это позволяет подключать экосистему,
не делая её ограничением для touch canvas, analog input и typed outcomes.

Источники протокольного направления:

- <https://github.com/OpenActionAPI/docs>
- <https://github.com/nekename/OpenDeck/blob/main/src-tauri/src/plugins/mod.rs>
- <https://github.com/OpenActionAPI/rust>
