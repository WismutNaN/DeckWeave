# Безопасность и доверие DeckWeave

DeckWeave управляет вводом, устройствами, приложениями и в будущем media streams. Пользователь выбирает, какие packages устанавливать, но продукт обязан ясно показывать последствия, минимизировать случайный вред и не выдавать process isolation за полную sandbox.

## Trust model

| Объект | Базовое доверие | Граница |
|---|---|---|
| Core/application | Audited project code | Один per-user process |
| Built-in adapter | Высокое, минимальный набор | In-process |
| Native control plugin | Выбран пользователем, потенциально ошибочный | Отдельный process, но права OS user |
| Legacy plugin | Неизвестные предположения/dependencies | Compatibility process tree |
| Custom web editor/widget | Web content package | Isolated origin/process + scoped bridge |
| Native media plugin | Высокое доверие требуется | Отдельный media host |
| Remote agent | Paired device | Mutual identity + revocable grants |
| API client | Local authenticated principal | Scoped token/session |

Отдельный process локализует crash и упрощает restart. Без OS sandbox он не мешает злонамеренному executable обращаться к ресурсам пользователя вне DeckWeave. UI должен говорить об этом прямо.

## Permission model

Permission состоит из:

- action family;
- resource selector;
- access mode;
- duration/persistence;
- requesting package/contribution;
- human-readable reason;
- grant revision.

Manifest request — не grant. Runtime не получает ресурс, если request не соответствует сохранённому grant.

Особо чувствительные permissions:

- `input.intercept` и `input.emit`;
- `device.open.exclusive`;
- `device.provision`;
- `process.spawn`;
- unrestricted filesystem/network;
- secrets;
- camera/microphone/screen capture;
- media native code.

Изменение permission set в update блокирует activation до подтверждения.

## Опасные операции

Трёхступенчатая модель:

1. **Describe:** получить capabilities, risk, current state и planned diff.
2. **Confirm:** пользователь или authorized client получает short-lived token для точного plan hash/device identity.
3. **Execute:** host проверяет token, session generation, deadline и performs operation.

Так работают provisioning, exclusive input interception, plugin install с high-risk permissions и future media capture. Confirmation нельзя переиспользовать для другого plan.

Firmware flashing запрещён и не может быть включён generic permission.

## Secrets

- Profile хранит `CredentialRef`, а не value.
- Installed mode использует OS credential store.
- Portable mode по умолчанию также ссылается на OS store и поэтому не обещает перенос secret.
- Non-private export исключает credential references, если пользователь не выбрал metadata.
- Private export никогда не содержит plaintext secret по умолчанию; encrypted bundle требует отдельного ADR/implementation.
- Logs, traces, crash reports и plugin protocol проходят redaction.
- Plugin получает secret только в scoped operation или через broker, если integration позволяет не раскрывать value.

## Local API

- Только loopback bind.
- Случайный bearer/session token с user-only ACL.
- Origin/CSRF checks для HTTP.
- Rate/size limits.
- Separate read/draft/apply/admin scopes, когда появятся внешние clients.
- Dangerous command использует confirmation token.
- API discovery file не содержит reusable remote credential.
- Remote access к local API отсутствует по умолчанию.

## Plugin install/update

1. Download/local file попадает в staging.
2. Проверяются archive size, path traversal, symlinks, manifest/schema.
3. Вычисляется hash; source/provenance сохраняется.
4. UI показывает license, entrypoints, runtimes и permissions.
5. Version устанавливается в immutable directory.
6. Package не запускается до activation.
7. Update проходит такую же проверку и сохраняет rollback version.

Install scripts запрещены. Каталог не делает package доверенным автоматически.

## Web property editors и widgets

- Unique origin/session per package/context.
- Content Security Policy и запрет `file://` navigation.
- Scoped message bridge с schema validation.
- Никаких Tauri privileged commands.
- Network только при declared/granted permission.
- Files только через user-selected broker.
- Frame/input rate limits.
- Crash/hang не останавливает compositor/host.

## Remote agents

- Pairing подтверждается на обоих устройствах.
- Node identity отделена от IP address.
- Transport encryption и mutual authentication обязательны.
- Capability advertisement не является permission.
- Camera, microphone, speaker, display capture и touch имеют отдельные revocable grants.
- Control и media sessions имеют deadlines/session generations.
- Lost/revoked agent немедленно инвалидирует handles.

## Input safety

- Observe, intercept и emit — отдельные grants.
- Injected events получают synthetic origin.
- Default loop guard не направляет synthetic event обратно к source route.
- Privileged helper имеет минимальный protocol и не читает workspace.
- При crash/timeout interception component должен fail open, если выбранная функция не требует fail closed.
- Emergency disable shortcut/physical recovery procedure обязателен до выпуска suppression.

## Device safety

- Provider публикует только typed safe commands.
- Raw write доступен только внутри provider implementation, не graph/API.
- Brightness, haptic amplitude, duty cycle и rate ограничиваются descriptor.
- Safe state on disconnect/shutdown задаётся provider.
- Provisioning требует identity check, plan hash, readback и report.
- Firmware update отсутствует.

## Media safety

- Native DSP не загружается в desktop host.
- Media process имеет отдельный crash boundary.
- Capture требует visible/revocable session grant.
- Audio thread не пишет secrets/log payloads и не выполняет network/filesystem operations.
- Bypass применяется только при согласованном format; иначе stream останавливается.

## Supply chain и лицензии

- Каждый заимствованный fragment/dependency фиксирует origin, revision, license и modifications.
- `cargo-deny`/audit equivalents планируются в CI.
- Lockfiles committed.
- Release artifacts воспроизводимы настолько, насколько позволяет toolchain, и публикуют SBOM/checksums.
- GPL core и permissive SDK находятся в явно лицензированных packages; правовая оценка plugin boundary не подменяется технической документацией.

## Incident/diagnostics

Diagnostics bundle по умолчанию содержит versions, health, redacted errors и metrics. Raw input, window titles, notification text, URLs и media samples требуют отдельного opt-in. Перед export показывается точный состав bundle.

