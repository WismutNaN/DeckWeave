# Продуктивность, представления и проверочные плагины

Статус — проектирование. [Реестр](README.md), [automation](../contracts/automation.md), [presentation](../contracts/presentation.md). Эти идеи проверяют общие механизмы платформы; domain logic не привязывается к USB или размерам одного Mirabox.

<a id="plg-011"></a>
## PLG-011 — Pomodoro

Work/break presets (включая 5/10/15 минут), start/pause/resume/reset, cycles и configurable long break. Прикладная policy живёт в provider; scheduling/deadline time domain использует PLG-008/host. Elapsed event содержит timer/cycle/occurrence identity; state показывает phase и remaining, а не общую переменную-флаг, которую два consumer могут случайно сбросить.

Timer instance удерживает lease без открытого UI. Пользователь выбирает pause-on-sleep либо elapsed-during-sleep, а restart не воспроизводит пропущенные внешние actions автоматически. Приёмка: cancel на границе фазы, два timers, shift wall clock, sleep/restart и ровно один transition на occurrence. Показ времени переиспользует PLG-022, sound/light — обычные downstream actions.

<a id="plg-019"></a>
## PLG-019 — Заметки

Notes provider хранит несколько записей с IDs/revisions, предоставляет list/get/save/delete, search и changed events. Короткая edit form возвращает typed result; сохранённая заметка выводится на кнопке, участке экрана или постоянном always-on-top desktop view. Отображение живёт независимо от завершения edit invocation.

Host владеет persistent `DesktopViewSession`, placement/focus/click-through policy и закрытием; plugin предоставляет bounded data/model. View не получает прямой Tauri API. Notes storage отделён от workspace draft, runtime artifact и credential storage; export private content — отдельный opt-in. Приёмка: concurrent edit conflict, restart, hide/show, отсутствующий монитор, provider crash/revoke, screen lock/privacy и несохранённый текст. [Данные plugin](../contracts/plugin-composition.md#plugin-data), [view lifetime](../contracts/presentation.md#desktop-views).

<a id="plg-020"></a>
## PLG-020 — Пипетка и палитра

Button → host picker session → выбрать пиксель → typed color result → сохранить palette item → перекрасить назначенный visual/light target. Platform capture/sample contribution переиспользуется другими pickers, palette storage — заметками/другими коллекциями; Mirabox лишь один sink.

Result описывает RGBA, color space, источник/monitor и правило HDR→SDR. Sampling требует отдельного permission, доступность protected/secure content не обещана. Capture policy исключает собственный overlay при необходимости; координаты переводятся с учётом DPI. Приёмка: Escape, monitor change, закрытие target, late result старой session, нельзя захватить выбранный контент, неверный color format. Трасса по умолчанию содержит выбранный цвет, не снимок всего рабочего стола. Palette хранится до явного изменения, не до конца run.

<a id="plg-021"></a>
## PLG-021 — «Поймай бобра»

Game provider владеет game session/score/seed/difficulty, выдаёт bounded scene state и принимает hit(cell, round revision). Появление/исчезновение бобра управляется согласованным clock; общие presenter и input route работают и на virtual surface. USB и frame generation остаются provider/compositor boundary.

Проверяет timers, быстрые reliable edges, mixed input/render load, stale visual hit и cleanup. Нажатие учитывается по конкретной round/scene revision; запоздалое нажатие не засчитывается следующему бобру. Switch scene/disconnect/apply завершает game owner; score persistence — явная настройка. Replay фиксирует seed, tick/order и inputs. Измерять input→logic→frame enqueue→ACK/доступный display evidence отдельно, не обещать FPS по latency Фазы 5. Drop frames не теряет input и не даёт расти очередям.

<a id="plg-022"></a>
## PLG-022 — Общие представления и navigation

Переиспользуемые scene templates: text/icon/value, gauge, sparkline/chart, collection pages, selected item actions, back/home. Domain connectors возвращают data model и action descriptors; physical surface, rotation/scale и touch/key mapping адаптирует presenter. Built-in scene nodes не требуют отдельного web process; custom renderer нужен лишь для неподдержанного стандартными nodes представления.

Task list, windows, notes и Telegram используют одну collection navigation с item ID/revision. Slot привязывается к item и presented revision до обработки click; sort/update не меняет объект уже начатого жеста. State/freshness, units, paging, missing icon, loading/offline и destructive-action policy общие. Приёмка: список обновился между press/release, более одной страницы, два независимых displays, owner cleanup при смене scene. [Контракт](../contracts/presentation.md#collections-and-navigation).

Системный presenter используется также для unread badges/counters, health progress, countdown blocks и news. Общие typography/layout/accessibility и event-driven invalidation уменьшают число renderers. Host предоставляет стандартный rendering/window service; templates могут поставляться bundled package. Data connector не зависит от конкретного web/native backend и не запускает отдельный UI process для каждого виджета.

<a id="plg-023"></a>
## PLG-023 — Переключение режима другого устройства

Keyboard chord → выбрать заранее настроенную scene/layer group на Mirabox → запустить game или иной recipe. Это применение Automation и scoped presentation state, а не обязательный новый plugin и не hardware-specific RPC. Manual selection, временный hold и foreground-context selection имеют явные priorities и owner tokens.

Преднастроенные scenes/bindings находятся в одном applied plan, переключение выбирает активную группу без записи draft и полного workspace apply. Новые настройки/packages требуют обычный plan/apply. Временный hold возвращает прежнее состояние только своим token; release доставляется исходному owner. Приёмка: два controlling devices, повторный chord, process/keyboard disconnect, manual override, scene switch во время удержания и запуск/остановка PLG-021. Device firmware preset — отдельная provisioning операция PLG-029, не разновидность UI scene switch.

<a id="plg-043"></a>
## PLG-043 — Карусель виджетов одной кнопки

Clock/calendar → click → countdown → click → unread counter → обратно. Пользователь задаёт ordered widget set в одном stable slot (кнопка/область), trigger next/previous/select и optional timed rotation. Это runtime presentation state/recipe; соседние slots и общий экран не сбрасываются.

Slot содержит active widget instance, local state, owner/revision и input policy: click для переключения либо domain action, hold/secondary action при необходимости. Один press не должен сначала переключить виджет, затем вызвать action нового. Меняется только region/hit mapping с прежним owner release; hidden widgets освобождают ненужные rendering/subscription leases, общие источники продолжают работать для других consumers. Resume policy сохраняет/сбрасывает local page явно. Приёмка: два slots с разными наборами, hold и timer одновременно, apply/revoke, hidden unread source, renderer crash и независимость остальных controls. [Контракт slot switching](../contracts/presentation.md#slot-switching).
