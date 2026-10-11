# Контракты исполнения и восстановления

Документ задаёт нормативную семантику для Host, Store, Registry, Automation и Surface. [Архитектура](../architecture/overview.md) описывает компоненты, а здесь определены их совместные границы отказа. Контракты являются требованиями к будущей реализации; runtime и fault tests пока отсутствуют.

## Владение host и workspace

В v1 работает один host на OS user session и не более одного active workspace в нём. Хранить и редактировать можно несколько workspaces. Переключение workspace проходит тот же apply, что смена revision, и освобождает прежние output leases.

Host до открытия Store, API и hardware получает single-instance lock пользовательской сессии и writer lock канонического data root. Второй запуск с тем же root открывает существующий UI; запуск с другим root сообщает конфликт и требует явного завершения первого host. Installed и portable roots не смешиваются. Автоматическое вытеснение работающего host запрещено. Lock освобождается ОС после crash; наличие PID-файла само по себе не считается lock.

Эта политика не исключает сторонние программы: ошибку открытия занятого hardware provider возвращает как `resource_busy`. Одновременные host для нескольких roots потребуют отдельного ADR с арбитражем общих ресурсов.

## Разные виды версий

| Тип | Что идентифицирует | Где проверяется |
|---|---|---|
| `WorkspaceRevisionId` | Неизменяемый документ конфигурации с package lock | Store, draft base, apply |
| `DraftVersion` | Версия mutable draft, включая undo/redo | Каждая draft mutation |
| `HostBootId` | Один запуск host | Все runtime handles и subscription cursors |
| `ActivationEpoch` | Одна активация плана в данном запуске | Signals, effects, timers, continuations, frames |
| `PluginSessionId`, `DeviceSessionId` | Один handshake / подключение устройства | Registry и dispatcher |
| `RegistryRevision`, `GrantRevision` | Топология и актуальная авторизация | Validation/plan и повторная проверка перед effect |

Повторное применение того же документа создаёт новую `ActivationEpoch`. Пара `(HostBootId, ActivationEpoch)` никогда не восстанавливается из profile как действующий runtime handle. Revision не заменяет session generation и наоборот.

`PortRef` содержит namespace node/provider, stable device binding, element/capability/port IDs. Разрешённый runtime endpoint дополнительно содержит session IDs и descriptor version. Профиль хранит stable reference, а не разрешённый endpoint. При неоднозначной physical identity автоматическое переназначение output запрещено.

## Сохранение и apply

### Содержимое хранилища

Source of truth — versioned JSON documents и assets. `WorkspaceRevisionId` — уникальный ID, связанный с неизменяемыми bytes и checksum; hash содержимого не заменяет identity revision. Поэтому rollback может создать новую revision с прежним содержимым. Именованные snapshots и история сохраняют исходные ссылки.

Revision включает точные package versions, artifact hashes, contribution/settings schema versions и asset references. Install/update не меняет этот lock; переход версии создаёт новый draft и проходит apply. Активную версию и версии, нужные snapshots, нельзя удалить без явного отчёта о зависимостях. Импорт metadata никогда не скачивает и не запускает executable автоматически.

Grants, API principals, operation receipts и credential mappings хранятся отдельно от переносимой конфигурации. Rollback и import не восстанавливают отозванные grants. `CredentialRef` в импорте считается unresolved, пока пользователь не сопоставил его локальному credential; совпадение имени не даёт доступа к существующему secret.

Подтверждённая draft mutation сохраняет документ и receipt до ответа client. Undo/redo history может оставаться in-memory в v1: после restart восстанавливается последний сохранённый draft, а UI сообщает об утрате undo stack. Read-only или неподдерживаемый будущий schema major открывается без перезаписи.

### Порядок apply

