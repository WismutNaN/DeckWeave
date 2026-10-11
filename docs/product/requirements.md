# Требования DeckWeave

Документ является проверяемым контрактом продукта. Идентификаторы используются в плане, ADR, тестах и issue tracker. Архитектурно предусмотренная функция не считается реализованной, пока её acceptance criteria не пройдены.

## Приоритеты

- **P0** — обязательное основание первой ежедневной версии.
- **P1** — обязательное расширение после первой N4 Pro вертикали.
- **P2** — архитектурно предусмотрено, реализация после стабилизации v1.

Все P0 должны быть закрыты к milestone **M2 после Фазы 11**. Фаза 9 даёт N4 Pro preview, но ещё не подтверждает готовность layers/parallel/error policies. P1 закрываются к v1; P2 не входят в release gate. Статус каждого требования до появления implementation evidence — не реализовано.

## Функциональные требования

### Рабочее пространство и устройства

| ID | Приоритет | Требование | Критерий приёмки |
|---|---:|---|---|
| DW-WS-001 | P0 | Workspace объединяет несколько физических, виртуальных, программных и удалённых nodes | В одном workspace событие virtual device меняет output другого provider без изменения core |
| DW-WS-002 | P0 | Каждый device описывается elements, capabilities и типизированными ports | Неизвестный namespaced port сохраняется и виден в диагностике |
| DW-WS-003 | P0 | Физическая identity отделена от session и OS path | Reconnect создаёт новую session, но восстанавливает bindings |
| DW-WS-004 | P0 | UI показывает цифровое представление всех устройств на infinite canvas | Device можно выбрать, переместить и открыть inspector |
| DW-WS-005 | P1 | Несколько устройств можно группировать как логический unit без потери физической identity | Group получает layout и routes, а disconnect одного участника даёт degraded state |
| DW-WS-006 | P1 | Fake keyboard, mouse, gamepad и test surface доступны simulation/replay без регистрации OS virtual devices | Recorded fixture воспроизводится без hardware и live input emission |
| DW-WS-007 | P0 | Capability templates, dynamic instances и descriptor revisions поддерживают разные классы устройств без device-name special cases | Независимые keyboard/analog/RGB/surface/job/media descriptors валидируются общим compiler; удалённый port не переназначает binding |

### Назначения и автоматизация

| ID | Приоритет | Требование | Критерий приёмки |
|---|---:|---|---|
| DW-AU-001 | P0 | Простая связь source → action создаётся через drag-and-drop или форму | Частый сценарий занимает не более трёх пользовательских действий |
| DW-AU-002 | P0 | Один event может запускать несколько actions последовательно или параллельно | Порядок и join policy воспроизводимы в simulation |
| DW-AU-003 | P0 | Conditions, state, variables, timers и temporary layers являются штатными nodes | Удержание клавиши временно меняет назначение N4 Pro и возвращает его после release |
| DW-AU-004 | P0 | Ошибки имеют настраиваемые stop/continue/retry/fallback policies | Ошибка подсвечивает node и не скрывается успешной соседней ветвью |
| DW-AU-005 | P1 | Поддерживаются schedules, cycles с положительным delay/ожиданием нового внешнего события и Pomodoro-like timers | Цикл без временной границы не компилируется; execution/rate budgets ограничены |
| DW-AU-006 | P1 | Active application/window может участвовать в condition и layer selection | Один control имеет разные bindings в Word и VS Code |
| DW-AU-007 | P1 | Events записываются и воспроизводятся с monotonic timestamps и fake clock | Один fixture даёт одинаковый итоговый state в повторных тестах |
| DW-AU-008 | P1 | Конфликтующие owners/routes видимы, а пользователь задаёт priority | UI показывает конкурирующие rules до apply |
| DW-AU-009 | P1 | Python plugin может реализовать сложное condition/action в отдельном процессе | Timeout Python runtime не блокирует event engine |
| DW-AU-010 | P0 | Plan epochs, ordering и cleanup предотвращают stale effects и stuck outputs | Late completion после apply не запускает action; release/disconnect/overflow очищают held outputs; encoder deltas не теряются |
| DW-AU-011 | P0 | Action объявляет input/result/error schema; sequence получает результат конкретного invocation | Два run не смешивают результаты; accepted без terminal success не продолжает цепочку; unknown не повторяется вслепую |
| DW-AU-012 | P1 | Долгие jobs поддерживают progress/cancel, bounded artifacts и run-local data | Cancel/TTL/quota/revoke/restart fixtures проходят; WAV/текст не попадают в profile assets и default logs |
| DW-AU-013 | P1 | Коллекции имеют bounded pagination, item identity, freshness и gap recovery отдельно от state | Fake notification feed обрабатывает edit/delete, expired cursor и reconnect без бесконечной очереди |
| DW-AU-014 | P2 | Отдельные packages реализуют Handy/capture/browser и mail/Telegram workflows | Реальный version-pinned end-to-end тест подтверждает result correlation, API limits и отсутствие смешивания private data |

