# Контракт автоматизации и передачи результатов

Статус: проектируемый контракт. Минимальное исполнение проверяется в Фазе 5, композиция — в Фазе 11 [плана](../implementation/roadmap.md). Общие epochs, ordering и recovery определены в [lifecycle](lifecycle.md); этот документ определяет данные и семантику цепочек. Мотивировка: [исследование реализаций](../research/implementation-review.md).

## Разные виды данных

| Данные | Назначение | Время жизни |
|---|---|---|
| Signal / state | Нажатие, положение оси, текущее mute | Event или versioned snapshot с freshness |
| Invocation result | Текст транскрибации, созданная вкладка, receipt письма | Конкретный вызов внутри конкретного run |
| Interaction result | Выбранный пользователем item/form value либо explicit cancel | Конкретная interaction session и owner invocation |
| Runtime artifact | Запись WAV, большой текст, вложение | Ограниченный срок хранения и lease |
| Media stream | Непрерывный звук или видео | Negotiated stream session, отдельный data plane |

Результат action не публикуется в общую изменяемую переменную автоматически. Два одновременных нажатия получают разные `RunId` и не подменяют результаты друг друга. `DataEdge` между узлами workflow адресует поле результата конкретного invocation; это внутренняя связь графа, а не новый hardware port. `CommandIn` описывает вход и результат одной операции. `StateOut` остаётся наблюдением текущего состояния, а не почтовым ящиком ответов.

## Сборка входа действия

`actionArguments[nodeId]` отделяет событие запуска (`triggerPort`) от входа operation. Для record-input можно заполнить `fields`: каждое поле получает constant, trigger pointer, state pointer либо run-result pointer. Graph format **1.1** добавляет альтернативный `value` — binding целого входа, включая число, boolean, строку/enum, массив, nullable, record или null. `value` и непустой `fields` взаимоисключающие; отсутствие `value` сохраняет семантику 1.0. Старые readers отклоняют новый формат; compiler version 2 участвует в хеше плана.

Компилятор проверяет shape, диапазон, допустимые значения и единицы измерения до запуска. Constant задаётся в единицах целевой схемы. Передача целого значения между узлами требует совпадения единиц; извлечение части значения с единицей требует явного преобразователя. Flow-trigger содержит только null, а исходный payload доступен как результат source данного run. Ссылки на результаты будущих шагов запрещены. Scheduler удерживает необходимые результаты в пределах своего run независимо от способа сборки входа.

Редактор использует те же schema forms для scalar/enum/array и record inputs. Действие без параметров получает null и может следовать за шагом с результатом. Копирование, удаление и перестановка проверяют зависимости как в `fields`, так и в `value`; ссылки на событие копируемой кнопки переназначаются на новую кнопку.

## Descriptor операции

Каждый command/async transform объявляет:

| Поле | Смысл |
|---|---|
| `operation_id`, `version`, input/result/error `SchemaRef` | Стабильная операция и типизированный ответ; отсутствие результата задаётся unit schema |
| `execution` | Short request или job; поддержка progress, cancel и query/reconcile |
| `effects` | Read-only, external write, input emission, process execution и требуемые resource scopes |
| `idempotency` | None либо provider-defined key scope, retention и способ повторного получения outcome |
| `limits` | Deadline, input/output bytes, concurrent invocations, queue bound и rate budget |
| `resources` | Shared/exclusive leases и удержание ресурса до завершения/cleanup |
| `data_policy` | Чувствительность полей, внешние получатели, допустимость trace и retention |

Операция не получает права только потому, что появилась в graph. Host проверяет актуальные grants перед каждым dispatch и выдачей artifact. Имена schemas и операций namespaced; несовпадение версий требует явного converter. Scalar range/unit входит в schema; `missing`, `null`, `unavailable`, `failed` и пустая строка имеют разные значения. `unknown_outcome` не считается пустым успешным результатом.

