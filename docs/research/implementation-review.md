# Исследование реализаций и пересмотр архитектуры

Дата: 2026-10-12. Проверены исходные файлы на зафиксированных commits, перечисленных в [REFERENCES.md](REFERENCES.md), и официальная документация. Чужие приложения и hardware не запускались; performance, совместимость конкретных release binaries и физические режимы N4 Pro здесь не подтверждены. Код из референсов в DeckWeave не скопирован. Наблюдения ниже относятся к указанным файлам, а предлагаемые контракты — решения DeckWeave.

## Что изменилось после сравнения

| Пробел прежней спецификации | Почему это мешало бы расширению | Решение |
|---|---|---|
| Outcome есть, но нет полноценной модели результата/job | «Запустить Handy» не равно «получить текст для следующего шага» | Typed input/result, run scope, terminal completion, artifact broker |
| Notifications описаны как state | Latest snapshot теряет историю и изменения; «все» не помещаются в очередь | Arrival event + summary state + paginated query |
| Light units перечислены без topology | Нельзя воспроизводимо построить волну по клавиатуре | Отдельные geometry, key→LED mapping и light scene |
| Feedback routes могут независимо писать свет | Конфликт анимации, слоя, preview и hardware preset | Один output owner и явная композиция |
| Onboard programs недостаточно отделены от live commands | Обычный apply мог бы писать EEPROM или оставлять автономный macro | Отдельные host/volatile/persistent contracts |
| Dynamic capability означает только available/unavailable | Sidechain, изменившаяся topology и disappearing port требуют иной revalidation | Atomic descriptor revision, templates/requestable instances |
| Media обещает bypass без точной crash boundary | Падение всего media-host уничтожает и находящийся в нём обход | Backend-specific buffer/lifecycle/bypass gate |
| N4 coordinate event смешан с полноценным touch lifecycle | Не доказаны release/contact ID; hold/drag могут залипнуть | `pointer.sample` отдельно от `touch.points`, hardware evidence gate |
| Разные документы повторяют правила без владельца | Любое расширение требует искать противоречия по всему дереву | Тематические contracts, короткие module boundaries, архив исторических материалов |

## Host плагинов и совместимость