### Surface, feedback и N4 Pro

| ID | Приоритет | Требование | Критерий приёмки |
|---|---:|---|---|
| DW-SF-001 | P0 | Визуальная surface является непрерывным canvas, не сеткой legacy controls | Изображение заполняет N4 Pro display без искусственных чёрных промежутков |
| DW-SF-002 | P0 | Scene objects и hit regions редактируются независимо | Текст пересекает границы template cells, а четыре touch regions остаются активными |
| DW-SF-003 | P0 | Scene поддерживает text, image, icon, shape, group, clipping и state bindings | Mute/recording feedback меняет внешний вид без пересоздания workspace |
| DW-SF-004 | P0 | N4 Pro plugin предоставляет десять keys, четыре encoders/press, touch coordinates, full display, light и haptic capabilities согласно реально подтверждённому протоколу | Hardware conformance fixture и ручной smoke проходят |
| DW-SF-005 | P0 | Свет и поддержанные haptic modes управляются раздельно по доступным hardware units | Доступные ring colors/brightness и built-in vibration enable независимы от image; pattern доступен лишь при подтверждённом actuator playback |
| DW-SF-006 | P1 | Renderer использует dirty regions/latest-frame-wins и device FPS budget | Медленный sink не создаёт неограниченную очередь |
| DW-SF-007 | P1 | Custom widgets могут поставляться plugin package и обновляться по событиям | Widget crash не останавливает surface compositor |
| DW-SF-008 | P1 | Анимация поддерживается как ограниченный timeline, но статические event-driven updates являются default | Неактивная animation не потребляет frame loop |
| DW-SF-009 | P1 | Lighting имеет versioned geometry, coordinate frames и many-to-many key/LED mapping | Virtual irregular keyboard с missing/shared LEDs корректно показывает spatial wave и unsupported geometry |
| DW-SF-010 | P1 | Reactive dim/cooldown/fade и слои компонует единый output owner | Fake clock/retrigger/frame drop/layer switch/preview fixtures воспроизводимы; animation не пишет persistent memory |

### Плагины и совместимость

| ID | Приоритет | Требование | Критерий приёмки |
|---|---:|---|---|
| DW-PL-001 | P0 | Manifest индексируется без запуска plugin executable | Каталог contributions доступен при нуле активных сторонних runtimes |
| DW-PL-002 | P0 | Каждая активная runtime group package работает в отдельном процессе и может предоставлять несколько contributions | Crash group не завершает host, affected ports unavailable, остальные groups продолжают работу |
| DW-PL-003 | P0 | Protocol поддерживает Rust и TypeScript SDK; Python SDK следует после стабилизации | Reference plugins проходят общий conformance suite |
| DW-PL-004 | P0 | Settings задаются schema-form или sandboxed custom property editor | Invalid settings не достигают active revision |
| DW-PL-005 | P0 | Permissions показываются до запуска, scope сохраняется отдельно от profile secrets | Новое permission после update требует повторного подтверждения |
| DW-PL-006 | P1 | Central catalogue хранит metadata и ссылки на immutable GitHub release artifacts | Установка проверяет hash, а update всегда подтверждается пользователем |
| DW-PL-007 | P0 | Handshake согласует обязательные features/schemas/limits, а не только package version | Unsupported required feature блокирует route/group с report; limits не превышают host policy; independent contribution сохраняет совместимость |
| DW-CP-001 | P1 | OpenAction/Elgato compatibility host запускает проверенное подмножество существующих plugins без изменения их исходников | Reference packages достигают документированных C0–C3 levels |
| DW-CP-002 | P2 | Bitfocus Companion bridge подключает modules/variables/actions через публичные или изолированные compatibility interfaces | Companion version несовместимость не ломает native plugins |
| DW-CP-003 | P1 | Нативный replacement plugin может объявить migration from legacy action | Пользователь видит settings diff и может rollback |

