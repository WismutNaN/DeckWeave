# Режимы работы DeckWeave

Режимы являются независимыми измерениями. Например, portable distribution может одновременно работать с applied workspace, observer input plugin и active N4 Pro provider. Никакого глобального переключателя «простой/экспертный» нет.

## Режимы запуска

| Режим | Поведение | Ограничения |
|---|---|---|
| Installed desktop | Один per-user host, tray, optional autostart, data root в пользовательском каталоге | Не является system service; работает только в вошедшей пользовательской сессии |
| Portable desktop | Тот же host и UI, data root выбирается рядом с приложением или пользователем, autostart выключен | OS credential store может сделать secrets непереносимыми |
| Headless host, будущее | Application core без WebView, управление CLI/API | Не является plugin; требует отдельного packaging и lifecycle ADR |
| Remote agent, будущее | Подчинённый node объявляет capabilities основному host | Не хранит master workspace и не принимает доступ без pairing |

В первой версии процесс DeckWeave остаётся жив после закрытия окна: закрывается или скрывается WebView, но tray host, plugin supervisor и active workspace продолжают работу.

## Состояния workspace

```text
Stored revision ──open──> Draft ──validate──> Validated draft
      ▲                         │                    │
      │                         └──undo/redo─────────┘
      │                                              │ apply plan
      └──────────── snapshot/rollback ◀── Applied / Degraded
```

| Состояние | Значение |
|---|---|
| Draft | Редактируемая копия; hardware и automation ещё не изменены |
| Validated draft | Schema, references, permissions и graph constraints проверены |
| Applying | Host сохраняет candidate и готовит diff; barrier/handoff может временно приостановить затронутые routes |
| Applied | Runtime подтверждает новую immutable revision |
| Degraded | Выбрана одна целая revision; её unavailable routes остановлены по объявленной policy, причины видимы |
| Rejected | Commit не выполнен; выбранной остаётся прежняя revision, возможный failed handoff отражён в health |

Undo/redo действует внутри draft. Named snapshot ссылается на immutable revision и позволяет явный rollback.

Commit/recovery и судьба pending actions определены в [RUNTIME_CONTRACTS.md](RUNTIME_CONTRACTS.md#сохранение-и-apply). После durable commit нельзя вернуть `Rejected` или молча восстановить старый graph; rollback является новым apply.

## Уровни настройки элемента

Это progressive disclosure одной модели, а не два режима продукта:

1. **Быстрое назначение:** перетащить action на элемент, выбрать preset, применить.
2. **Inspector:** изменить параметры action, appearance, trigger и feedback.
3. **Advanced:** открыть созданный subgraph, добавить conditions, branches, state, timing и error policy.
4. **Raw diagnostics:** просмотреть descriptors и envelopes без возможности обходить validation.

Переход к advanced не создаёт вторую копию настройки. Простое назначение всегда является сворачиваемым subgraph.

## Режимы input

| Режим | Семантика | Permission |
|---|---|---|
| Observe | Получать события, не препятствуя ОС и исходному приложению | `input.observe` |
| Intercept | Получать событие до target и решать pass/suppress/replace | `input.intercept`, возможно privileged component |
| Emit | Создавать виртуальный keyboard/mouse/gamepad output | `input.emit` |
| Exclusive device | Владеть специализированным HID/interface, если это безопасно и возможно | Scoped `device.open.exclusive` |
| Provision | Менять persistent memory/configuration через typed plan и readback | `device.provision` |

Observe не обещает возможность suppress. Emit обязан маркировать synthetic origin, чтобы default policy предотвращала self-loop.

## Режимы plugin runtime

| Runtime class | Процесс | Назначение |
|---|---|---|
| Isolated control | Один дочерний процесс на активную runtime group package | Device, action, state, automation extensions |
| OpenAction compatibility | Отдельное process tree | Legacy plugins и property inspectors |
| Widget | Lazy optional renderer process | Custom web widgets вне desktop WebView |
| Trusted media | Специализированный process с real-time threads | Audio/video sources, filters и sinks |
| Remote | Отдельный paired node | Android, Steam Deck, другой ПК |
| Built-in audited | Внутри host, минимальный набор | Clock, locks, data roots, credentials, autostart, process cleanup; fixtures только в test adapter |

Несколько contributions одного package разделяют runtime, но имеют отдельные instances, settings, scopes и health.

## Режимы UI

| Режим центральной области | Основная задача |
|---|---|
| Workspace | Infinite canvas всех devices и быстрые назначения |
| Graph | Сложные связи, conditions, layers, branches и feedback |
| Surface | Независимый scene graph, hit regions и preview |
| Device Studio | Discovery, calibration, protocol traces и descriptor authoring |
| Runtime | Sessions, permissions, latency, queues, conflicts и logs |
| Plugin Library | Установка, compatibility report, update и contributions |

## Режимы визуального обновления

- **Event-driven:** default; scene пересобирается только при изменении связанного state.
- **Timeline:** ограниченная animation с заявленным FPS и duration.
- **Continuous:** разрешается только для plugin/widget с явным resource budget.
- **Preview:** временный output lease с deadline; stop/expiry возвращает текущую applied scene при доступном sink, disconnect даёт явный unavailable report.

## Режимы ошибок automation

| Политика | Поведение |
|---|---|
| Stop branch | Остановить текущую ветвь |
| Continue | Записать failure и продолжить независимые шаги |
| Retry | Повторить с bounded attempts/backoff/deadline |
| Fallback | Выполнить явно заданную резервную ветвь |
| Compensate | Выполнить обратные actions, если они определены |

Global «игнорировать все ошибки» отсутствует. Policy задаётся на group/node и видна в graph.

## Режимы remote и media

- Pairing не равен capture permission.
- Control events и media streams используют разные channels и QoS.
- Widget panel является первым remote display use case.
- Window streaming и virtual monitor являются отдельными capabilities.
- Media graph имеет `stopped`, `running`, `bypassed` и `degraded`; bypass доступен только при корректной совместимости форматов.
