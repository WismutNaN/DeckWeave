# Совместимость с OpenAction и Elgato Stream Deck

Совместимость является ранней функцией DeckWeave, потому что готовая экосистема действий важнее собственного большого каталога на старте. При этом legacy protocol остаётся bridge на границе и не ограничивает внутреннюю capability model. Первым реализуется OpenAction/опубликованное подмножество Elgato; Bitfocus Companion рассматривается отдельным более поздним bridge.

## Цели

- устанавливать открытые OpenAction-плагины без переписывания;
- запускать совместимые Stream Deck-плагины по опубликованному WebSocket protocol;
- показывать их actions в общем каталоге DeckWeave;
- поддерживать property inspectors в изолированном web context;
- переводить feedback plugins в универсальные состояния и surface nodes;
- выдавать пользователю точный compatibility report вместо бинарного «работает/не работает»;
- позволять позже переписать конкретный plugin нативно, сохранив пользовательские bindings через migration.

## Не цели

- эмулировать все внутренние детали фирменного Stream Deck software;
- гарантировать работу закрытых plugins с undocumented native dependencies;
- представлять аналоговую ось или произвольную touch surface как legacy key, если это теряет семантику;
- переносить чужой код без provenance, проверки лицензии и отделения ecosystem-specific логики;
- разрешать property inspector прямой доступ к privileged DeckWeave API.
- автоматически переписывать исходники стороннего plugin под DeckWeave SDK.

## Compatibility architecture

```text
DeckWeave Capability/Event Model
             │
             v
┌───────────────────────────────────────┐
│ Compatibility Host                   │
│                                       │
│ Context Map        Settings Store     │
│ Event Translator   Image/Title Bridge │
│ Manifest Adapter   PI Static Server   │
└───────────────┬───────────────────────┘
                │ legacy WebSocket protocol
                v
      OpenAction / Stream Deck Plugin
                │
                v
       Sandboxed Property Inspector
```

Compatibility host запускается только при наличии активных legacy actions или открытого property inspector. Он является отдельным runtime package/process tree и не входит в hot path native plugins.

## Уровни совместимости

| Уровень | Поддержка | Критерий |
|---|---|---|
| C0 — Indexed | Manifest распознан, actions видны | Package устанавливается без запуска |
| C1 — Basic Action | `willAppear`, key down/up, settings, basic feedback | Reference plugin проходит lifecycle fixture |
| C2 — Property Inspector | HTML PI, scoped communication, settings round-trip | PI не имеет privileged bypass и сохраняет настройки |
| C3 — Dial/Touch Feedback | dial events, title/value/image layouts | Encoder reference plugin проходит visual test |
| C4 — Profiles & Multi-actions | profiles, state switching, multi-action semantics | Fixture восстанавливается после restart |
| C5 — Extended OpenAction | Документированные OpenAction extensions | Каждое extension имеет отдельный conformance case |

Совместимость указывается для конкретных package version, OS и DeckWeave version. Статус одного плагина не переносится на весь marketplace.

## Перевод контекстов

Legacy context обычно связывает device, profile, controller и position. DeckWeave создаёт временный `LegacyContextId`, сопоставленный с устойчивым `BindingId` рабочего пространства.

| Legacy | DeckWeave |
|---|---|
| `Keypad` action instance | Binding к `button.digital` и одному или нескольким visual targets |
| `Encoder` action instance | Binding к `encoder.relative`/`encoder.press` и optional surface region |
| `keyDown`/`keyUp` | Edge events без coalescing |
| `dialRotate` | `EventOut` с signed delta, без latest-value coalescing |
| `setImage` | Scene asset/update для выбранного visual target |
| `setTitle` | Text scene node, если target поддерживает текстовую композицию |
| `setState` | Legacy action state внутри binding |
| global/settings | Namespaced settings documents |
| profile switch | Запрос compatibility profile, не произвольная мутация workspace |

Существующий plugin видит ожидаемую legacy topology. Ядро при этом не притворяется, что весь N4 Pro состоит из четырёх encoder displays: непрерывный canvas может предоставить compatibility regions только выбранному layout.

## Результаты legacy actions

Доставка keyDown/dialRotate сама по себе не подтверждает завершение внешнего действия plugin. Bridge объявляет `legacy.dispatch` с результатом доставки и явным completion predicate; не обещает typed transcript/mail result или cancel внешнего эффекта, которых исходный protocol не предоставляет. UI помечает такую границу в sequence. Если нужен semantic result, необходим подтверждённый extension, независимый state/query adapter либо native replacement. `setState`/`setTitle` не считаются ответом нужному invocation по случайному совпадению времени.

Нативный [automation contract](automation.md) допускает result schema unit, но требует честно указать, что именно считается completed. Существующий plugin остаётся полезным как action/feedback, не притворяясь надёжным job provider.

## Manifest и установка

Установка выполняется через staging:

