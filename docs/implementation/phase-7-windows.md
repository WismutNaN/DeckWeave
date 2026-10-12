# Фаза 7: Windows base provider и Platform Input

Фаза создаёт первый полезный first-party platform package без N4-specific logic. Нормативные границы заданы [Platform Adapters](../architecture/modules/platform-adapters.md), [Platform Input](../architecture/modules/platform-input.md), [ADR-0017](../decisions/0017-bundled-platform-input.md) и [ADR-0020](../decisions/0020-windows-provider-runtime-groups.md). Выбор Windows API и provenance референсов зафиксированы в [исследовании](../research/phase-7-windows.md).

## Состояние

Фаза начата. Срезы 7.0 и 7.1 завершены: source audit, neutral input contracts, schemas, manifest template и исполняемые shells двух runtime groups проходят production process/supervisor boundary. Все endpoint-ы package пока честно объявлены `unavailable`: реальных Windows effects до среза 7.2 нет. Каждый следующий срез должен оставлять `npm run check` и workspace tests зелёными и не объявлять fake/manual result production capability.

Phase 6 может сохранять отдельные открытые UI/E2E acceptance items: они не разрешают Windows package обходить Application Gateway или plugin lifecycle. Phase 7 использует существующий production composition и добавляет capabilities через общий package/runtime contract.

## Принятые границы

- Package ID: `org.deckweave.platform.windows`.
- Runtime group `control`: audio, media, active-window/catalog/focus, exact process launch и file/URI activation.
- Runtime group `input-observe`: Raw Input keyboard source. Отдельный process нужен не «на каждую функцию», а для независимых permissions, message-loop lifetime и отказов.
- `input.intercept`, generic `input.emit` и scripts не входят в эти groups. Их появление потребует отдельного grant/runtime group и acceptance gate.
- Windows-specific DTO остаются внутри package. Статический manifest публикует только namespaced JSON schemas и общие endpoint descriptors.
- Начальный keyboard descriptor — `system_aggregate`, `canObserve=true`, `canIntercept=false`. Observe policy всегда `passThrough=true`, `failOpen=true`.

## 7.0 — Evidence, API и reuse plan

**Статус:** [x]

- [x] Сверить официальные Core Audio, window, Raw Input, SendInput, Shell и GSMTC APIs.
- [x] Сопоставить OpenDeck/InputPlumber/Kanata/Wooting с фактическими границами DeckWeave.
- [x] Зафиксировать local `io_type_84` commit/files/hashes и не копировать код без явной лицензии/provenance.
- [x] Выбрать два runtime groups и список функций, которые намеренно не входят в baseline.

Выход: [research/phase-7-windows.md](../research/phase-7-windows.md) и [ADR-0020](../decisions/0020-windows-provider-runtime-groups.md).

## 7.1 — Contracts, schemas и package shell

**Статус:** [x]

Сначала добавляется исполняемый package shell и platform-neutral runtime vocabulary; реальные OS effects ещё заменены controlled adapters.

- [x] Добавить `InputObservation`, edge/repeat semantics, normalized key identity, `InputOrigin` и `PrivilegeStatus` в `deckweave-platform-input`. Не добавлять Win32 virtual-key/HWND в Shared Kernel.
- [x] Определить package schemas для audio commands/state, media transport, app target, window selector/ref, active-window state и keyboard observation.
- [x] Создать manifest с двумя artifacts/runtime groups, contribution → group mapping, минимальными Windows architecture filters и permissions:
  - `windows.audio.observe`, `windows.audio.control`;
  - `windows.window.observe`, `windows.window.activate`;
  - `windows.process.launch`, `windows.shell.open`;
  - `windows.media.control`;
  - `input.observe`.
- [x] Добавить stable unavailable/error mapping: unsupported OS/build, permission denied, no default endpoint, no media session, stale window reference, foreground denied, launch rejected, OS API unavailable, transient rebind.
- [x] Поднять обе runtime groups через production supervisor на controlled adapters; activation commit предшествует signal/state publication.

Acceptance:

- [x] manifest/schema fixtures проходят validation без запуска executable;
- [x] permission revoke одной group не останавливает независимую group;
- [x] два одинаковых local endpoint IDs не конфликтуют благодаря полному `EndpointPath`;
- [x] Windows crate/dependency не появляется в domain/application/automation crates;
- [x] non-Windows workspace build компилирует явный unsupported shell без Win32 linkage.

Evidence: `plugins/platform-windows/tests/package_shell.rs` собирает временный пакет из реальных binaries, индексирует его общим `PackageStager`, запускает обе группы через `ProcessRuntime` и проверяет scoped revoke через `RuntimeController`. `scripts/check-schemas.mjs` валидирует manifest template и восемь package schemas. Это проверка lifecycle и границ, а не доказательство готовности audio/window/media/input adapters.

## 7.2 — Core Audio vertical slice

- Выделить COM MTA worker, который владеет `IMMDeviceEnumerator`, текущим render/multimedia endpoint и `IAudioEndpointVolume`.
- Реализовать `set_volume`, `change_volume`, `set_mute`, `toggle_mute`; значения clamp/validate выполняются в package application layer до FFI.
- Подключить `IAudioEndpointVolumeCallback` и `IMMNotificationClient`. Callback складывает минимальное событие в bounded channel; rebind/default-device resolution выполняет worker.
- Публиковать snapshot `{endpointRef, scalar, muted, availability, revision}`. EndpointRef не раскрывает raw COM identity и меняется при default-device switch.
- Собственный context GUID помечает изменения DeckWeave там, где Core Audio API его поддерживает; callback всё равно публикует подтверждённое состояние, а не предполагаемый результат команды.

