# Модуль: Platform Adapters

**Ответственность:** предоставляет OS-dependent input/output, active application, volume/media и virtual targets как isolated plugin contributions. Bundled normal-keyboard path уточнён в [Platform Input](platform-input.md). Locks, autostart и credentials принадлежат [Host Platform Services](host-platform-services.md).
**Расположение:** `plugins/platform-windows/`, позже `plugins/platform-linux/` и `plugins/platform-macos/`.

## Публичный интерфейс

Целевой Windows-набор разделён по самостоятельным возможностям: Audio, Media, Windows, Applications и Keyboard observation ([ADR-0023](../../decisions/0023-composable-windows-plugins.md)). Текущий двухгрупповой package ещё требует миграции. Reusable live data/commands предоставляются через [service bindings](../../contracts/plugin-composition.md), общие stateless adapters — через SDK/library; установка набора не запускает все его процессы.

| Символ | Тип | Описание |
|---|---|---|
| `PlatformProvider` | plugin contribution | OS capability collection |
| `InputObservation` | event | Physical/system input |
| `InputDecision` | enum | Pass/suppress/replace |
| `SyntheticInput` | command | Keyboard/mouse/media/gamepad output |
| `ActiveApplicationState` | state | Process/window identity |
| `AudioEndpointControl` | command/state | Volume/mute |
| `PrivilegeStatus` | state | Available helper/mode |

## Зависимости

| Модуль | Что использует |
|---|---|
| Plugin SDK | Control protocol |
| OS APIs | Windows first, platform-specific |

## Инварианты

- Observe не обещает suppress.
- Synthetic events получают origin marker/loop policy.
- Privileged helper имеет минимальный contract.
- Input interception fail-open unless explicit safe policy.
- OS-specific types переводятся в domain descriptors на boundary.
- Intercept deadline не ждёт round-trip через async automation/plugin chain; будущий helper использует заранее подготовленное ограниченное decision rule и fail-open timeout.
- Output key-down/release принадлежит lease; disconnect/revoke/layer switch запускает cleanup.

## Намеренно НЕ обрабатывает

- automation decisions;
- device-specific vendor HID;
- firmware;
- cross-platform lowest-common-denominator illusion.

## Заметки для агента

> Первая Windows contribution: media commands, volume/mute, application launch/focus и active-window state. Full interception следует отдельной фазой после recovery design.

Обычная клавиатура поставляется тем же first-party package и видна пользователю как базовая возможность, но не получает скрытого core API. Observe/pass-through включается как безопасный default; intercept и emit остаются отдельными grants.

Принятое Windows-разбиение — один package с runtime groups `control` и `input-observe`. Это сохраняет lazy activation и не смешивает Raw Input message loop с Core Audio/window failures. Scripts, generic input emission и interception в эти groups не входят. Пользовательские процессы нельзя выпускать из plugin Job Object через общий `BREAKAWAY_OK`: exact launch/open выполняет узкий Host broker по [ADR-0021](../../decisions/0021-brokered-user-process-launch.md). Детали runtime groups: [ADR-0020](../../decisions/0020-windows-provider-runtime-groups.md) и [план Фазы 7](../../implementation/phase-7-windows.md).