1. Host фиксирует `DraftVersion`, base revision, package lock и разрешённую политику degraded/downtime. Validate/plan не запускают plugin code и не меняют hardware. Plan содержит hash, expiry, версии topology/grants и список затронутых routes/resources.
2. Store сохраняет immutable candidate revision, связанные assets и operation intent. Файлы пишутся во временное место на том же filesystem, проверяются и сбрасываются на носитель до публикации ссылок. Нехватка места или ошибка записи не переключает runtime.
3. Prepare запускает требуемые runtime groups, валидирует session descriptors и резервирует доступные resources. Активация не исполняет пользовательские actions, не отправляет frames/haptics и не применяет новые settings к работающему instance. Если provider не умеет prepare без изменения текущего instance, plan заранее объявляет disruptive handoff.
4. Для обычного apply прежний plan работает до короткого cutover barrier. При exclusive handoff одновременно открыть старую и новую версии provider нельзя: affected routes quiesce, старый lease освобождается, новый приобретается с deadline. Это требует `allow_disruption` в точном plan; без него apply отклоняется. Попытка восстановления прежнего provider может дать degraded, что явно отражается в report.
5. На barrier прекращается запуск новых effects, выполняется bounded drain/cancel старых executions и cleanup удерживаемых outputs. Host повторно проверяет expected active revision, draft version, grants, sessions и resource reservations. Вход во время barrier bounded; при overflow действует fault policy, а не скрытая потеря release.
6. Store атомарно публикует commit record: candidate revision, предыдущая revision, package lock reference, operation ID и commit status. **Это точка фиксации apply.** До неё failure оставляет прежнюю revision выбранной; после неё автоматического возврата указателя нет. Effect dispatcher остаётся закрытым до шага 7.
7. Host переключает plan, назначает новую activation epoch, инициализирует state и открывает dispatcher. Публикует `Applied` либо `Degraded` с перечнем unavailable routes. Cleanup старых ресурсов не превращает уже committed apply в `Rejected`.

Barrier разделяет ingress до и после cutover. Старые buffered edges не переадресуются новому mapping и не воспроизводятся после restart; cutover report содержит их число. На boundary host выполняет input resync/neutral rearm и cleanup независимо от доставки release. При abort до commit старый plan возобновляет новые triggers, но уже отменённые executions не запускаются заново. Если durable commit нельзя подтвердить или исключить после I/O error, dispatcher остаётся закрытым до чтения commit record/recovery — old plan не возобновляется наугад.

`ApplyReport` разделяет `not_committed`, `committed` и `recovery_required`, выбранную revision, текущую runtime health и последствия внешних действий. Потеря ответа client разрешается через operation query. Cancel после commit возвращает `too_late`, rollback является новым apply.

Состояния `Applied` и `Degraded` относятся к **одному целому плану**. Запрещено неявно смешивать graph nodes из старой и новой revision. Отсутствующий hardware допускается только по явно заданной availability policy; invalid schema, unknown required semantics и missing grant на обязательный route блокируют apply. Unavailable route не получает synthetic success.

### Crash recovery и граница гарантий

| Точка сбоя | Поведение следующего запуска |
|---|---|
| До публикации commit record | Восстановить предыдущую committed revision; недостижимый candidate удалить позднее |
| Во время публикации | Выбрать целый checksum-valid record по storage protocol; не угадывать по времени изменения файла |
| После commit, до переключения memory plan | Загрузить новую committed revision с новой epoch; отметить восстановление operation |
| После отправки внешней команды | Сохранить известный outcome или `unknown_outcome`; не повторять action при старте |
| Нельзя восстановить обязательный provider | Сохранить выбранную revision, показать degraded/stopped routes; предложить отдельный rollback |

Startup восстанавливает configuration и безопасные desired outputs, но не проигрывает button events, незавершённые macros или process launches. Таймеры и held-input state сбрасываются; persisted variables в будущем требуют явной schema/policy. Secrets/grants перепроверяются.

Atomic replace одного файла не является транзакцией нескольких документов. Store сначала сохраняет зависимости, затем один commit record. Backup/checksum и fault injection обязательны на Windows и Linux; конкретные replace/flush primitives выбираются storage adapter. Data root на network filesystem с неподтверждёнными locking/durability semantics не поддерживается. Portable на локальном removable volume допускается после проверки; power-loss durability зависит от filesystem/устройства и не обещается одной операцией rename.

Asset/package garbage collection идёт только по достижимости от active revision, сохранённых revisions/snapshots/drafts, pending operations и preview leases. Удаление истории — отдельная операция; GC не пересекается с публикацией commit без согласованного snapshot ссылок.

## Executions, cancellation и смена плана

Каждый execution, timer, command и continuation несёт `ExecutionId`, revision, activation epoch, session IDs и deadline. Completion старой epoch попадает в diagnostics/operation outcome, но не меняет state нового плана и не запускает следующую action.

`RunId` в workflow vocabulary означает тот же `ExecutionId` корневого запуска; отдельные шаги имеют `InvocationId`, а долгие provider operations — `JobId`. Typed results, branch scopes и transient artifacts определены в [automation contract](automation.md). Runtime artifacts отделены от persisted profile assets и не восстанавливают незавершённые effects после restart.

