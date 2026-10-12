# Исследование Фазы 7: Windows provider и Platform Input

**Статус:** решение перед реализацией
**Дата проверки:** 2026-10-12
**Область:** Windows 10/11, first-party control package, штатная клавиатура в observe/pass-through mode

Этот документ определяет, что DeckWeave заимствует из референсов, а что реализует как собственный узкий adapter. Он не является доказательством работы реального Windows runtime: каждый выбранный API проходит отдельный contract, integration и manual gate в [плане Фазы 7](../implementation/phase-7-windows.md).

## Итог выбора

- Один first-party package `org.deckweave.platform.windows` содержит две независимые runtime groups: `control` и `input-observe`. Они запускаются только при наличии lease и имеют раздельные permissions/failure domains.
- Master volume/mute реализуются через Core Audio endpoint APIs и callbacks, а не через media-key emulation. Смена default endpoint приводит к rebind и новому state snapshot.
- Active-window state приходит через `SetWinEventHook(EVENT_SYSTEM_FOREGROUND, WINEVENT_OUTOFCONTEXT)`; перечисление и активация окон остаются отдельными операциями. Успешное перечисление не доказывает право вызвать `SetForegroundWindow`.
- Запуск executable и открытие файла/URI — разные typed operations. Произвольный shell source не входит в Windows base provider.
- Базовые media transport commands используют короткую атомарную пару `SendInput` down/up с DeckWeave origin tag. Generic input emission и held keys не публикуются до host-owned output lease.
- Штатная клавиатура наблюдается через Raw Input в фоне: `RIDEV_INPUTSINK | RIDEV_DEVNOTIFY`, без `RIDEV_NOLEGACY`. Поэтому исходное действие ОС сохраняется. Первый provider объявляет `system_aggregate`; переход к `os_logical_device` требует отдельного доказательства стабильности identity и reconnect mapping.
- `WH_KEYBOARD_LL` не используется для безопасного observe baseline. Microsoft рекомендует Raw Input для наблюдения; low-level hook имеет message-loop/deadline requirements и может быть молча удалён при timeout. Такой hook допустим только в будущем intercept-helper с fail-open policy.
- GSMTC рассматривается отдельным spike для current session/track metadata и прямых media-session commands. Наличие API начиная с Windows build 17763 и `globalMediaControl` capability ещё не доказывает работу packaged/unpackaged сборок DeckWeave.

## Первичные Windows API