**Наблюдение.** OpenDeck запускает Native/Node/Wine children и регистрирует plugins по WebSocket; отдельный spawner учитывает lifetime родительского thread. Context связывает actions с profiles/controllers/position. `set_settings` меняет runtime settings под lock и сообщает изменение, global settings записываются отдельным JSON. Это практический источник lifecycle/compatibility fixtures, но такие mutable settings не заменяют DeckWeave immutable revisions. Источники: [plugins/mod.rs](https://github.com/nekename/OpenDeck/blob/66db15d9d5d41597772a4f209cd6a93b498fc7df/src-tauri/src/plugins/mod.rs), [shared.rs](https://github.com/nekename/OpenDeck/blob/66db15d9d5d41597772a4f209cd6a93b498fc7df/src-tauri/src/shared.rs), [settings.rs](https://github.com/nekename/OpenDeck/blob/66db15d9d5d41597772a4f209cd6a93b498fc7df/src-tauri/src/events/inbound/settings.rs).

OpenAction документирует аргументы запуска, registration и разные plugin/property-inspector подключения; API совместимости не сводится к forwarding одинаковых JSON. Каталог — источник pinned test packages, не доказательство поддержки всех plugins. Источники: [registration](https://github.com/OpenActionAPI/docs/blob/ffe2564bd42f54b19443cf1dac821adf6a85a848/src/api/registration.md), [manifest](https://github.com/OpenActionAPI/docs/blob/ffe2564bd42f54b19443cf1dac821adf6a85a848/src/manifest.md), [catalogue](https://github.com/OpenActionAPI/plugins/blob/32d7ec36492bbdadc99d92df601fbab30dbb67c8/catalogue.json). Ожидания Elgato дополнительно сверяются с [официальным WebSocket contract](https://docs.elgato.com/streamdeck/sdk/references/websocket/plugin/) и [settings guide](https://docs.elgato.com/streamdeck/sdk/guides/settings/).

**Решение DeckWeave.** Сохраняем отдельный compatibility process, versioned context mapping и overlay settings с явным переносом в draft. Копирование OpenDeck целиком ускорило бы clone Stream Deck, но сделало бы display cell основной сущностью для клавиатуры, microphone и notification feed. Native contract остаётся независимым; bridge может честно вернуть unsupported.

## Ввод, слои и двусторонние устройства

**Наблюдение.** InputPlumber разделяет input и output traits source/target. Физический источник ввода также принимает feedback, а virtual target может возвращать output events; composite device объединяет capabilities. Source driver имеет channel buffer size. Конкретные transports сведены в собственные enum и Linux infrastructure. Источники: [source](https://github.com/ShadowBlip/InputPlumber/blob/d951ef9df713a3ad3a88645a8c07b79ea5592274/src/input/source/mod.rs), [target](https://github.com/ShadowBlip/InputPlumber/blob/d951ef9df713a3ad3a88645a8c07b79ea5592274/src/input/target/mod.rs), [composite device](https://github.com/ShadowBlip/InputPlumber/blob/d951ef9df713a3ad3a88645a8c07b79ea5592274/src/input/composite_device/mod.rs).

**Вывод.** Направление принадлежит порту, не классу «входное устройство». Берём двустороннюю композицию и bounded communication, но native DeckWeave plugin не должен требовать изменения transport enum ядра или Linux daemon.

Kanata/keyberon принимает events и ticks, хранит waiting/tap-hold/layer state. Его core model опирается на keyboard matrix; это хороший кандидат для keyboard-specific adapter или timing fixtures, но неудобная универсальная модель любых устройств. [layout.rs](https://github.com/jtroo/kanata/blob/c391826f6bbc8d5dc1645bbedc5596e22ae5982a/keyberon/src/layout.rs), [action.rs](https://github.com/jtroo/kanata/blob/c391826f6bbc8d5dc1645bbedc5596e22ae5982a/keyberon/src/action.rs). Для DeckWeave закреплены owner-based release, layer tokens и fake clock. Внешние async conditions не вызываются внутри синхронного suppression deadline.

Wooting объявляет ABI version, bounded buffers, callback/device-info lifetimes и unload. C API также использует специальные float error values, которые не стоит переносить в общий wire contract. [plugin.h](https://github.com/WootingKb/wooting-analog-sdk/blob/be35a629bd79967012da4dcb731fa9e5a6d3c132/includes/plugin.h), [описание plugins](https://github.com/WootingKb/wooting-analog-sdk/blob/be35a629bd79967012da4dcb731fa9e5a6d3c132/docs/PLUGINS.md). Берём явное владение и проверку версий; foreign ABI помещается внутрь isolated provider, а range/error передаются типизированно. Ход клавиши не объявляется силой без физического измерения.

## Handy и результаты действий

**Проверено в source.** В Handy есть отдельные CLI modes: interactive toggle и headless transcription файла; headless имеет JSON output. [cli.rs](https://github.com/cjpais/Handy/blob/48a3d8e9ee456c1c6a3b7223779271c457890b4d/src-tauri/src/cli.rs). Реализация проверяет 16 kHz mono 16-bit PCM WAV, использует установленную модель, возвращает JSON с `text`, отправляет логи в stderr и обходит single-instance forwarding для headless. [lib.rs](https://github.com/cjpais/Handy/blob/48a3d8e9ee456c1c6a3b7223779271c457890b4d/src-tauri/src/lib.rs). UI pipeline отдельно выполняет post-processing/output; его поведение нельзя автоматически приписывать headless path. [actions.rs](https://github.com/cjpais/Handy/blob/48a3d8e9ee456c1c6a3b7223779271c457890b4d/src-tauri/src/actions.rs).

**Решение.** Первым кандидатом становится adapter конечного файла с результатом, exit status и cancellation. Это не обещание API у всех Handy releases. Provider проверяет pinned executable/flags/model, а capture и browser integration реализуются отдельно. Toggle не превращается в correlated job без отдельного способа узнать его результат. Сценарий подробно задан в [product/scenarios.md](../product/scenarios.md#кнопка--handy--обработка-текста--вкладки-браузера).

**Сравнение workflow engines.** Node-RED разделяет получение сообщения, `send` и completion callback, учитывает message fan-out/cloning. Это подтверждает необходимость отдельной семантики output/completion и владения payload. [Node.js](https://github.com/node-red/node-red/blob/935b8d3e12ec8be064f3a72ac6547131f778a7b1/packages/node_modules/@node-red/runtime/lib/nodes/Node.js). Но произвольный mutable message object и JS runtime не становятся внутренним API Rust scheduler; DeckWeave связывает typed results по `RunId` и branch scope.

В Companion `ActionRunner` ожидает результат action, может записать его в переменную, умеет sequential/parallel execution и проверяет abort перед сохранением. При этом рассматриваемая sequence ловит ошибки и продолжает обработку. [ActionRunner.ts](https://github.com/bitfocus/companion/blob/404070f390b8b18530c46f9040211a5004947fd9/companion/lib/Controls/ActionRunner.ts). Для DeckWeave stop/continue/fallback — явные policies, а shared variable не является default способом передачи текста.

В `FramedMessageChannel` Companion pacing ждёт stream drain, но локальный `writeQueue.push` в исследованном классе не имеет собственного hard limit. Это не оценка лимитов всей программы; вывод для нас — backpressure транспортного socket недостаточен без квоты application queue. [FramedMessageChannel.ts](https://github.com/bitfocus/companion/blob/404070f390b8b18530c46f9040211a5004947fd9/companion/lib/Instance/Common/FramedMessageChannel.ts). Native stdio protocol сохраняет hard limits и priority budget для cancel/heartbeat.

## Геометрия и подсветка

QMK `led_config_t` раздельно хранит input-matrix→LED mapping, points и flags, включая отсутствие LED; reactive state хранит координаты/время последних нажатий. [rgb_matrix_types.h](https://github.com/qmk/qmk_firmware/blob/7a1bbf37c5c07da4ea0a162bb139083a46ef40a1/quantum/rgb_matrix/rgb_matrix_types.h). OpenRGB различает LEDs, zones, segments, matrix maps и modes, а device operations скрывает за controller boundary. [RGBController.h](https://github.com/CalcProgrammer1/OpenRGB/blob/4d17bac5b8f34746847e25bd42189cde8014f0cc/RGBController/RGBController.h).

**Решение.** Заимствуем разделение моделей, добавляем stable IDs, coordinate frames, many-to-many mapping и descriptor revision. Host renderer рассчитывает волну и per-key envelope, provider отправляет bulk frame. QMK firmware и OpenRGB controller не становятся обязательными зависимостями core. Для широкого RGB hardware покрытия предпочтителен отдельный OpenRGB adapter после проверки версии network protocol и resource ownership. Hardware presets и frames конкурируют за один output owner; persistent memory имеет иной workflow.

## N4 Pro: полезный код и ограничения доказательств

В C++ N4 Pro descriptor есть основной размер 800×480, отдельные second-screen параметры, десять keys, encoder codes и четыре LEDs. Это требует проверки mapping физических областей, а не подстановки основного размера в touch transform. `registerTouchBarCallback` разбирает bytes 10–13, но использует внутренний lambda вместо переданного callback и не проверяет минимальную длину до индексации. [streamdockN4Pro.cpp](https://github.com/MiraboxSpace/StreamDock-Device-SDK/blob/b63be85c687d3d31e3e2b7e32dddc9a50efa5ed9/CPP-SDK/src/HotspotDevice/StreamDockN4Pro/streamdockN4Pro.cpp).

Python-реализация проверяет длину, создаёт `TOUCH_POINT` и вызывает callback; выбранный decoder не устанавливает contact ID/down/up. [StreamDockN4Pro.py](https://github.com/MiraboxSpace/StreamDock-Device-SDK/blob/b63be85c687d3d31e3e2b7e32dddc9a50efa5ed9/Python-SDK/src/StreamDock/Devices/StreamDockN4Pro.py). Значит, координатный sample подтверждён исходниками, полноценный multitouch/hold lifecycle — нет. C++ общий device layer имеет background/frame методы и ограничения feature support; RGB controller предоставляет общий и per-unit color путь. [streamdock.cpp](https://github.com/MiraboxSpace/StreamDock-Device-SDK/blob/b63be85c687d3d31e3e2b7e32dddc9a50efa5ed9/CPP-SDK/src/DeviceInfo/streamdock.cpp), [rgbcontroller.cpp](https://github.com/MiraboxSpace/StreamDock-Device-SDK/blob/b63be85c687d3d31e3e2b7e32dddc9a50efa5ed9/CPP-SDK/src/DeviceInfo/Feature/RGBController/rgbcontroller.cpp).

`opendeck-akp05` показывает практические keepalive/cancel, image conversion и отдельные encoder images; callback `set image` использует `unwrap` для data URL, поскольку ожидает контролируемый источник. Такой trust assumption не переносится на вход native DeckWeave protocol. [device.rs](https://github.com/ambiso/opendeck-akp05/blob/8f7742b7ba3125ca2a9df106b18fbf227e37a0d5/src/device.rs), [watcher.rs](https://github.com/ambiso/opendeck-akp05/blob/8f7742b7ba3125ca2a9df106b18fbf227e37a0d5/src/watcher.rs).

**Решение.** Native provider может переиспользовать проверенный protocol parser/encoder или transport library после fixture/license gate. Нужно подтвердить model/firmware, экранные targets, coordinates/rotation, touch phase, volatile/persistent commands, per-ring output и vibration semantics. Флаг enable встроенной вибрации не доказывает standalone pattern playback. Наличие C++ метода не является hardware conformance. До подтверждения contact phases provider публикует `pointer.sample` с quality, не придумывает release/pressure. N4 full-screen UX остаётся целью; hardware gate может потребовать пересмотра product claim, а не выдуманного descriptor.

## Медиа и удалённые узлы

GStreamer документирует always/sometimes/request pads, различие возможных и negotiated caps и явные converters. [Pads](https://gstreamer.freedesktop.org/documentation/application-development/basics/pads.html), [caps negotiation](https://gstreamer.freedesktop.org/documentation/plugin-development/advanced/negotiation.html). PipeWire отделяет media graph от session-management objects и соединений. [Objects design](https://docs.pipewire.org/page_objects_design.html). Это аргументы за разделение template/instance, allocation и routing policy; они не выбирают готовый Windows backend за нас.

CLAP задаёт init/activate/start/process/stop/deactivate/destroy, доступность optional extensions и lifetime process pointers; параметры и audio ports вынесены в extensions. [plugin.h](https://github.com/free-audio/clap/blob/a47f6badb49d948fd009998f28309cdab78979c9/include/clap/plugin.h), [params.h](https://github.com/free-audio/clap/blob/a47f6badb49d948fd009998f28309cdab78979c9/include/clap/ext/params.h), [audio-ports.h](https://github.com/free-audio/clap/blob/a47f6badb49d948fd009998f28309cdab78979c9/include/clap/ext/audio-ports.h). DeckWeave должен сохранять эти thread contracts внутри media bridge, а не оборачивать `process` в ordinary async command.

Android MediaProjection имеет пользовательское разрешение и lifecycle сессии, который remote agent обязан уважать. [Официальная документация](https://developer.android.com/media/grow/media-projection). Для MCP исследован [официальный Rust SDK](https://github.com/modelcontextprotocol/rust-sdk) как кандидат внешнего application API adapter. Эти два источника проверены на уровне документации/API, не полного исходного кода или совместимости устройств. Production remote/media остаются отдельными программами.

## Альтернативы и выбор

| Вариант | Выигрыш | Ограничение для DeckWeave | Решение |
|---|---|---|---|
| Форк OpenDeck как всё приложение | Готовая совместимость и UI panels | Device/profile/action context определяет core, нет наших contracts для jobs/media/lighting | Использовать bridge/fixtures, не брать core целиком |
| Встроить Node-RED как главный engine | Большая automation ecosystem | Второй runtime/state store, иная семантика timing/typing/permissions | Свой малый scheduler; отдельный bridge возможен по спросу |
| InputPlumber/Kanata как всё ядро | Проверенные input paths | OS/matrix domain, нет общего результата workflow и визуальных surfaces | Adapters и bounded reuse отдельных частей |
| Один универсальный RPC/ABI для всего | Меньше protocol families | Не различает result, frame, realtime buffer и lifetime | Общие IDs/grants/lifecycle плюс специализированные контракты |
| Свои transport/DSP/ASR для каждого устройства | Полный контроль | Большой объём поддержки и повторение уже решённых задач | Тонкие adapters к существующим libraries/processes, собственная orchestration |

Собственный код необходим там, где решения DeckWeave различаются: revisions/apply, capability binding, scoped results, resource arbitration и редактор. Его объём ограничивается этими границами. Ни один reference не доказывает «идеальность» всей конструкции; ранние heterogeneous fixtures и benchmarks позволяют опровергнуть неверное решение до стабилизации SDK.

## Кандидаты переиспользования

Лицензии ниже — наблюдения о выбранных metadata/files, не заключение обо всём dependency tree. Перед переносом проверяются exact files, headers, transitive dependencies, notices и способ распространения; copyleft code не помещается в permissive SDK. [ADR о лицензировании](../decisions/0007-licensing.md).

| Источник | Наблюдаемая лицензия | Кандидат и ограничение |
|---|---|---|
| Mirabox SDK | [MIT, корневой LICENSE](https://github.com/MiraboxSpace/StreamDock-Device-SDK/blob/b63be85c687d3d31e3e2b7e32dddc9a50efa5ed9/LICENSE) | Parser/encoder/transport в N4 provider; исправить callback/bounds, подтвердить firmware fixtures |
| OpenDeck | [GPL-3.0-or-later в Cargo metadata](https://github.com/nekename/OpenDeck/blob/66db15d9d5d41597772a4f209cd6a93b498fc7df/src-tauri/Cargo.toml) | Compatibility host код/fixtures в GPL части; не импортировать profile model в SDK |
| opendeck-akp05 | [`GPL-3.0` в Cargo metadata](https://github.com/ambiso/opendeck-akp05/blob/8f7742b7ba3125ca2a9df106b18fbf227e37a0d5/Cargo.toml) | Keepalive/output mapping в GPL provider; проверить точные headers и mirajazz dependency отдельно |
| InputPlumber | [GPL-3.0-or-later в Cargo metadata](https://github.com/ShadowBlip/InputPlumber/blob/d951ef9df713a3ad3a88645a8c07b79ea5592274/Cargo.toml) | Linux adapter/composition fixtures; не тащить daemon во все builds |
| Kanata / kanata-keyberon | [LGPL-3.0-only](https://github.com/jtroo/kanata/blob/c391826f6bbc8d5dc1645bbedc5596e22ae5982a/Cargo.toml) / [MIT keyberon](https://github.com/jtroo/kanata/blob/c391826f6bbc8d5dc1645bbedc5596e22ae5982a/keyberon/Cargo.toml) | Кандидат keyberon library внутри keyboard adapter; выбранные файлы/зависимости проверять отдельно от лицензии всего Kanata |
| Wooting | [MPL-2.0 в Cargo metadata](https://github.com/WootingKb/wooting-analog-sdk/blob/be35a629bd79967012da4dcb731fa9e5a6d3c132/Cargo.toml) | Использование SDK за process boundary и соблюдение файловых notices при изменении |
| CLAP | MIT | Headers/validator/client bridge, native DSP внутри media-host |
| OpenRGB / QMK | GPL family; OpenRGB root GPLv2, QMK выбранный header GPL-2.0-or-later | Предпочесть protocol adapter; не делать blanket claim о лицензии всех drivers/firmware files |
| Node-RED | Apache-2.0 | Готовый внешний runtime/adapter либо отдельные algorithms после provenance; не обязательно встраивать весь engine |
| Companion | MIT-раздел LICENSE и отдельный contributor agreement | API/bridge patterns; licenses modules проверяются независимо, внутренний transport version-pinned |
| Handy | MIT | CLI adapter сначала; extraction ASR internals лишь при доказанной необходимости с аудитом model/backend licenses |
| OpenAction docs | MIT | Fixtures протокола/схем; catalogue не заменяет licenses каждого plugin |

Для реального заимствования PR фиксирует repository URL, commit, file paths, исходную лицензию, notices, сделанные изменения, update strategy и tests. Сейчас выбран только способ интеграции; vendoring, добавление dependencies и принятие лицензии чужого package не выполнялись.

## Проверки перед стабилизацией

1. Фаза 2: независимые descriptors observe keyboard, analog keyboard, irregular RGB keyboard, screen/touch, async service и audio template проходят общую schema validation; Фаза 5 проверяет compiler без device-name special cases.
2. Фазы 3/5: динамическое удаление порта, oversize output, corrupted framing, lost result и cancel чужой epoch.
3. Фазы 9/11: light topology, waveform/cooldown, layer ownership, два concurrent typed runs и paginated feed.
4. Фаза 8: реальные N4 captures вместо предположений о screen/touch/haptic. Любой gap остаётся открытым hardware gate.
5. После v1: Handy/capture/browser end-to-end; mail/Telegram provider contract; media backend benchmark. Неподтверждённая интеграция не помечается готовой по успеху fake fixture.
