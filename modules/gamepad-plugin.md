# Модуль: Gamepad Plugin

**Ответственность:** публикует Bluetooth/USB gamepads как buttons, axes, motion, battery, haptic и light capabilities без привязки core к gamepad class.
**Расположение:** `plugins/device-gamepad/`.

## Публичный интерфейс

| Символ | Тип | Описание |
|---|---|---|
| `GamepadProvider` | `DeviceProvider` | Enumeration/hotplug |
| `GamepadDescriptor` | descriptor | Controls and outputs |
| `AxisCalibration` | settings | Range/dead-zone/curve |
| `MotionSample` | value | Gyro/accelerometer with units |
| `RumbleTarget` | haptic target | Supported vibration channels |
| `GamepadFixture` | test artifact | Replayable virtual device |

## Зависимости

| Модуль | Что использует |
|---|---|
| Plugin Rust SDK | Lifecycle/capabilities |
| Cross-platform gamepad library/OS backend | Basic device access |

## Инварианты

- Missing gyro/rumble/light is unsupported, not zero.
- Reconnect preserves identity when evidence permits.
- Axis values include raw/calibrated semantics.
- High-rate motion uses value coalescing and metrics.
- Force feedback wheel APIs are not implied by simple rumble.

## Намеренно НЕ обрабатывает

- wheel force-feedback engine;
- virtual target implementation;
- automation mapping.

## Заметки для агента

> Этот plugin — ранний architecture test против Stream Deck bias. Bluetooth reconnect, multiple devices, motion rate and rumble cancellation должны входить в fixtures.