1. проверить размер, структуру архива и отсутствие path traversal;
2. прочитать manifest и schema без запуска кода;
3. выбрать entry point по OS/architecture;
4. вывести requested legacy permissions и зависимости;
5. индексировать actions и assets;
6. сохранить package неизменяемой версией;
7. запускать только при активации action.

`CodePath`, Node runtime, native addons и внешние executables попадают в compatibility report. Если dependency отсутствует, package остаётся установленным со статусом `NeedsAttention`.

## Property Inspector

- Отдельный origin и browser storage partition на package/session; случайный session path не считается origin isolation.
- CSP и запрет навигации на `file://`.
- Scoped token связан с package/action/context и имеет TTL.
- Сообщения проходят через legacy PI bridge, а не Tauri privileged commands.
- Сетевые запросы PI регулируются permission policy package.
- Закрытие PI не останавливает action runtime.
- Ошибка PI не помечает device disconnected.

Legacy `setSettings`/`setGlobalSettings` из active plugin создают versioned runtime settings overlay, scoped package/binding, и возвращаются plugin через legacy protocol. Они не мутируют immutable revision или открытый draft. Overlay хранится отдельно, показывается UI как external settings diff и переносится в draft явной командой с expected version; при rollback/rebind overlay старого контекста не применяется автоматически. PI редактирует draft settings через broker, live preview изолирован. Persistence/restart этих settings входит в C1/C2 fixtures; если конкретный plugin требует иной семантики, report ограничивает compatibility claim.

Overlay привязан к workspace revision, package hash, settings schema и binding либо package-global scope. Обычный restart той же revision восстанавливает его; переход revision требует явного merge/reset, а не переноса по совпадению context ID. Mutations проходят host application broker и его persistence/limits, не direct file write. Scheme полей, содержащих secrets, требует credential mapping; generic raw settings не становятся исключением из redaction/export policy.

Browser origin/storage/bridge isolation проверяется по [Безопасность](security.md#web-property-editors-и-widgets). Нельзя обходить её ради legacy inspector.

## Conformance suite

Минимальные fixtures:

1. action lifecycle: install → appear → key down/up → disappear → restart;
2. settings: global/context → modify → reload → migrate;
3. feedback: image/title/state/alert;
4. dial: rotate/press/touch layout;
5. PI: registration, sendToPlugin, sendToPropertyInspector;
6. plugin crash и повторная регистрация;
7. invalid manifest/path/assets;
8. slow consumer и oversized message;
9. unsupported event с диагностикой;
10. несколько devices и одинаковых action instances.

Для первой публичной матрицы выбираются только открытые plugins из [OpenAction catalogue](https://github.com/OpenActionAPI/plugins), чтобы CI мог легально и воспроизводимо собирать fixtures. Каждая fixture фиксирует upstream revision, license и способ получения.

## Миграция в нативный plugin

Если legacy plugin переписывается под DeckWeave:

- новый package объявляет `migrates_from` с legacy UUID/action UUID;
- migration переводит settings, но не запускает действия;
- пользователь видит diff и подтверждает замену;
- старый package сохраняется до успешного запуска новой binding revision;
- rollback возвращает предыдущий compatibility binding.

Таким образом, существующая экосистема помогает проекту стартовать, но не блокирует более качественные нативные реализации.

## Будущий Bitfocus Companion bridge

Companion не является частью первой compatibility вертикали. Его bridge проектируется отдельно, потому что Companion modules, surface integrations и внутренний tRPC имеют другие lifecycle и риски совместимости.

Предпочтительный порядок:

1. использовать документированные HTTP/WebSocket interfaces Companion, где они достаточны;
2. предоставить DeckWeave virtual surface/device endpoint, если это можно сделать через устойчивый контракт;
3. запускать module compatibility в отдельном process и публиковать version matrix;
4. использовать внутренний tRPC только в явно version-pinned adapter с contract fixtures;
5. при несовместимости отключать только Companion bridge, не native packages.

Bridge может экспортировать actions, variables и feedback как DeckWeave ports. Он не получает права менять workspace files и не становится зависимостью N4 Pro plugin.

Автоматическое преобразование Companion module в native DeckWeave package не планируется. Автор или сообщество может создать нативную замену и объявить migration settings.

## Лицензирование compatibility code

Application/core DeckWeave планируется под `GPL-3.0-or-later`, поэтому совместимое GPL-заимствование теоретически возможно. Однако предпочтение остаётся за опубликованными protocol contracts и собственными conformance tests: это уменьшает связанность с чужой внутренней архитектурой. Любое прямое заимствование проходит отдельную проверку origin, revision, license, notices и modifications.

## Известные ограничения

- Некоторые плагины проверяют имя/версию фирменного приложения.
- Native addons могут существовать только для одной OS/architecture.
- Закрытые login flows или marketplace services могут быть недоступны.
- Plugins могут рассчитывать на конкретные Elgato layouts и размеры.
- Undocumented события и внутренние API не входят в compatibility contract.
- Юридическая возможность установки стороннего package не следует из технической совместимости.