Descriptor определяет completion predicate: например, процесс завершён и JSON разобран, вкладка создана либо legacy event доставлен. Последнее не является подтверждением внешнего действия. Compiler/UI не разрешают использовать delivery receipt как transcript или результат отправки письма без явного adapter с соответствующей семантикой.

## Invocation и job

`RunId` — workflow-название корневого `ExecutionId` из lifecycle, не второй независимый идентификатор. `InvocationId` уникален для `(HostBootId, ActivationEpoch, RunId, GraphNodeId, iteration, attempt)`. `GraphNodeId` не смешивается с capability `NodeId` среды исполнения. Запрос содержит resolved target session/descriptor revision, input, deadline, cancellation scope, trace и при необходимости provider idempotency key.

```text
created → dispatched → accepted → running → succeeded(result)
              │           │          ├── failed(error)
              │           │          ├── cancelled
              └───────────┴──────────└── unknown_outcome
```

Локальный delivery и ответ provider — разные факты. `rejected_before_send` гарантирует, что запрос не покинул Host; `enqueued` подтверждает только помещение в bounded локальную очередь; `possibly_sent` означает потерю уверенности после возможной передачи и обязательно несёт `unknown_outcome`. Успешный `ProcessRuntime::send` не является provider acceptance и тем более terminal result.

`accepted` подтверждает принятие работы, не завершение эффекта. Короткая операция может сразу вернуть terminal response. Долгая возвращает `JobId` и далее progress/terminal outcome с `InvocationId`; `JobId` также scoped к provider session, если descriptor не обещает durable reconciliation. Progress coalesce допустим; terminal outcome — нет. Host принимает один terminal outcome, дубликат того же результата игнорирует, противоречащий результат регистрирует как protocol fault. До доставки запрос можно отклонить с гарантией отсутствия эффекта; потеря связи после dispatch без подтверждения даёт неизвестный outcome.

Cancel — запрос остановить работу. `cancelled` означает подтверждённое завершение cancellation, но не отмену уже выполненных внешних действий; report содержит известные частичные effects. По deadline host прекращает continuation, запрашивает cancel и очищает собственные leases. Если provider не подтвердил исход, остаётся `unknown_outcome`. Поздний результат может уточнить audit record, но не запускает отменённую/старую цепочку. Длительный job удерживает runtime lease, имеет общий deadline и progress timeout policy; heartbeat процесса не заменяет progress.

После host restart незавершённые runs не возобновляются автоматически. Сохранённый redacted run journal позволяет показать interrupted/unknown и запросить reconcile у поддерживающего его provider. Это не durable workflow engine с exactly-once effects. Apply отменяет runs прежней epoch по общему lifecycle. Перенос живых continuations между revisions вне v1.

## Композиция графа

`GraphDocument` имеет version, исполняемый `GraphDefinition`, отдельный UI `GraphLayout` и явные extension maps. Исполняемые варианты v1 закрыты: source, action, state, condition и feedback. Неизвестный executable kind отклоняется; extension не может скрытно добавлять исполняемую семантику. Flow/data edge всегда ссылается на существующие output/input ports, а их `SchemaRef` совпадает точно до появления явного converter. Layout может ссылаться только на существующий node, но его координаты, collapse и viewport не участвуют в semantic compile/hash.

Host policy задаёт ненулевые limits document bytes, nodes, edges, ports per node, reliable queued events, pending runs/invocations, timers и trace entries. Документ не может увеличивать эти значения. Превышение возвращает structured limit kind, maximum и actual; executable/data payload не усекается для продолжения работы.

Semantic compiler использует immutable catalog schemas, static/cached endpoint descriptors, operation descriptors и package runtime requirements. Он не запускает plugin и не выбирает transient session. Результат — canonical DAG IR: порядок nodes/edges нормализован по stable IDs, source subscriptions и state slots индексированы, UI layout исключён. Runtime binding является отдельной операцией над фактическим descriptor snapshot; missing session возвращает `needs_discovery`, а schema/direction/unit/range/permission/resource/revision drift блокирует activation. Динамические grants и leases повторно проверяются непосредственно перед dispatch независимо от успешного compile.