### Системный ввод и hardware provisioning

| ID | Приоритет | Требование | Критерий приёмки |
|---|---:|---|---|
| DW-IN-001 | P0 | OS plugin предоставляет media keys, volume/mute, application launch и active-window state | Базовые действия работают на Windows 10/11 |
| DW-IN-002 | P1 | Input modes различают observe, intercept/suppress и emit | Permission и diagnostics явно показывают фактически доступный mode |
| DW-IN-003 | P1 | Injected events маркируются для предотвращения self-loop | Emitted key не повторно запускает исходное rule без явного opt-in |
| DW-IN-004 | P2 | Selective keyboard/mouse replacement использует optional privileged platform component | Отключение компонента возвращает безопасный системный ввод |
| DW-IN-005 | P2 | Device memory/configuration изменяется только через отдельный typed provisioning plan с readback | V1 объявляет read-only descriptor и отклоняет write; реализация записи начинается отдельной post-v1 программой |
| DW-IN-006 | P0 | Firmware flashing находится вне области DeckWeave | Ни один standard permission не позволяет firmware update |

### API, профили и эксплуатация

| ID | Приоритет | Требование | Критерий приёмки |
|---|---:|---|---|
| DW-API-001 | P0 | Все операции UI используют общий application service contract | UI не обращается напрямую к plugin process или profile files |
| DW-API-002 | P0 | Mutations имеют command-specific preconditions, durable dedup, validate/dry-run и логически atomic apply | Apply проверяет draft и active revision; повтор после lost response/restart не исполняет mutation второй раз |
| DW-API-003 | P1 | CLI и Codex используют локальный authenticated API; MCP — optional adapter | MCP можно удалить без потери core API |
| DW-API-004 | P0 | Authorization проверяется ApplicationService для всех adapters; grants отделены от profiles | Read/draft principal не может apply/grant; revoke во время prepare блокирует dispatch; rollback не возвращает grant |
| DW-PR-001 | P0 | Draft имеет undo/redo; applied revisions неизменяемы; доступны named snapshots | До commit failure сохраняет прежнюю выбранную revision, после commit startup восстанавливает новую с явным health report |
| DW-PR-002 | P0 | Экспорт выбирает workspaces, plugins metadata, assets и private section | Экспорт без secrets не содержит credential values |
| DW-PR-003 | P1 | Cross-platform import сообщает unavailable contributions, не теряя их settings | Профиль Windows открывается на Linux как degraded, а не повреждается |
| DW-PR-004 | P0 | Revision фиксирует package versions/hashes, assets; import/migrations не запускают executable | Restart/rollback разрешает тот же artifact; unknown data сохраняются, unresolved secrets требуют local mapping |
| DW-OP-001 | P0 | Installed mode поддерживает автозапуск, tray и продолжение работы после закрытия окна | Закрытие WebView не прекращает actions |
| DW-OP-002 | P0 | Portable mode использует выбранный data root и не включает autostart сам | Две установки не смешивают профили |
| DW-OP-003 | P0 | Диагностика показывает sessions, latency, queue depth, dropped/coalesced values и last error | Причина stale/unavailable port доступна без raw log |
| DW-OP-004 | P0 | Один host на OS user session, один active workspace; roots защищены writer lock | Второй installed/portable запуск не открывает конкурентный Store/hardware owner |

### Будущие remote и media capabilities