| Задача | Выбранный API | Обязательная граница |
|---|---|---|
| Master volume/mute | [`IMMDeviceEnumerator`](https://learn.microsoft.com/en-us/windows/win32/api/mmdeviceapi/nn-mmdeviceapi-immdeviceenumerator), [`IAudioEndpointVolume`](https://learn.microsoft.com/en-us/windows/win32/api/endpointvolume/nn-endpointvolume-iaudioendpointvolume) | COM worker владеет interfaces; отсутствие endpoint и exclusive-mode ограничения не маскируются |
| Audio state/hotplug | [`IAudioEndpointVolumeCallback`](https://learn.microsoft.com/en-us/windows/win32/api/endpointvolume/nn-endpointvolume-iaudioendpointvolumecallback), [`IMMNotificationClient`](https://learn.microsoft.com/en-us/windows/win32/api/mmdeviceapi/nn-mmdeviceapi-immnotificationclient) | callback только ставит bounded event; default-device change пересоздаёт endpoint binding |
| Foreground state | [`SetWinEventHook`](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-setwineventhook) | dedicated message thread; callback не читает process/icon и не блокируется на IPC |
| Window catalog | [`EnumWindows`](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-enumwindows) | `WindowRef` session-scoped; title/HWND не являются persisted app identity |
| Window activation | [`SetForegroundWindow`](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-setforegroundwindow) | Windows может отклонить focus; результат проверяется и возвращается как typed denial |
| Exact process launch | `std::process::Command`/`CreateProcessW` semantics | executable, argv, cwd и env передаются раздельно; никакой shell interpolation |
| File/URI activation | [`ShellExecuteExW`](https://learn.microsoft.com/en-us/windows/win32/api/shellapi/nf-shellapi-shellexecuteexw) | отдельный STA executor; успешный launch не означает готовность окна/документа |
| Keyboard observe | [Raw Input](https://learn.microsoft.com/en-us/windows/win32/inputdev/about-raw-input) | message-only window, background delivery, pass-through, bounded normalizer queue |
| Media key output | [`SendInput`](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-sendinput) | UIPI может блокировать injection; частичная вставка и release attempt дают отдельный outcome |
| Injected origin evidence | [`KBDLLHOOKSTRUCT`](https://learn.microsoft.com/en-us/windows/win32/api/winuser/ns-winuser-kbdllhookstruct) | `LLKHF_INJECTED`/`dwExtraInfo` полезны только в hook path; не приписываются Raw Input без test evidence |
| Media session metadata, optional | [`GlobalSystemMediaTransportControlsSessionManager`](https://learn.microsoft.com/en-us/uwp/api/windows.media.control.globalsystemmediatransportcontrolssessionmanager.requestasync) | отдельный compatibility spike и unavailable reason при отсутствии capability/session |

Rust binding — официальный [`microsoft/windows-rs`](https://github.com/microsoft/windows-rs), MIT OR Apache-2.0. Windows dependencies находятся только в plugin infrastructure crates/modules и включают минимальные feature flags. Общие DeckWeave contracts не импортируют Win32/COM types. `unsafe` разрешается локально вокруг FFI с инвариантом безопасности у каждого блока; workspace-wide ослабление lint запрещено.

## Референсы проектов

| Референс | Что используем | Что не переносим |
|---|---|---|
| OpenDeck | plugin lifecycle/settings patterns и опыт desktop host | его core/profile model и GPL-код как скрытую часть DeckWeave SDK |
| InputPlumber | разделение source/target/composite, bounded event paths, recovery mindset | Linux device taxonomy и daemon как Windows core |
| Kanata/keyberon | edge/timing/layer fixtures для будущих recognizers | LGPL Windows backend или keyboard-specific engine в Phase 7 baseline |
| Wooting Analog SDK | explicit init/close/callback lifetime и version negotiation | vendor ABI как универсальную keyboard identity model |
| `G:/APPS/io_type_84` | локальные проверяемые примеры `SendInput`, foreground activation и Core Audio probe | прямое копирование до явного определения copyright/license и file-level provenance |

### Локальный `io_type_84`

Проверен clean HEAD `9a6553bb486a754a6cd82b379cfcdac3bdb7ebb1`. Полезные файлы:

| Файл | SHA-256 | Наблюдение |
|---|---|---|
| `crates/io-platform/src/computer.rs` | `3C9D06DC74BB5C24BEB3D24A39890DD05538C573B0B91462F6A9136A8D946A9C` | allowlisted media/volume key pair, origin tag, partial-send release attempt |
| `crates/io-platform/src/app_windows.rs` | `BA881D0FC53FC0EA744AA6AE2334F64B3D58A7C7F4EDC0153937693406F1CE48` | EnumWindows/process path, activation denial, exact executable launch |
| `crates/io-platform/examples/support/audio_probe.rs` | `0C8D7B1D16F05F59679076669A18D6E6D90ABEBC05701F2820D1F8FE52073867` | COM apartment lifetime и read-only endpoint volume probe |

В корне и выбранных Cargo/source metadata не обнаружено явное license expression или file SPDX. Поэтому эти файлы пока являются design/test reference. Если код будет переноситься, отдельный commit фиксирует авторство, разрешение/лицензию, точные строки происхождения, изменения и tests; `THIRD_PARTY.md` обновляется в том же commit.

## Runtime и DDD-границы

```text
Automation invocation         Windows callback/event
          |                             |
          v                             v
plugin protocol interface      bounded adapter channel
          |                             |
          v                             v
control application port       input/state normalizer
          |                             |
          +------------+----------------+
                       v
              Win32/COM infrastructure
```

- Platform Input владеет normalized observation/origin/granularity/capture policy, но не automation route.
- Windows control package владеет переводом Win32/COM results в plugin outcomes, но не host authorization и не persisted workspace.
- Capability Registry получает descriptors/states/signals через общий protocol; raw HWND, COM pointer, hook handle и device handle через IPC не проходят.
- Callback threads не пишут в stdio и не ждут host. Они публикуют в bounded channel; reliable edges при overflow переводят runtime в fault/degraded, replaceable state coalesces latest value.
- Отзыв runtime lease останавливает message loops, снимает hooks/callbacks и закрывает COM apartment до terminal process exit.

## Не включать в базовый срез

- global interception, replacement и suppression;
- generic keyboard/mouse emission, chords и held key-down;
- scripts/PowerShell и unrestricted process execution;
- tray enumeration через undocumented Explorer internals;
- per-app audio, audio capture/DSP и virtual microphone;
- обещание физической keyboard identity по одному Raw Input handle/path;
- track metadata до GSMTC compatibility evidence.

## Gate прямого переиспользования

Перед vendoring/copying конкретного fragment обязательны: exact repository/commit/file, лицензия и notices, сравнение ownership/failure semantics, минимальный diff, unit/adapter fixture и update strategy. Если подходящий API покрывается официальным `windows-rs`, предпочтителен небольшой собственный adapter: это уменьшает чужую domain coupling и объём кода, который DeckWeave обязан сопровождать.