Acceptance:

- unit: HRESULT/Win32 result → domain/plugin outcome mapping;
- adapter: no endpoint, endpoint removed during command, callback burst, default switch, revoke during rebind;
- manual Windows 10/11: set/read round-trip и external volume/mute change;
- действие не объявляется успешным только потому, что command был поставлен в очередь.

## 7.3 — Active window, catalog, focus и launch

- Dedicated message thread устанавливает out-of-context foreground hook; callback передаёт только HWND/event metadata worker-у.
- Worker строит `ActiveApplicationState`: ephemeral WindowRef, PID/session evidence, executable/app selector, bounded title metadata и freshness. Raw HWND не сохраняется в profile.
- Catalog различает app identity и конкретные windows, фильтрует только документированные признаки и возвращает generation. Icon loading остаётся bounded optional query/cache.
- `activate_window` проверяет generation/ref, существование окна и фактический foreground после вызова. Отказ Windows — нормальный typed outcome.
- `launch_process` принимает exact executable + argv + cwd policy. `open_target` отдельно принимает file/URI и исполняется на STA Shell worker. Готовность окна — отдельный predicate, не часть launch receipt.

Acceptance:

- два окна одного executable, закрытие между list/click, reused handle, elevated target, empty title, missing icon;
- activation denied не превращается в success;
- Unicode paths/argv и пробелы не проходят через shell concatenation;
- app/window state переключает virtual feedback через существующий automation path.

## 7.4 — Media transport и origin

Первый production backend отправляет allowlisted `play_pause`, `next`, `previous`, `stop` как одну bounded down/up batch через `SendInput`.

- Каждый event содержит process-random origin tag в `dwExtraInfo`; tag живёт только в generation runtime и не является security token.
- Частичный send вызывает best-effort release и возвращает `partial_or_unknown`, а не success.
- UIPI limitation публикуется в descriptor/unavailable diagnostics; provider не обещает управление elevated/защищённым target.
- Эти короткие пары не публикуют generic input emitter и не создают held output. Перед первым held emission обязателен host-owned lease manager.
- Отдельный GSMTC spike проверяет packaged/unpackaged desktop build, `globalMediaControl`, current session changes, metadata и отсутствующую session. Только после evidence он может стать preferred backend/state provider; SendInput остаётся explicit fallback.

Acceptance:

- unit: complete/zero/partial `SendInput`, release attempt и error mapping;
- scenario: DeckWeave-origin media event не запускает keyboard binding повторно;
- manual: common media app, no session, elevated target, rapid repeat;
- GSMTC metadata не появляется в manifest как available до compatibility fixture/manual evidence.

## 7.5 — Raw Input keyboard observe

- `input-observe` создаёт message-only window и регистрирует keyboard TLC с `RIDEV_INPUTSINK | RIDEV_DEVNOTIFY`, без `RIDEV_NOLEGACY`.
- Message thread считывает `WM_INPUT`/device change и передаёт bounded raw record normalizer-у. Никакой plugin IPC/automation work внутри window proc.
- Normalizer выдаёт pressed/released/repeat с scan-code semantics и отдельным logical display label. Layout-dependent text не подменяет physical key identity.
- Первый public descriptor агрегирует keyboards и не сохраняет Raw Input device handle/path как physical identity. Per-device spike может повысить granularity только после reconnect/two-keyboard evidence.
- Neutral rearm: после overflow/restart/device removal неизвестное held state сбрасывается и следующий edge не дублируется. Source sequence/generation защищают от stale events.

Acceptance:

- normal key запускает virtual action, одновременно продолжая обычное действие ОС;
- key down/repeat/up, reconnect и runtime restart не создают duplicate terminal edge;
- slow consumer/queue overflow дают наблюдаемый fault, а не бесконечную память;
- две клавиатуры не заявляются физически различимыми, пока evidence отсутствует;
- close/revoke снимает registration/message loop за bounded deadline.

## 7.6 — Production composition и release evidence

- Установить package как bundled immutable artifact с проверяемым manifest/hash, без install script.
- Подключить lazy leases: неиспользуемая runtime group не запускается и не занимает постоянную память.
- Добавить runtime health/unavailable reasons в существующие desktop projections; UI не получает Win32 handles или plugin process controls.
- Выполнить end-to-end: virtual/keyboard signal → compiled automation → Windows action → callback state → feedback.
- Зафиксировать Windows 10/11 manual matrix, crash/restart/revoke, suspend/resume, default endpoint switch и foreground denial.
- Обновить `THIRD_PARTY.md` только для реально добавленных dependencies/заимствований; source references без copied code туда не выдаются за bundled component.

Gate Фазы 7 закрывается, когда volume/mute/media/window/launch и aggregate keyboard observe работают через production package lifecycle, а не через in-process shortcut. GSMTC metadata, intercept, scripts и generic held input могут остаться последующими явно отмеченными срезами и не блокируют baseline, если UI/manifest честно показывает их отсутствие.

## Rollback

Каждый срез — отдельный commit. Package artifacts и manifest добавляются вместе, но active workspace lock не обновляется автоматически. При отказе нового Windows adapter удаляется только соответствующий bundled package/version; core contracts остаются backward-compatible. Нельзя откатывать runtime crash удалением пользовательского workspace или grant store.