| ID | Приоритет | Требование | Критерий приёмки |
|---|---:|---|---|
| DW-RM-001 | P2 | Remote agent объявляет display, touch, buttons, sensors, camera, microphone и speaker независимо | Pairing не активирует media capture автоматически |
| DW-RM-002 | P2 | Control transport не зависит от Wi-Fi или cable carrier | Один capability descriptor сохраняется при смене transport |
| DW-RM-003 | P2 | Первая роль Steam Deck/Android — widget panel с возвратом touch coordinates | Touch point достигает workspace с правильной transform |
| DW-MD-001 | P2 | Audio/video buffers не проходят через обычный plugin IPC или automation graph | Benchmark подтверждает отдельный data plane |
| DW-MD-002 | P2 | Media graph допускает sources, filters, sinks, bypass и virtual OS endpoints | Crash filter приводит к выбранной bypass/stop policy |
| DW-MD-003 | P2 | Control parameters и meters доступны обычному graph без блокировки media thread | Automation меняет gain через bounded control bridge |
| DW-MD-004 | P2 | Media имеет negotiated ports, buffer ownership, clock, reconfigure и failure contracts | Backend gate измеряет latency/copies/xruns; old-generation buffers отвергаются, bypass проверяется при объявленной crash boundary |

## Нефункциональные требования

| ID | Область | Требование |
|---|---|---|
| DW-NF-001 | Надёжность | Внешние данные не приводят к panic host; plugin crash локализуется |
| DW-NF-002 | Производительность | Digital input → dispatch p95 ≤ 10 ms на reference PC без внешнего сервиса; точный baseline фиксируется benchmark |
| DW-NF-003 | Память | Неиспользуемые runtimes не запускаются; тяжёлые widget/media hosts являются optional |
| DW-NF-004 | Backpressure | Все runtime queues bounded; edge events не coalesce, continuous values могут coalesce с метрикой |
| DW-NF-005 | Переносимость | Domain, protocol и profile crates не импортируют OS-specific APIs |
| DW-NF-006 | Локализация | UI и plugin schemas поддерживают минимум ru/en без идентификаторов, зависящих от перевода |
| DW-NF-007 | Наблюдаемость | Correlation/causation IDs проходят через event, action, feedback и diagnostics |
| DW-NF-008 | Восстановление | Запись профиля атомарна; startup может восстановить последнюю целую revision |
| DW-NF-009 | Безопасность | Secrets не попадают в profile, logs или default export |
| DW-NF-010 | Тестируемость | Clock, devices, transports и plugin processes имеют fakes/fixtures |
| DW-NF-011 | Доступность | Цвет не является единственным признаком состояния; все редакторы доступны с клавиатуры |
| DW-NF-012 | Публичный проект | Каждый merge оставляет build/test/docs в согласованном состоянии и указывает provenance заимствований |
| DW-NF-013 | Восстановление кадров | Diff имеет подтверждённую base; drop/timeout/reconnect вызывает full-frame resync |
| DW-NF-014 | Зависимости | Runtime не импортирует testkit, SDK не импортирует GPL domain/host; OS bootstrap не зависит от lazy plugin |

## Первый сквозной критерий

Первая ежедневная версия считается полезной, когда пользователь на Windows 10/11:

1. запускает DeckWeave в tray;
2. видит N4 Pro на canvas;
3. назначает кнопкам запуск приложения и media commands;
4. использует энкодер для volume и press для mute;
5. выводит произвольную композицию на полный touch display без сеточных промежутков;
6. связывает независимые hit regions с actions;
7. задаёт light и haptic feedback;
8. закрывает окно, продолжая использовать панель;
9. записывает тестовый event trace, изменяет draft и безопасно возвращается к snapshot.

N4 screen/touch/per-ring возможности проходят отдельный model/firmware gate: координатный sample не доказывает release/multitouch, а размер основного background не доказывает размер touch strip. Если hardware gate не подтверждает P0-возможность, M2 не объявляется достигнутым: требуется явное изменение требования/ADR, а не синтетический descriptor. [Сквозные сценарии](scenarios.md) расширяют критерии проверки контракта, сохраняя отдельные сроки конкретных integrations.
