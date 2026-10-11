# Модуль: Platform Input

**Контекст:** системные input sources и synthetic targets.  
**Расположение:** bundled packages `plugins/platform-windows/`, позже Linux/macOS; узкие privileged helpers — отдельные platform components.

## Ответственность

- наблюдение обычной клавиатуры/мыши и доступных media controls;
- явное объявление source identity и granularity;
- отдельные capabilities `observe`, `intercept` и `emit`;
- origin marker для synthetic input и cleanup held outputs;
- перевод OS events в platform-neutral descriptors.

## Безопасный базовый режим

Bundled provider доступен сразу после установки, но по умолчанию только наблюдает и пропускает ввод. Если ОС не позволяет различить физические клавиатуры, он объявляет агрегированный системный source и не создаёт ложные `DeviceIdentity`. Interception требует отдельного grant/helper и при сбое работает fail-open.

## Публичная граница

| Символ | Роль |
|---|---|
| `InputSourceDescriptor` | вид ввода, granularity и фактические modes |
| `InputObservation` | нормализованное edge/value событие |
| `InputDecision` | pass/suppress/replace для подготовленного intercept path |
| `SyntheticInput` | typed output с origin и lease |
| `PrivilegeStatus` | доступность platform helper и причина downgrade |

## Намеренно не обрабатывает

- выбор automation route;
- vendor-specific HID panels;
- host locks/autostart/credentials;
- firmware и произвольное переписывание keyboard memory.
