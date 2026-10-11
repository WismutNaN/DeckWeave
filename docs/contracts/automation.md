# Контракт автоматизации и передачи результатов

Статус: проектируемый контракт. Минимальное исполнение проверяется в Фазе 5, композиция — в Фазе 11 [плана](../implementation/roadmap.md). Общие epochs, ordering и recovery определены в [lifecycle](lifecycle.md); этот документ определяет данные и семантику цепочек. Мотивировка: [исследование реализаций](../research/implementation-review.md).

## Четыре разных вида данных

| Данные | Назначение | Время жизни |
|---|---|---|
| Signal / state | Нажатие, положение оси, текущее mute | Event или versioned snapshot с freshness |
| Invocation result | Текст транскрибации, созданная вкладка, receipt письма | Конкретный вызов внутри конкретного run |
| Interaction result | Выбранный пользователем item/form value либо explicit cancel | Конкретная interaction session и owner invocation |
| Runtime artifact | Запись WAV, большой текст, вложение | Ограниченный срок хранения и lease |
| Media stream | Непрерывный звук или видео | Negotiated stream session, отдельный data plane |

Результат action не публикуется в общую изменяемую переменную автоматически. Два одновременных нажатия получают разные `RunId` и не подменяют результаты друг друга. `DataEdge` между узлами workflow адресует поле результата конкретного invocation; это внутренняя связь графа, а не новый hardware port. `CommandIn` описывает вход и результат одной операции. `StateOut` остаётся наблюдением текущего состояния, а не почтовым ящиком ответов.

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

`accepted` подтверждает принятие работы, не завершение эффекта. Короткая операция может сразу вернуть terminal response. Долгая возвращает `JobId` и далее progress/terminal outcome с `InvocationId`; `JobId` также scoped к provider session, если descriptor не обещает durable reconciliation. Progress coalesce допустим; terminal outcome — нет. Host принимает один terminal outcome, дубликат того же результата игнорирует, противоречащий результат регистрирует как protocol fault. До доставки запрос можно отклонить с гарантией отсутствия эффекта; потеря связи после dispatch без подтверждения даёт неизвестный outcome.

Cancel — запрос остановить работу. `cancelled` означает подтверждённое завершение cancellation, но не отмену уже выполненных внешних действий; report содержит известные частичные effects. По deadline host прекращает continuation, запрашивает cancel и очищает собственные leases. Если provider не подтвердил исход, остаётся `unknown_outcome`. Поздний результат может уточнить audit record, но не запускает отменённую/старую цепочку. Длительный job удерживает runtime lease, имеет общий deadline и progress timeout policy; heartbeat процесса не заменяет progress.

После host restart незавершённые runs не возобновляются автоматически. Сохранённый redacted run journal позволяет показать interrupted/unknown и запросить reconcile у поддерживающего его provider. Это не durable workflow engine с exactly-once effects. Apply отменяет runs прежней epoch по общему lifecycle. Перенос живых continuations между revisions вне v1.

## Композиция графа

| Узел или политика | Нормативное поведение |
|---|---|
| Sequence | Следующий шаг получает validated result предыдущего terminal success; ожидание acceptance недостаточно |
| Condition / transform | Встроенные pure expressions имеют пределы сложности/размера; plugin expression — async invocation с deadline |
| Branch | Невыполненная ветвь имеет `skipped`, а не фиктивный результат; downstream явно обрабатывает optional value |
| Parallel / join-all | Branch-local immutable results; join выдаёт record по стабильным branch IDs после terminal outcomes всех ветвей |
| Fail-fast | Останавливает дальнейший dispatch, запрашивает cancel остальных; уже совершённые effects остаются в report |
| First-success | Выбор победителя по записанному ingress order; проигравшие отменяются; применимость к side effects требует явной политики |
| Retry | Только явно выбранные категории, bounded attempts/backoff/jitter; неизвестный effect повторяется лишь с действующей provider idempotency guarantee |
| For-each | Snapshot входной коллекции, item scope, limit элементов и concurrency; результаты упорядочены по input index |
| Reentrancy | У binding явно `reject`, bounded `queue`, `restart` или bounded `parallel`; default — reject while running |
| Variables | Run-local default; shared workspace state меняется scheduler command с revision/merge policy, не конкурентной записью из plugin |

`restart` сначала отзывает право continuation старого run. Если требуется exclusive output, новая попытка ждёт подтверждённого cleanup/resource release; cancel не разрешает два владельца. Loops подчиняются временному барьеру и execution budget из lifecycle. Error policies `continue`/`fallback` передают tagged error/result, не превращают failure в success. Compensation — отдельный явно заданный action и тоже может завершиться ошибкой.

Случайность, current time и внешние ответы при replay берутся из fixture. Ветвь без recorded result получает `inconclusive`. Simulation никогда не читает реальные уведомления, clipboard или микрофон и не запускает код/браузер.

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

Telegram-подобный provider объявляет три независимые возможности:

1. `EventOut` новых/изменённых/удалённых записей с item ID и revision.
2. `StateOut` connection/unread/retention summary с freshness.
3. `CommandIn` read-only query для bounded page результатов с cursor, snapshot/watermark и `has_more`; acknowledgement — отдельная write operation.

Cursor непрозрачен и scoped к account/filter/provider generation; истечение требует нового snapshot. При разрыве sequence UI показывает gap и выполняет bounded resync. Item IDs дедуплицируются; edit/delete обновляют существующий item. History принадлежит сервису/provider cache, а не бесконечной очереди automation. «Все уведомления» означает доступную историю в объявленных account/retention границах; provider обязан показать ограничения API. Render карточки не помечает сообщение прочитанным автоматически.

## Слои и удерживаемые outputs

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
