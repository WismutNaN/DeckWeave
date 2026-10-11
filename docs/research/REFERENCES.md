# Референсы DeckWeave

Документ фиксирует, какие решения из сторонних проектов исследуются и какие границы запрещают механический перенос их архитектуры или кода. Ссылки ведут на первичные репозитории и официальную документацию.

## Карта применимости

| Референс | Что проверяем | Что берём | Что не переносим |
|---|---|---|---|
| [OpenDeck](https://github.com/nekename/OpenDeck) | Host для OpenAction/Elgato, запуск плагинов, property inspectors, профили | Lifecycle отдельного процесса, WebSocket bridge, manifest discovery, recovery хранения | Модель, где основной контекст всегда является кнопкой/энкодером Stream Deck; ecosystem-specific domain внутри core |
| [OpenAction](https://github.com/OpenActionAPI/docs) | Платформенно-независимый protocol и обратная совместимость | Базовый vocabulary событий, open plugin ecosystem, тестовые плагины | Расширять legacy protocol так, чтобы он определял внутреннюю модель DeckWeave |
| [Elgato Stream Deck SDK](https://docs.elgato.com/streamdeck/sdk/) | Фактический manifest, process arguments, WebSocket registration и property inspector | Compatibility fixtures и ожидаемое поведение популярных плагинов | Предположение, что все устройства имеют только `Keypad` и `Encoder` |
| [OpenAction plugin catalogue](https://github.com/OpenActionAPI/plugins) | Открытые плагины для conformance | Небольшой публичный набор проверяемых плагинов | Обещание совместимости со всеми закрытыми marketplace-плагинами |
| [InputPlumber](https://github.com/ShadowBlip/InputPlumber) | Composite devices, source/target mapping, profiles | Разделение source/target, декларативный match, виртуальные targets | Linux-specific daemon architecture как обязательная форма DeckWeave |
| [Kanata](https://github.com/jtroo/kanata) | Слои, tap/hold, детерминированный event loop | Семантика времени, тестируемые state machines, OS adapters | Конфигурационный язык как единственный пользовательский интерфейс |
| [Wooting Analog SDK](https://github.com/WootingKb/wooting-analog-sdk) | Device plugins для аналоговых клавиатур | Нормализация аналоговых измерений, stable ABI как референс, virtual keyboard | Сведение всех аналоговых величин к «силе нажатия» |
| [StreamDock Device SDK](https://github.com/MiraboxSpace/StreamDock-Device-SDK) | N4 Pro: framebuffer, touch, LED, vibration, hot-plug | Реализация отдельного N4 Pro provider и hardware fixtures | Mirabox-команды в ядре DeckWeave |
| [opendeck-akp05](https://github.com/ambiso/opendeck-akp05) | Практический N4-family provider | Discovery, keepalive, disconnect cleanup, устройство как плагин | Фиксированное разбиение непрерывного touch-display на legacy encoder slots |
| [GStreamer](https://gstreamer.freedesktop.org/documentation/) | Граф audio/video: sources, filters, sinks, typed pads | Идея stream-портов, negotiation и lazy loading media elements | Собственный медиадвижок в control runtime |
| [PipeWire](https://docs.pipewire.org/) | Low-latency graph и out-of-process processing | Требования к real-time data plane и Linux adapter | PipeWire как обязательная зависимость на Windows/macOS |
| [CLAP](https://github.com/free-audio/clap) | Стабильный ABI аудиоэффектов и thread contracts | Будущий bridge к готовым EQ/noise plugins, validator-driven совместимость | Исполнение непроверенного native DSP в главном процессе |
| [Android MediaProjection](https://developer.android.com/media/grow/media-projection) | Удалённый экран и системные разрешения | Remote agent с явно выдаваемыми сессиями и отзывом доступа | Постоянный скрытый доступ к экрану, камере или микрофону |
| [Model Context Protocol Rust SDK](https://github.com/modelcontextprotocol/rust-sdk) | Optional MCP adapter для Codex/LLM clients | Отдельный adapter поверх стабильного local API | MCP как внутренний transport или источник истины |
| [Bitfocus Companion](https://github.com/bitfocus/companion) | Будущий bridge к modules/actions/variables | Version-pinned compatibility process и conformance matrix | Зависимость native runtime от внутреннего tRPC |

## Наиболее важные проверенные решения

### OpenDeck и OpenAction

OpenDeck использует Rust/Tauri host, запускает плагины отдельными процессами, выделяет им WebSocket endpoint и обслуживает HTML property inspectors. Это доказывает практичность compatibility host, но не требует использовать его внутреннюю модель `device/profile/controller/position` в DeckWeave.

OpenAction заявляет обратную совместимость со Stream Deck SDK и независимость от языка и устройства. Поэтому DeckWeave сначала реализует общую подмножество Elgato/OpenAction, а собственные расширения публикует отдельно и никогда не выдаёт их за возможности исходного SDK.

### Общая модель устройств

InputPlumber показывает полезность композиции нескольких физических sources в логическое устройство и раздельных target devices. Kanata показывает, что time-sensitive keyboard semantics необходимо реализовывать детерминированным state machine, а не цепочкой произвольных async callbacks. Wooting SDK показывает, что аналоговое устройство должно объявлять собственные измерения и диапазоны через provider.

### Визуальные поверхности

Официальный Mirabox SDK документирует временный полноэкранный background stream N4 Pro и touch events с координатами. Следовательно, DeckWeave должен моделировать `DisplaySurface` и независимые `TouchRegion`, а не наследовать сетку legacy-кнопок. Изображения отдельных LCD-клавиш остаются частным видом surface target.

### Аудио и видео

GStreamer связывает элементы через типизированные source/sink pads и загружает plugin только при запросе его element. PipeWire отделяет graph processing от управления и поддерживает low-latency out-of-process graph. CLAP задаёт стабильный ABI и явные требования к audio-thread. Вместе эти решения подтверждают необходимость отдельного media runtime, управляемого DeckWeave, но не исполняемого обычным JSON event loop.

### Удалённые узлы

Android требует явного пользовательского разрешения на MediaProjection и позволяет отозвать его в любой момент. Remote agent поэтому объявляет возможности отдельно от активных grants: наличие камеры или экрана не означает, что поток уже доступен.

## Лицензионные границы

| Проект | Лицензия на момент исследования | Политика DeckWeave |
|---|---|---|
| StreamDock Device SDK | MIT | Допускается использование с атрибуцией и проверкой конкретных файлов |
| OpenDeck | GPL-3.0-or-later | Совместимо с GPL application/core при сохранении notices/provenance; не переносить его device-centric domain автоматически |
| opendeck-akp05 | GPL-3.0 | Возможное заимствование только в GPL first-party plugin с origin/revision/modifications; hardware behavior проверять fixtures |
| InputPlumber | GPL-3.0-or-later | Возможное заимствование только после проверки границ/платформенной применимости; не переносить Linux daemon architecture целиком |
| Kanata | LGPL-3.0 | Использование библиотеки или кода требует отдельной проверки способа линковки и модификаций |
| Wooting Analog SDK | MPL-2.0 | Возможна файловая совместимость при соблюдении MPL; проверять перед заимствованием |
| CLAP | MIT | Кандидат для будущего audio compatibility bridge |
| DeckWeave protocol/SDK | MIT OR Apache-2.0 | Отдельные packages; позволяют независимым plugins использовать contracts |

Application/core DeckWeave планируется под GPL-3.0-or-later, а protocol/SDK/examples — под MIT OR Apache-2.0. Этот раздел не заменяет юридическую проверку перед публикацией релиза. Для каждого заимствования в PR должны быть указаны источник, версия, лицензия, способ использования и изменения.

## Дополнительное исходное исследование

[stack.md](stack.md) содержит первый развёрнутый анализ стека и мотивацию самостоятельного Rust-ядра. Он сохранён как исходное исследование и поэтому может содержать раннее предложение permissive-лицензии. Настоящий документ и [ADR-0007](../adr/0007-licensing.md) фиксируют последующее подтверждённое решение: GPL application/core и permissive protocol/SDK.
