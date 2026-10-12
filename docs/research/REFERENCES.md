# Референсы DeckWeave

Реестр первичных источников. Исследование исходников выполнено 2026-10-12; commits зафиксированы ниже, чтобы выводы не зависели от изменяющейся ветки. Разобранные файлы, обнаруженные ограничения, альтернативы и кандидаты переиспользования находятся в [анализе реализаций](implementation-review.md). Это не перечень обязательных dependencies.

## Зафиксированные версии

| Проект | Commit | Проверенная область |
|---|---|---|
| [nekename/OpenDeck](https://github.com/nekename/OpenDeck) | [66db15d9d5d41597772a4f209cd6a93b498fc7df](https://github.com/nekename/OpenDeck/tree/66db15d9d5d41597772a4f209cd6a93b498fc7df) | Host/lifecycle, contexts и settings |
| [ShadowBlip/InputPlumber](https://github.com/ShadowBlip/InputPlumber) | [d951ef9df713a3ad3a88645a8c07b79ea5592274](https://github.com/ShadowBlip/InputPlumber/tree/d951ef9df713a3ad3a88645a8c07b79ea5592274) | Source/target/composite traits и буферы |
| [jtroo/kanata](https://github.com/jtroo/kanata) | [c391826f6bbc8d5dc1645bbedc5596e22ae5982a](https://github.com/jtroo/kanata/tree/c391826f6bbc8d5dc1645bbedc5596e22ae5982a) | Keyberon event/tick/layer state и лицензии subpackages |
| [WootingKb/wooting-analog-sdk](https://github.com/WootingKb/wooting-analog-sdk) | [be35a629bd79967012da4dcb731fa9e5a6d3c132](https://github.com/WootingKb/wooting-analog-sdk/tree/be35a629bd79967012da4dcb731fa9e5a6d3c132) | C ABI, buffer/callback lifetime, plugins |
| [MiraboxSpace/StreamDock-Device-SDK](https://github.com/MiraboxSpace/StreamDock-Device-SDK) | [b63be85c687d3d31e3e2b7e32dddc9a50efa5ed9](https://github.com/MiraboxSpace/StreamDock-Device-SDK/tree/b63be85c687d3d31e3e2b7e32dddc9a50efa5ed9) | N4 C++/Python, touch decoder, background frames и RGB |
| [ambiso/opendeck-akp05](https://github.com/ambiso/opendeck-akp05) | [8f7742b7ba3125ca2a9df106b18fbf227e37a0d5](https://github.com/ambiso/opendeck-akp05/tree/8f7742b7ba3125ca2a9df106b18fbf227e37a0d5) | Device output, keepalive, cancellation и watcher |
| [free-audio/clap](https://github.com/free-audio/clap) | [a47f6badb49d948fd009998f28309cdab78979c9](https://github.com/free-audio/clap/tree/a47f6badb49d948fd009998f28309cdab78979c9) | Lifecycle, params, audio ports и thread contracts |
| [OpenActionAPI/docs](https://github.com/OpenActionAPI/docs) | [ffe2564bd42f54b19443cf1dac821adf6a85a848](https://github.com/OpenActionAPI/docs/tree/ffe2564bd42f54b19443cf1dac821adf6a85a848) | Manifest, registration, device/settings API |
| [OpenActionAPI/plugins](https://github.com/OpenActionAPI/plugins) | [32d7ec36492bbdadc99d92df601fbab30dbb67c8](https://github.com/OpenActionAPI/plugins/tree/32d7ec36492bbdadc99d92df601fbab30dbb67c8) | Каталог кандидатов fixtures; не аудит кода всех packages |
| [bitfocus/companion](https://github.com/bitfocus/companion) | [404070f390b8b18530c46f9040211a5004947fd9](https://github.com/bitfocus/companion/tree/404070f390b8b18530c46f9040211a5004947fd9) | Action results, abort и framed IPC |
| [CalcProgrammer1/OpenRGB](https://github.com/CalcProgrammer1/OpenRGB) | [4d17bac5b8f34746847e25bd42189cde8014f0cc](https://github.com/CalcProgrammer1/OpenRGB/tree/4d17bac5b8f34746847e25bd42189cde8014f0cc) | RGBController LEDs/zones/matrix/modes |
| [node-red/node-red](https://github.com/node-red/node-red) | [935b8d3e12ec8be064f3a72ac6547131f778a7b1](https://github.com/node-red/node-red/tree/935b8d3e12ec8be064f3a72ac6547131f778a7b1) | Node receive/send/done и message fan-out |
| [cjpais/Handy](https://github.com/cjpais/Handy) | [48a3d8e9ee456c1c6a3b7223779271c457890b4d](https://github.com/cjpais/Handy/tree/48a3d8e9ee456c1c6a3b7223779271c457890b4d) | CLI, headless execution и interactive output pipeline |
| [qmk/qmk_firmware](https://github.com/qmk/qmk_firmware) | [7a1bbf37c5c07da4ea0a162bb139083a46ef40a1](https://github.com/qmk/qmk_firmware/tree/7a1bbf37c5c07da4ea0a162bb139083a46ef40a1) | RGB matrix types, positions и LED flags |

Исходники получены через read-only git fetch/show; build, install scripts и сторонние executable не запускались. По OpenAction catalogue проверялась metadata, а не реализация перечисленных plugins. По остальным строкам исследовались выбранные source files, не весь репозиторий. Наличие функции в source HEAD не означает её наличие в стабильном binary release.

## Официальная документация

Дополнительные источники для каталога plugins — Windows window/input APIs, Singularity, Sprut/MQTT, Zotero/BBT, unread feeds и Samsung/Health Connect — сведены в [проверке реализуемости](plugin-feasibility.md#external-evidence). Там же зафиксированы HEAD, dirty-tree status и hashes локального [Zotst](plugin-feasibility.md#zotst); текущие изменения этого референса не приписываются upstream commit. Заимствования кода не выполнены.

| Источник | Проверенная граница |
|---|---|
| [Elgato WebSocket](https://docs.elgato.com/streamdeck/sdk/references/websocket/plugin/), [settings](https://docs.elgato.com/streamdeck/sdk/guides/settings/) | Launch/registration/context и settings compatibility |
| [GStreamer pads](https://gstreamer.freedesktop.org/documentation/application-development/basics/pads.html), [negotiation](https://gstreamer.freedesktop.org/documentation/plugin-development/advanced/negotiation.html) | Dynamic/request pads, exact caps и converters |
| [PipeWire objects](https://docs.pipewire.org/page_objects_design.html) | Media graph и session-management boundary |
| [Android MediaProjection](https://developer.android.com/media/grow/media-projection) | Consent, session lifecycle и отзыв захвата |
| [MCP Rust SDK](https://github.com/modelcontextprotocol/rust-sdk) | Optional внешний adapter к application API |

Документация проверена 2026-10-12 без pin полного upstream website. Перед implementation обновляется конкретная версия SDK/API; эти источники не использовались как доказательство Windows driver support или hardware timing.

## Как использовать референс

1. Найти конкретную проблему и прочитать соответствующий source path из [анализа](implementation-review.md).
2. Сравнить runtime, ownership, failure и license boundaries с DeckWeave; не переносить чужую domain model автоматически.
3. Выбрать adapter, dependency, vendoring отдельных файлов либо собственную небольшую реализацию; зафиксировать tradeoff в ADR, если меняется общий контракт.
4. Для заимствования записать origin/commit/files/license/notices/modifications и update strategy. Проверить зависимости и headers выбранных файлов отдельно.
5. Подтвердить перенос fixture или benchmark. Заявление README и наличие метода не заменяют conformance.

Лицензионные наблюдения и конкретные reuse-кандидаты: [матрица](implementation-review.md#кандидаты-переиспользования). Стратегия приложения и SDK: [ADR-0007](../decisions/0007-licensing.md).

## История

[Первоначальное исследование стека](../archive/initial-stack.md) сохранено как исторический материал. Его ранние предложения могут расходиться с принятыми решениями; нормативны текущие product/contracts и ADR. [Первый аудит](../archive/2026-10-12-initial-audit.md) предшествует этому сравнению исходников.