| Узел или политика | Нормативное поведение |
|---|---|
| Sequence | Следующий шаг получает validated result предыдущего terminal success; ожидание acceptance недостаточно |
| Condition / transform | Встроенные pure expressions имеют пределы сложности/размера; plugin expression — async invocation с deadline |
| Branch | Невыполненная ветвь имеет `skipped`, а не фиктивный результат; downstream явно обрабатывает optional value |
| Parallel / join-all | Branch-local immutable results; join выдаёт record по стабильным branch IDs после terminal outcomes всех ветвей |
| Fail-fast | Останавливает дальнейший dispatch, запрашивает cancel остальных; уже совершённые effects остаются в report |
| First-success | Выбор победителя по записанному ingress order; проигравшие отменяются; применимость к side effects требует явной политики |
| Retry | Не входит в текущую пользовательскую модель: ошибка не запускает action автоматически повторно. Provider idempotency поддерживает reconciliation исходного вызова, а не включает retry policy |
| For-each | Snapshot входной коллекции, item scope, limit элементов и concurrency; результаты упорядочены по input index |
| Reentrancy | У binding явно `reject`, bounded `queue`, `restart` или bounded `parallel`; default — reject while running |
| Variables | Run-local default; shared workspace state меняется scheduler command с revision/merge policy, не конкурентной записью из plugin |

`restart` сначала отзывает право continuation старого run. Если требуется exclusive output, новая попытка ждёт подтверждённого cleanup/resource release; cancel не разрешает два владельца. Loops подчиняются временному барьеру и execution budget из lifecycle. Error policies `continue`/`fallback` передают tagged error/result, не превращают failure в success. Compensation — отдельный явно заданный action и тоже может завершиться ошибкой.

Случайность, current time и внешние ответы при replay берутся из fixture. Ветвь без recorded result получает `inconclusive`. Simulation никогда не читает реальные уведомления, clipboard или микрофон и не запускает код/браузер.

<a id="structured-bindings"></a>
## Структурированные bindings и вызов action

Целевое расширение после M1: input binding адресует constant, field конкретного invocation result либо versioned state snapshot. Field path проверяется относительно versioned schema; required/optional/missing/null различаются. Host проверяет структуру records/arrays, nesting/bytes, enum/range и совпадение wire schema/encoding, а не только JSON parse или общий ValueKind. Конкретный поддерживаемый subset schema фиксируется conformance fixtures; unknown required constraint нельзя игнорировать.

Action имеет явный trigger и набор именованных inputs. Изменение data input само не запускает внешний effect, если это не заявленный trigger. На trigger фиксируется согласованный run-local snapshot arguments; недостающие обязательные значения дают diagnostic/error, не null placeholder. Два data edges не означают два invocation. Projection/record construction остаются bounded pure operations; plugin transform — async command. Reentrancy/joins и остальная композиция по-прежнему следуют правилам выше.

M1 routing ещё не реализует эту общую argument model; [pre-6 gate](../implementation/pre-phase-6.md) требует minimal typed binding до inspector UI. Domain connectors не должны собирать shell/HTTP strings из непроверенного текста вместо общего schema contract.

<a id="time-and-schedules"></a>
## Время, таймеры и schedules

Host clock и bounded timer queue — plumbing, доступная без установки plugin. Все invocation deadlines исполняются live owner даже при отсутствии новых input events. Wire time domain/единицы и пересчёт remaining budget согласованы; timestamp чужого процесса или прежнего HostBootId нельзя сравнивать как локальный monotonic tick. Cancellation/deadline/timer arrival упорядочены в том же ingress owner.

Interval/countdown использует monotonic elapsed, calendar schedule — wall time + timezone с явной DST/clock-change policy. Sleep behavior (`pause` либо `elapsed`) выбирается отдельно от restart behavior. Timer occurrence имеет ID: coalescible remaining/state не заменяет reliable elapsed event. После suspend не выдаётся burst пропущенных ticks; выбранная catch-up policy ограничена и не повторяет внешние effects автоматически.

