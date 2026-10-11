# Безопасность и доверие DeckWeave

DeckWeave управляет вводом, устройствами, приложениями и в будущем media streams. Пользователь выбирает, какие packages устанавливать, но продукт обязан ясно показывать последствия, минимизировать случайный вред и не выдавать process isolation за полную sandbox.

## Trust model

Чувствительные результаты automation (голос, транскрипт, письма, уведомления) имеют scoped доступ и отдельный retention. [Runtime artifacts](automation.md#artifact-и-доступ-к-данным) не являются profile assets: не включаются автоматически в export, snapshots и diagnostics. Host проверяет grants перед выдачей каждого consumer lease и блокирует новые reads после revoke; уже прочитанное сторонним процессом невозможно отозвать задним числом.

Результат plugin не считается командой для shell или инструкцией изменить grants. Script/process adapters передают executable и argv раздельно, ограничивают stdout/stderr и валидируют result schema. Browser/tab, clipboard и account actions объявляют отдельные scopes/targets. Clipboard fallback не вставляет данные после незамеченной смены foreground target; restore проверяет ownership текущего clipboard value. Открыть письмо/вкладку, заполнить и отправить — разные effects в пользовательской конфигурации.

| Объект | Базовое доверие | Граница |
|---|---|---|
| Core/application | Audited project code | Один per-user process |
| Built-in adapter | Высокое, минимальный набор | In-process |
| Native control plugin | Выбран пользователем, потенциально ошибочный | Отдельный process, но права OS user |
| Legacy plugin | Неизвестные предположения/dependencies | Compatibility process tree |
| Custom web editor/widget | Web content package | Isolated origin/process + scoped bridge |
| Custom interaction view | Web content package внутри host-owned session | Isolated origin/process + session-scoped bridge |
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

Manifest request — не grant. Host broker не выдаёт ресурс или command dispatch без соответствующего актуального grant. Это enforcement внутри DeckWeave, а не запрет native executable обращаться к OS напрямую.

Grants принадлежат host authorization store, не profile revision. Они привязаны к principal/package, resource и grant revision; импорт, rollback и совпадение package ID не выдают доверие заново. Отзыв блокирует новые dispatch, инвалидирует broker handles и вызывает bounded cancel/cleanup; уже выполненный внешний эффект не отменяется автоматически. Prepare/commit повторно проверяют grants, чтобы отзыв во время apply не обходился старым plan.

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

Scopes contributions не создают memory isolation внутри общей runtime group. Перед выдачей secret UI/authorization учитывают суммарное доверие к process; отдельная trust boundary требует отдельного runtime. Даже отдельные процессы под тем же OS user не являются защитой от malicious native code без sandbox.

## Опасные операции

Трёхступенчатая модель:

1. **Describe:** получить capabilities, risk, current state и planned diff.
2. **Confirm:** пользователь или authorized client получает short-lived token для точного plan hash/device identity.
3. **Execute:** host проверяет token, session generation, deadline и performs operation.

Так работают provisioning, exclusive input interception, plugin install с high-risk permissions и future media capture. Confirmation нельзя переиспользовать для другого plan.

Token выдаёт host authorization broker после решения пользователя либо ранее выданного точного scope authorized client. Read/draft credential не может сам создать grant или confirmation. Token привязан к principal, operation kind, plan hash, resource identity/session, grant revision и TTL; consume фиксируется один раз, retry с тем же operation key читает receipt. После изменения plan/session/grant требуется новая проверка. Provisioning отсутствует в automation graph даже при наличии token — только отдельная application operation.

Firmware flashing запрещён и не может быть включён generic permission.

## Secrets

- Profile хранит `CredentialRef`, а не value.
- Installed mode использует OS credential store.
- Portable mode по умолчанию также ссылается на OS store и поэтому не обещает перенос secret.
- Non-private export исключает credential references, если пользователь не выбрал metadata.
- Private export никогда не содержит plaintext secret по умолчанию; encrypted bundle требует отдельного ADR/implementation.
- Logs, traces, crash reports и plugin protocol проходят redaction.
- Plugin получает secret только в scoped operation или через broker, если integration позволяет не раскрывать value.

Credential broker и OS store adapter входят в минимальные host platform services, а не lazy platform plugin. Profile Store знает только references. Import создаёт unresolved references; local credential/grant mapping выполняется отдельно, не по совпадению имени. Redaction применяется до записи diagnostic копии сообщения; secret-bearing wire payload предназначенному plugin не подменяется отредактированной строкой.

## Local API

- Только loopback bind.
- Случайный bearer/session token с user-only ACL.
- Origin/CSRF checks для HTTP.
- Rate/size limits.
- Read/draft/apply/admin scopes обязательны с первого ApplicationService; каждый adapter передаёт principal, service проверяет authorization.
- Dangerous command использует confirmation token.
- API discovery file не содержит reusable remote credential.
- Remote access к local API отсутствует по умолчанию.

Origin/Host validation и WebSocket handshake auth обязательны отдельно от CORS. Browser cookies не используются как implicit authority; bearer token не передаётся в URL/logs. Discovery не содержит общего admin credential. Отзыв client token закрывает subscriptions и запрещает новый dispatch в его scope, не отменяя уже committed effects.

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

- Отдельный browser context/storage partition и origin на package/session; непредсказуемый URL path сам по себе не создаёт origin isolation.
- Content Security Policy и запрет `file://` navigation.
- Scoped message bridge с schema validation.
- Никаких Tauri privileged commands.
- Network только при declared/granted permission.
- Files только через user-selected broker.
- Frame/input rate limits.
- Crash/hang не останавливает compositor/host.

Origin различается по scheme/host/port либо является sandboxed opaque origin; path не участвует в этой границе ([HTML Standard](https://html.spec.whatwg.org/multipage/browsers.html#origin)). Loopback ports без раздельного browser storage недостаточны для изоляции cookies. Поэтому backend обязан разделять storage, запрещать service workers и проверять cross-context fetch/storage/bridge fixtures. Если legacy PI требует несовместимого доступа, соответствующая возможность помечается unsupported; ослаблять общий privileged bridge нельзя.

### Interaction views

Interaction window всегда создаёт host, даже если package предоставляет custom content. Plugin не получает API создания/перемещения окон, global shortcuts, Tauri commands или общий local API token. Bridge ограничен `package + contribution + InteractionSessionId`, объявленной request/result schema, bounded pages и session deadline. Focus, Escape/cancel, clipboard policy, placement и закрытие остаются у host.

Schema-native views не запускают plugin web content. Custom runtime создаётся лениво, получает отдельный origin/storage partition и завершается либо уходит в bounded idle pool после закрытия последней session. Navigation, popup/new-window, service workers и download запрещены; network/filesystem доступны только через отдельные grants/brokers. Result валидируется host и может завершить только owner invocation.

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
- Bundled keyboard provider по умолчанию работает observe/pass-through и честно объявляет `system_aggregate`, если OS не даёт identity физического устройства.
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
- Bypass применяется только при согласованном format и живом обходном пути; иначе stream останавливается либо выдаёт silence по policy.

## Supply chain и лицензии

- Каждый заимствованный fragment/dependency фиксирует origin, revision, license и modifications.
- `cargo-deny`/audit equivalents планируются в CI.
- Lockfiles committed.
- Release artifacts воспроизводимы настолько, насколько позволяет toolchain, и публикуют SBOM/checksums.
- GPL core и permissive SDK находятся в явно лицензированных packages; правовая оценка plugin boundary не подменяется технической документацией.

## Incident/diagnostics

Diagnostics bundle по умолчанию содержит versions, health, redacted errors и metrics. Raw input, window titles, notification text, URLs и media samples требуют отдельного opt-in. Перед export показывается точный состав bundle.
