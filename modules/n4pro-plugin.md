# Модуль: N4 Pro Plugin

**Ответственность:** реализует Mirabox N4 Pro discovery/protocol и публикует его hardware как стандартные DeckWeave capabilities.
**Расположение:** `plugins/device-n4pro/`.

## Публичный интерфейс

| Символ | Тип | Описание |
|---|---|---|
| `N4ProProvider` | `DeviceProvider` | Discovery/open/hotplug |
| `N4ProDescriptor` | descriptor | Keys/encoders/surface/light/haptic |
| `N4ProSession` | runtime handle | HID/session/keepalive |
| `N4ProFrameSink` | `FrameSink` | Full display frames |
| `N4ProLightTarget` | command/value target | Individual rings where supported |
| `N4ProHapticTarget` | haptic target | Enable/disable/pattern degradation |

## Зависимости

| Модуль | Что использует |
|---|---|
| Plugin Rust SDK | Protocol/lifecycle |
| Mirabox Device SDK/protocol library | Hardware transport |
| Fixtures | Captured reports/frame checks |

## Инварианты

- USB IDs/model/firmware live only in plugin.
- Full touch display is one surface; grid is not hardware truth.
- Ten keys and four encoders/presses have stable element IDs.
- Late reports after disconnect ignored by session generation.
- Ring brightness/color reflect actual protocol, never simulated as image background.
- Safe cleanup/keepalive behavior tested.

## Намеренно НЕ обрабатывает

- user surface layout;
- media/volume logic;
- OpenAction contexts;
- firmware flashing.

## Заметки для агента

> Сначала подтвердить protocol behavior на реальном N4 Pro и SDK fixtures. Unsupported per-ring capability must be reported honestly. Haptic complex patterns may degrade to timed on/off with explicit quality.