Restart не переносит произвольную workflow continuation. Durable timer checkpoint — прикладные данные с version/reconciliation policy; показывать missed/interrupted допустимо, автоматически повторять действие без явной политики нельзя. Time/date formatting принадлежит provider/presenter, Pomodoro work/break policy — plugin/recipe. Game randomness и timer ordering записываются в bounded replay fixture. Real-time media clock остаётся в [media plane](media.md).

## Interaction sessions

Interaction node приостанавливает только свою continuation и создаёт request через application port Interaction Runtime. Request содержит `InteractionSessionId`, owner `InvocationId`, view kind/schema, initial query/state, result schema, deadline, cancel policy и требуемые provider/grants. Окно и пользовательский lifecycle принадлежат host; provider поставляет данные страниц, validation или custom sandboxed representation.

Terminal outcomes:

- `selected(result)` — значение прошло result schema и принадлежит этой session;
- `cancelled(reason)` — пользователь закрыл окно/нажал Escape либо policy отменила session;
- `failed(error)` — provider/view/session fault;
- `timed_out` — deadline истёк без terminal selection;
- `unknown_outcome` не применяется к локальному выбору: host либо зафиксировал selection, либо нет. Следующий внешний action уже имеет собственную outcome semantics.

Только `selected` продолжает обычную success-ветвь. Остальные исходы обрабатываются явной error/cancel policy. Повторная terminal message игнорируется и диагностируется; результат другой session отклоняется. Apply/restart/revoke отменяет старые sessions по epoch. Query pages bounded, cursor scoped к provider generation/session; пользовательский текст и selection не попадают в profile/default log.

Schema-native list/search/form/confirm доступен без custom runtime. Custom view не получает прямой application API и общается через schema-validated session bridge. Replay использует записанный synthetic result либо `inconclusive`, но не открывает реальное окно.

## Artifact и доступ к данным

`RuntimeArtifactRef` содержит opaque ID, schema/media type, size, digest, owner run, sensitivity и expiration. Это не filesystem path, URL или право доступа само по себе. Artifact broker в Host Runtime выдаёт ограниченный read lease конкретному consumer с проверкой grants; transport — bounded chunks или scoped OS handle/private temporary file для подходящего adapter. Consumers не получают произвольный доступ к data root. Provider не может вернуть путь и заставить host прочитать его вне granted scope.

Transient artifacts хранятся отдельно от profile assets, не входят в revisions, snapshots и default export. Run/consumer leases удерживают данные до deadline, затем GC удаляет их по retention policy; persist/promote является отдельным пользовательским действием. Secret fields не попадают в run journal, stdout diagnostics или trace. Для replay чувствительные результаты заменяются synthetic fixtures; raw recording — отдельное разрешённое действие. Превышение квоты или expired artifact — structured error, без молчаливого усечения текста.

В v1 broker — небольшой host service с injected storage port, не новый распределённый storage engine. Ограничения total bytes, bytes per run и числа leases задаёт host; negotiation может их только уменьшать. При crash выполняется startup GC незавершённых временных artifacts.

## Уведомления и коллекции

