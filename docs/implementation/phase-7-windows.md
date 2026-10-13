# Фаза 7: Windows base provider и Platform Input

Фаза создаёт первый полезный first-party platform package без N4-specific logic. Нормативные границы заданы [Platform Adapters](../architecture/modules/platform-adapters.md), [Platform Input](../architecture/modules/platform-input.md), [ADR-0017](../decisions/0017-bundled-platform-input.md) и [ADR-0020](../decisions/0020-windows-provider-runtime-groups.md). Выбор Windows API и provenance референсов зафиксированы в [исследовании](../research/phase-7-windows.md).

## Состояние

Фаза начата. Срезы 7.0 и 7.1 завершены; в 7.2 реализован автоматизированный Core Audio path. В 7.3A готовы active-window state, bounded catalog и focus/activate actions. Ручные Windows 10/11 проверки ещё открыты. Exact launch/open остаются `unavailable`: [ADR-0021](../decisions/0021-brokered-user-process-launch.md) запрещает ослаблять plugin Job Object и требует узкий Host broker. Каждый следующий срез должен оставлять `npm run check` и workspace tests зелёными и не объявлять fake/manual result production capability.

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

Evidence: `plugins/platform-windows/tests/package_shell.rs` собирает временный пакет из реальных binaries, индексирует его общим `PackageStager`, запускает обе группы через `ProcessRuntime` и проверяет scoped revoke через `RuntimeController`. `scripts/check-schemas.mjs` валидирует manifest template и все объявленные package schemas. Это проверка lifecycle и границ, а не доказательство готовности audio/window/media/input adapters.

Повторная проверка runtime: добавлены периодический heartbeat, запрет вызовов до activation commit и для чужой generation/epoch, последовательная публикация начального и текущего состояния, отказ от просроченных команд в worker queue. Переполнение audio callback queue принудительно запрашивает rebind, поскольку потерянным событием могло быть переключение default endpoint. Process test проверяет idle heartbeat без утечки состояния до commit; worker tests — отказ до обращения к OS adapter.

## 7.2 — Core Audio vertical slice

**Статус:** [~] Реализация и автоматические tests готовы; ручная Windows 10/11 матрица и hardware default-switch evidence остаются открыты.

- [x] Выделить COM MTA worker, который владеет `IMMDeviceEnumerator`, текущим render/multimedia endpoint и `IAudioEndpointVolume`.
- [x] Реализовать `set_volume`, `change_volume`, `set_mute`, `toggle_mute`; значения clamp/validate выполняются в package application layer до FFI.
- [x] Подключить `IAudioEndpointVolumeCallback` и `IMMNotificationClient`. Callback складывает минимальное событие в bounded channel; rebind/default-device resolution выполняет worker.
- [x] Публиковать snapshot `{endpointRef, scalar, muted, availability, revision}` после activation commit. EndpointRef содержит SHA-256 от endpoint ID, не раскрывает raw COM identity и меняется при default-device switch.
- [x] Собственный context GUID помечает изменения DeckWeave; command success возвращается только после повторного чтения подтверждённого состояния. Успешный set с неудавшимся подтверждением возвращает `unknown_outcome`.
- [x] Добавить четыре простых action forms и RU/EN labels в статический automation catalog; UI не требует ручного ввода tagged JSON command.

Acceptance:

- [x] unit: command validation/clamp, HRESULT → stable failure, подтверждение после set и `unknown_outcome` при потере подтверждения;
- [~] adapter: bounded callback burst и default-switch rebind intent проверены; no-endpoint state проходит production process path. Endpoint removal во время команды и revoke во время фактического rebind требуют fault-injection/manual evidence;
- [ ] manual Windows 10/11: set/read round-trip, external volume/mute change и реальная смена default endpoint;
- [x] действие не объявляется успешным только потому, что command был поставлен в очередь.

Автоматический process test намеренно только читает текущее Core Audio state и не меняет громкость машины разработчика. Запись проверяется fake backend-ом; ручной round-trip должен восстановить исходное состояние и фиксироваться отдельно, а не запускаться неявно в CI.

## 7.3 — Active window, catalog, focus и launch

**Статус:** [~] 7.3A window state/catalog/focus реализован и автоматически проверен; manual focus/elevated matrix, icons и brokered launch/open остаются открыты.

- [x] Dedicated message thread устанавливает out-of-context foreground hook; callback только coalesces событие в bounded channel, Win32 metadata читает worker.
- [x] Worker строит `ActiveWindowState`: session-scoped hashed WindowRef, PID/session/process-start evidence, executable selector, bounded title metadata и freshness. Raw HWND не проходит wire/profile boundary.
- [x] Bounded catalog различает app identity и конкретные windows, возвращает generation/revision и страницы до 10 из максимум 256 окон, поэтому укладывается в negotiated message limit. Недоступный/elevated executable остаётся `null`, но окно не исчезает из списка.
- [x] `activate_window` повторно перечисляет окна, проверяет generation/ref и фактический foreground. `focus_application` использует переносимый executable + optional title selector. Foreground denial является typed terminal failure, не success.
- [ ] Добавить paged/icon query; icon loading остаётся bounded optional cache и не задерживает foreground callback.
- [x] Реализовать Host broker из [ADR-0021](../decisions/0021-brokered-user-process-launch.md), затем включить `launch_process` с exact executable + argv + cwd policy.
- [x] Реализовать отдельный `open_target` на STA Shell adapter через тот же broker lifecycle. Готовность окна остаётся отдельным predicate, не частью launch receipt.

Acceptance:

- [x] unit: два окна одного executable, close/reuse invalidates old ref, missing executable evidence и activation denial;
- [x] production process: committed runtime публикует active-window state и возвращает schema-validated window catalog без focus side effect;
- [ ] manual: close между list/click, реальный handle reuse, elevated target, empty title, external foreground switch и focus denial;
- [x] broker: Unicode executable/argv/cwd и пробелы никогда не проходят через shell concatenation;
- [~] app/window state идёт через существующий automation signal path; virtual feedback E2E остаётся в production composition 7.6.

## 7.4 — Media transport и origin

**Статус:** media transport реализован; автоматические проверки полного/нулевого/частичного SendInput выполняются на controlled backend без изменения проигрывания на машине разработчика. Результат success означает постановку пары в системный поток ввода; подтверждение состояния проигрывателя требует отдельного provider.

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

**Статус:** реализован production message-only Raw Input source, bounded bridge и нормализатор. Статический каталог обычных scan-code клавиш позволяет назначать действия в редакторе; E1/Pause, consumer/media и неизвестные vendor scan codes пока не объявлены. Layout отражает эталонные позиции ANSI-клавиатуры, а не обещание физической геометрии конкретного устройства.

Проверены: down/repeat/up, aggregate refcount двух источников, отключение с единственным release, neutral rearm удержанных при старте клавиш, фильтрация media emission, запуск и остановка настоящего message loop через process lifecycle. Разрыв очереди и keyboard overrun завершают runtime с fault, не оставляют частично достоверный поток. Физический pass-through и reconnect двух клавиатур остаются в ручной матрице.

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
