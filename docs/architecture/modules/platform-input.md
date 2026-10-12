# Модуль: Platform Input

**Контекст:** системные input sources и synthetic targets.  
**Расположение:** contracts/validation/fixtures в `crates/deckweave-platform-input/`; first-party package shell в `plugins/platform-windows/`, позже Linux/macOS; privileged helpers — отдельные platform components.

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

R4 реализовал `InputSourceDescriptor`, `InputEmitterDescriptor`, `CapturePolicy` и проверки честной granularity/pass-through/fail-open. Срез 7.1 добавил platform-neutral `InputObservation`, явные pressed/repeated/released edges, key location, `InputOrigin` и `PrivilegeStatus`. `InputDecision`, synthetic output и host-owned held-output leases остаются отдельными будущими срезами; их нельзя подменять media-only короткими командами.

Windows baseline Фазы 7 использует Raw Input для фонового observe/pass-through без `RIDEV_NOLEGACY`. Первый descriptor остаётся `system_aggregate`; Raw Input handle/path не становится persisted physical identity без reconnect evidence. `WH_KEYBOARD_LL` зарезервирован для отдельного будущего intercept-helper, а не для базового наблюдения. Порядок и fault gates: [Фаза 7](../../implementation/phase-7-windows.md).

## Намеренно не обрабатывает

- выбор automation route;
- vendor-specific HID panels;
- host locks/autostart/credentials;
- firmware и произвольное переписывание keyboard memory.
