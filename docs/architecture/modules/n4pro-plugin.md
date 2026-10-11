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
| `N4ProHapticTarget` | optional capability | Built-in feedback enable отдельно от подтверждённого pattern playback |

## Зависимости

| Модуль | Что использует |
|---|---|
| Plugin Rust SDK | Protocol/lifecycle |
| Mirabox Device SDK/protocol library | Hardware transport |
| Fixtures | Captured reports/frame checks |

## Инварианты

- USB IDs/model/firmware live only in plugin.
- Каждый подтверждённый непрерывный display target — отдельная surface; основной background и touch strip требуют проверенных dimensions/transform.
- Ten keys and four encoders/presses have stable element IDs.
- Late reports after disconnect ignored by session generation.
- Ring brightness/color reflect actual protocol, never simulated as image background.
- Safe cleanup/keepalive behavior tested.
- Encoder signed deltas не coalesce. Sample-only coordinates публикуются как pointer.sample; touch.points/hold/drag допускаются лишь после проверки contact lifecycle.
- Frame receipt различает transport acceptance и известное applied state; без подтверждаемой diff base используются full frames.

## Намеренно НЕ обрабатывает

- user surface layout;
- media/volume logic;
- OpenAction contexts;
- firmware flashing.

## Заметки для агента

> Сначала подтвердить protocol behavior на реальном N4 Pro и SDK fixtures. Неподдерживаемый per-ring mode виден явно. SDK-флаг разрешения встроенной вибрации не является командой воспроизведения pulse; HapticIn объявляется только после проверки самостоятельного actuator control.

[Source audit](../../research/implementation-review.md#n4-pro-полезный-код-и-ограничения-доказательств) обнаружил проигнорированный callback и отсутствие length check в C++ touch helper; Python decoder проверяет длину и передаёт TOUCH_POINT, но не доказывает down/up. Не переносить helper вслепую. Gate Фазы 8 включает screen addressing, raw coordinate range/rotation, event phases, ring control, haptic semantics и volatile/persistent writes на конкретных model/firmware.