Default при apply — прекратить новые triggers, bounded drain/cancel текущих executions до commit, затем начать новый plan. Если обязательную операцию нельзя остановить в пределах deadline, apply отклоняется до commit; неизвестный результат явно сохраняется. Cancellation не означает отмену уже выполненного внешнего действия. Компенсация является отдельным объявленным action, также способным завершиться ошибкой.

State переносится только для совпадающих stable node IDs, state schemas и объявленной `preserve` policy. По умолчанию timers, tap/hold/chord и temporary layers сбрасываются. Cleanup отслеживает выходные key-down, contacts, haptic playback и temporary layers по owner lease. Release/cancel не зависит от текущего mapping, иначе смена слоя или revision может оставить клавишу зажатой.

Disconnect, grant revoke, queue fault и shutdown вызывают такой же bounded cleanup. После reconnect held source не создаёт новое нажатие: требуется neutral snapshot/release или явно разрешённая rearm policy. Если устройство не имеет stop/readback/watchdog, нельзя гарантировать физический safe state после обрыва — limitation отражается в descriptor и UI.

## Порядок сигналов и время

Provider нумерует envelopes в пределах `(PluginSessionId, DeviceSessionId, source stream)`; scope объявляется descriptor. Host добавляет monotonic `received_at`, `IngressOrdinal` и текущую activation epoch на ingress. Duplicate/late old-session message отбрасывается с метрикой; gap в edge stream инвалидирует достоверность held state и требует resync.

`captured_at` принадлежит clock domain источника. Для другого процесса/remote node его нельзя просто вычесть из host clock. Handshake объявляет `ClockDomainId`; remote clock mapping, если доступен, имеет uncertainty. Tap/hold и deadlines v1 работают по host monotonic time после ingress; source timestamps сохраняются для diagnostics. Нет обещания глобального физического порядка между устройствами. Wall clock используется только schedules с timezone/DST/missed-fire policy.

Scheduler сериализует transitions по ingress order, включая completions, timer firings и topology changes. `ValueOut` можно coalesce только в пределах сегмента до следующего edge/completion/timer/barrier, сохраняя последнее значение и sequence/drop metadata. Condition на button event не читает значение, пришедшее **после** этого button event. Реализация может использовать очереди/slots, но обязана сохранять эту семантику и bounded память.

### Edge, value и state

- `encoder.relative` и wheel delta — `EventOut`; latest-value-wins теряет движение. V1 сохраняет каждый signed delta в порядке поступления. Агрегация возможна позже только при явной algebraic policy, без перехода через binding/layer/barrier.
- Touch down/up/cancel — edge lifecycle; move координаты одного contact допускают coalescing между lifecycle edges. Последний snapshot контактов не заменяет release/cancel.
- `pointer.sample` без contact phases не создаёт held contact: никаких синтетических down/up без явно выбранного converter с degraded quality. Hold/drag требуют настоящего lifecycle.
- `axis.absolute` — latest-value. `StateOut` — наблюдение с freshness; переходы, которые должны вызывать action каждый раз, публикуются отдельным `EventOut`.
- Edge overflow вызывает fault source/routes, cleanup held outputs и resync. Fault/cleanup уведомление должно иметь зарезервированную capacity или отдельный флаг вне заполненной очереди.

### Детерминизм и циклы

Replay fixture содержит initial state/plan, порядок ingress, coalescing decisions, timer firings, external action outcomes, availability/grant changes и seed используемой случайности. Один такой fixture воспроизводит state и intents. Разный порядок внешних completion может дать другой результат; обещать одинаковый результат для произвольного reorder нельзя.

