# Модуль: Device Studio

**Ответственность:** поддерживает discovery, live inspection, calibration, safe protocol capture, declarative mapping и provider conformance.
**Расположение:** application services в `crates/deckweave-device-studio/` и UI в `ui/desktop/`; conformance fixtures в testkit, без production dependency на него.

## Публичный интерфейс

| Символ | Тип | Описание |
|---|---|---|
| `ProbeSession` | operation | Bounded diagnostic hardware session |
| `CaptureTrace` | document | Timestamped input/report fixture |
| `CalibrationProfile` | document | Range/dead-zone/transform |
| `DeviceDescriptorProject` | document | Declarative mapping draft |
| `ConformanceRun` | operation/report | Provider test result |

## Зависимости

| Модуль | Что использует |
|---|---|
| Capability Registry | Candidates/descriptors |
| Plugin Supervisor | Probe lease |
| Diagnostics | Capture/replay |
| Profile Store | Calibration/descriptor drafts |

## Инварианты

- Probe session имеет explicit device scope и stop/deadline.
- Unknown output command не отправляется автоматически.
- Raw capture не попадает в default diagnostics export.
- Calibration не изменяет physical quantity semantics.
- Generated descriptor проходит validation до install/use.
- Probe использует общий ResourceLease arbiter; active exclusive device не открывается вторым handle.

## Намеренно НЕ обрабатывает

- автоматический reverse engineering любого protocol;
- firmware flashing;
- production automation execution.

## Заметки для агента

> Standard HID/gamepad path должен работать без Rust code. Vendor protocol project может стать input для Codex/plugin author, но генерация package не считается доказательством safety.