Отображение коллекции, выбранный item/action, подменю и согласование render с нажатием определяются [presentation contract](presentation.md#collections-and-navigation). Typed query не подразумевает наличие transactional snapshot во внешнем API; adapter сообщает actual consistency. Persistent notes/palettes — [plugin data](plugin-composition.md#plugin-data), а не бесконечная очередь событий.

Telegram-подобный provider объявляет три независимые возможности:

1. `EventOut` новых/изменённых/удалённых записей с item ID и revision.
2. `StateOut` connection/unread/retention summary с freshness.
3. `CommandIn` read-only query для bounded page результатов с cursor, snapshot/watermark и `has_more`; acknowledgement — отдельная write operation.

Cursor непрозрачен и scoped к account/filter/provider generation; истечение требует нового snapshot. При разрыве sequence UI показывает gap и выполняет bounded resync. Item IDs дедуплицируются; edit/delete обновляют существующий item. History принадлежит сервису/provider cache, а не бесконечной очереди automation. «Все уведомления» означает доступную историю в объявленных account/retention границах; provider обязан показать ограничения API. Render карточки не помечает сообщение прочитанным автоматически.

Общий unread aggregator принимает normalized feed contracts, не реализует все API внутри core. Count содержит unit (`messages`/`threads`/`chats`/`mentions`), account/filter scope, observed time и exact/partial/unknown status. Разные units не суммируются; offline source не становится нулём. Total для совместимых sources несёт completeness/freshness каждого участника. Item identity включает source/account; одинаковый текст из разных сервисов не доказывает дубликат.

Remote read/ack, local dismissed и presentation viewed — независимые состояния. Render/autorotation не выполняют read-through. Query/aggregate не выдаёт внешнему consumer больше private preview fields, чем разрешено grants; для badge-only режима preview можно вообще не запрашивать. Time series/personal metrics аналогично несут unit, interval, source и aggregation provenance; исправленные/перекрывающиеся measurements не суммируются дважды. Прогноз является derived value с указанными assumptions, не исходным измерением.

## Слои и удерживаемые outputs

Назначаемый пользователем modifier — gesture role, а не Windows virtual-key flag. На down фиксируются owner и binding revision. Если до release не было другого key press, solo action выполняется один раз на release. Использование сочетания помечает modifier consumed и исключает solo; repeated не создаёт новый solo/chord. Cancel, disconnect, grant revoke и нейтральный rearm освобождают состояние без solo action. Несколько modifiers разрешаются по явно скомпилированным bindings; неоднозначные одинаковые chords являются ошибкой конфигурации, а не зависят от порядка IPC.

Это правило различает жесты внутри DeckWeave. Suppression исходного события ОС требует отдельного input.intercept capability/grant и не следует автоматически из назначения modifier. В observe-only режиме редактор объясняет, что обычное системное сочетание продолжает работать. [UX и основание](../decisions/0022-live-editing-and-interaction.md).

Для каждого input gesture scheduler фиксирует выбранный binding, layer stack и owner на press. Release доставляется этому owner даже после смены слоя. Temporary layer возвращает token; pop освобождает именно token, а не «верхний слой». При совпадении priority порядок задаётся стабильным binding order; transparent mapping явно пропускает событие к следующему слою, отсутствие mapping не считается suppress.

Layer state имеет scope device group/workspace и входит в единый snapshot для bindings и visual feedback. Held synthetic outputs считаются по owner tokens: release одного owner не отпускает клавишу, удерживаемую другим. Disconnect/overflow/apply освобождает его tokens и требует neutral rearm. Tap/hold/chord policy и timing берутся из ingress/fake clock; async plugin не участвует в синхронном OS suppression deadline. Слой подсветки и слой input mappings могут быть связаны, но это разные сущности.

## Обязательные fixtures

- Два конкурентных run получают разные transcript/tab/artifact results; cross-run reference отклоняется.
- Accepted job без terminal result не запускает следующий шаг; duplicate completion не запускает его дважды.
- Lost response после отправки письма даёт unknown, без автоматической повторной отправки.
- Cancel/restart/apply между принятием и завершением job не допускает stale continuation.
- Fan-out не мутирует общий input; join/error/skip/null воспроизводятся по одному trace.
- Artifact TTL, quota, revoke и restart cleanup проверяются без реальных private data.
- Press на одном слое и release после смены слоя не оставляет held output; два owners не мешают друг другу.
- Две concurrent interaction sessions не смешивают query/pages/selection; Escape, timeout и provider crash не становятся success; stale result после apply не продолжает новый plan.

Сквозные сценарии и границы готовности: [product/scenarios.md](../product/scenarios.md).