`sequence` ждёт semantic completion предыдущего шага и получает его typed result. `parallel` задаёт stable branch IDs и явный join (`all`, `first_success`); проигравшие ветви получают cancel, без обещания rollback effects. Конкурирующие записи в state/target требуют explicit priority/merge либо отклоняются compiler. Retry возможен для доказанного no-effect failure либо при объявленной provider idempotency; `unknown_outcome` не вызывает слепой повтор. Полные правила находятся в [automation contract](automation.md#композиция-графа).

State/latch сам по себе не разрешает бесконечный feedback. Любой цикл обязан пересекать положительный delay либо ожидание **нового внешнего** события. Дополнительно ограничены steps per activation, pending executions и effects/rate. Plugin condition/transform является async boundary с deadline и recorded outcome; он не исполняется внутри scheduler и не служит синхронным OS intercept decision.

## Resources, discovery и версии packages

`RuntimeLease` удерживает процесс, `ResourceLease` разрешает доступ к конкретному ресурсу — это разные сущности. Registry является единственным host arbiter. Canonical resource key включает node и physical interface/endpoint evidence, а не только `PortId` или package ID. Несколько providers одного physical interface конфликтуют независимо от contribution names. При неясном alias output требует явного выбора пользователя.

Один active runtime имеет ключ `(PackageId, artifact hash, runtime_group)`; внутри multiplexed instances. V1 разрешает одну маршрутизируемую active version package на host, конфликтующие version requirements обнаруживаются до apply. Подготовленный runtime новой версии не получает production routes/outputs до cutover. Старый artifact остаётся установленным для rollback, но не открывает exclusive hardware одновременно с новым. Updates проходят package-lock diff и применяются только после подтверждения пользователя.

Static indexing не запускает executable. Чтобы новое устройство появилось до первого binding, пользователь включает scoped discovery lease для установленного provider; first-party setup может предложить его явно. Пока lease включён, допустим bounded low-rate scan/watch и restart policy. После его выключения или закрытия разовой probe runtime может остановиться. Обнаруженный candidate не даёт разрешение на exclusive open или output. OS metadata watcher является host platform service и не содержит vendor protocol.

Prepare, diagnostics и Device Studio используют тот же resource arbiter. Нельзя открыть второй HID handle через probe в обход active lease. Безопасное совместное наблюдение требует объявленной shared capability.

## Frames и preview

`FramePacket` содержит session/epoch, scene revision, frame ID, dimensions/format и для diff — `base_frame_id`. Latest-frame-wins безопасен для полных кадров. Diff вычисляется относительно подтверждённого содержимого sink; нельзя выбросить промежуточный diff и послать следующий поверх неверной базы. При drop/timeout/reconnect/смене формата compositor объединяет изменения в полный актуальный кадр и сбрасывает base. Один кадр in-flight, один latest candidate; transport chunks дополнительно bounded.

Receipt различает `accepted` и подтверждение применения к framebuffer; первое не доказывает показ pixels. Если provider не может подтвердить базу, используются full frames. Frame bytes разрешены как bounded bulk output native control protocol; audio/video streams идут через отдельный data plane. Chunk scheduling не задерживает heartbeat/cancel/edge сверх control budget; при необходимости вводится negotiated bulk channel без изменения scene API.

Hardware preview — отдельный краткоживущий output lease с owner, target, epoch, TTL и stop. Его требует разрешённый runtime command, а не validate/simulate. Stop/disconnect/expiry снимает preview и отправляет **текущую** applied scene, а не сохранённый старый кадр. Apply отзывает конфликтующий preview; preview touch показывается в editor и не запускает production actions. Held contacts отменяются при смене сцены/epoch. Simulation/replay по умолчанию используют только fake targets.

## Миграции и неизвестные данные

Core migrations — host-owned pure transforms с backup/report. Package migrations в v1 — декларативные ограниченные transforms, исполняемые host без запуска package executable. Произвольный Rust/Python migration hook несовместим с обещанием pure import и не поддерживается. Нет подходящей migration — settings сохраняются opaque, contribution unavailable, пользователь выбирает совместимую версию или ручное редактирование.

Unknown optional JSON fields/settings сохраняются round-trip отдельно от typed views. Unknown required semantics, command variants и schema majors запрещают исполнение, но не удаляют данные. Для native wire optional metadata может быть пропущена endpoint; transparent relay сохраняет opaque payload bytes. Сохранение неизвестных Protobuf fields после decode/re-encode проверяется на выбранных Rust/TS/Python библиотеках и не предполагается автоматически, особенно при JSON conversion.

## Обязательные проверки реализации

| Контракт | Fault/acceptance fixture | Первая фаза |
|---|---|---|
| Host ownership | Два запуска с одинаковым и разными roots; crash владельца | 1 |
| Signals | Encoder +1,+1,-1; touch down/up под value flood; cross-source clocks | 2, 5 |
| Resource ownership | Два providers/probe одного interface; discovery до binding | 3 |
| Store/apply | Kill до/после каждой записи/commit/barrier, disk full, grant revoke во время prepare | 4, 5 |
| Epochs | Late completion после apply, held output при disconnect/layer switch | 5, 11 |
| Frames | Потеря diff/ACK, reconnect, preview expiry одновременно с apply | 9 |
| Replay | Recorded completions/coalescing; запрет live effects | 5, 10 |
| Package lock | Update/restart/rollback использует тот же hash, conflict двух versions | 12 |
| Migration | Unknown fields, missing migration, import без plugin execution | 4, 12 |

Решения закреплены в [ADR-0012](../decisions/0012-apply-and-execution-lifecycle.md); распределение зависимостей — в [ADR-0013](../decisions/0013-dependency-and-host-service-boundaries.md).
