# Граница ядра и плагина перед Desktop UI

Этот документ связывает контракты в один путь данных. Детальные правила остаются у [capabilities](capabilities.md), [plugins](plugins.md), [композиции](plugin-composition.md), [automation](automation.md), [presentation](presentation.md), [lifecycle](lifecycle.md) и [media](media.md). Наличие типа в protocol не означает наличие работающего runtime: доказательства и оставшиеся блокеры находятся в [pre-6 gate](../implementation/pre-phase-6.md).

## Что сообщает пакет и что сообщает активный плагин

До исполнения кода пакет предоставляет manifest, точные artifact hashes/platform, runtime groups/contributions, permissions, settings/payload schemas и статические endpoint contracts. Реализован manifest extension `org.deckweave.automationCatalog`: `{formatVersion: 1, schemas: CatalogSchema[], endpoints: CatalogEndpoint[]}`. Он входит в проверяемый hash manifest. Каталог выбирается по package lock конкретного workspace; неоднозначный package/version/artifact и противоречащие shared schemas отклоняются. Import/index не запускают процессы.

Статическая запись не доказывает доступность устройства. После authenticated handshake и feature negotiation host активирует configured instance: contribution ID, instance ID, settings revision/schema/value, CredentialRef mappings и locale context. Ответ содержит descriptors с revision: node, device/service identity, elements, ports, availability, directions, value schemas, units/ranges, geometry, rate/format/size limits. Readiness всех contributions одной runtime group предшествует публикации registry и запуску event workers. Configured runtime согласует `org.deckweave.activation_commit`: до `ActivationCommitted` он не публикует signals; сообщение содержит activation epoch и host ticks. Иначе первый source мог начать работу во время подготовки следующего instance.

У package, runtime group, configured instance, process session/generation, physical device и graph node разные IDs. Один процесс может обслуживать несколько contributions и accounts. Endpoint должен принадлежать именно activation выбранного instance; совпадения package/contribution недостаточно. Две activation не могут заявить один и тот же physical/service identity как разные независимые targets. Для двух аккаунтов provider объявляет разные устойчивые service identities. Item списка задач или окон не требует отдельного port.

Неизвестные optional extensions сохраняются при переносе документа. Неизвестная required feature или недоступный broker блокирует исполнение с диагностикой. Сохранённый `serviceBindings` уже имеет typed model, но live nested-service broker ещё не подключён: compiler отклоняет такое исполнение явно.

## Четыре пути данных

| Данные | Путь и ответственность ядра | Ответственность плагина |
|---|---|---|
| Нажатие/отпускание, энкодер, analog/pressure, state, action/result | Registry → полная endpoint identity → bounded control envelope → schema validation → ordered scheduler → invocation/value write → terminal result | Декодировать native report; объявить семантику, единицы и диапазон; не подменять относительное delta абсолютным state |
| Изображение, значок, файл, записанный звук | Asset/artifact reference с format, size, lifetime, sensitivity и consumer lease; control payload содержит reference | Получить/создать bytes, объявить MIME/codec и метаданные; преобразование не маскировать изменением label |
| Визуальный объект/сцена/подсветка | Presenter/compositor держит desired model, layout, ownership, hit mapping и frame fences; device adapter получает согласованный frame/light representation | Объявить surface/LED geometry, форматы, limits, readback и onboard capabilities; сериализовать команды устройства |
| Непрерывный звук/видео/частый sensor stream | Отдельный negotiated media lane, clock domain, buffer ownership/backpressure и discontinuity; не серия больших JSON events | Capture/playback/DSP в выбранном media runtime; реальные RT guarantees проверяются отдельно |

Первые control routes исполняются. Asset/data/service ports, presentation state и media negotiation имеют разную степень готовности; renderer, artifact broker и realtime media transport нельзя объявлять готовыми из-за наличия DTO. Полный `EndpointPath` добавлен в `FramePacket`/`FrameReceipt`; legacy `target_port_id` deprecated и не является глобальным адресом. Реальный frame adapter обязан согласовать `org.deckweave.frame.endpoint_path` до использования этих полей.

Pressure, travel и force — разные величины. Binary keyboard сообщает только доступное ему событие; ядро не может восстановить силу из binary input. Нормализованное значение 0..1 не становится ньютоном без объявленной calibration. Аналогично ядро знает отправленную сцену, но фактические pixels экрана, firmware objects или текущее onboard состояние читает только через явную readback capability. Unsupported, unavailable и stale — разные состояния.

## Совместимость и преобразования

`SchemaRef` включает namespace и major/minor. Incoming signal и terminal result проверяются относительно descriptor/ожидаемого operation result; чужой schema ID, отсутствующая schema или неверный JSON не попадают в graph. `PayloadSchema` — ограниченный structural vocabulary: bounded records/arrays/strings, enum choices, optional fields и nullable. Unknown constraints отклоняются; opaque `Json` не доказывает структуру для field binding. Числа и numeric ranges должны быть finite.

Action arguments задаются отдельной таблицей: один explicit trigger port и named fields из constants, trigger, state или результата узла в текущем run. Compiler проверяет поля и доступный тип проекции; scheduler повторно проверяет фактический record. Missing field не вызывает действие. Результаты двух concurrent runs не пересекаются и освобождаются при завершении/cleanup. Квоты общего payload действуют и на сохранённые run results.

Числовые units/ranges не теряются при соединении ports. Для вложенных физических величин требуется явный typed record/converter: текущий minimal binding не выполняет implicit unit conversion. Media negotiation консервативно требует совпадения полного format descriptor и пересечения buffer bounds; 44.1 kHz и 48 kHz требуют resampler, два clock domains — согласования времени. RGB/brightness conversion, channel mapping и координатные преобразования также принадлежат объявленным adapters, а не скрытой эвристике ядра.

## Язык и время

Язык программирования plugin определяется поддержанным runtime artifact/SDK, не языком Desktop UI. Rust/TypeScript/Python не переключаются при выборе русского интерфейса.

Язык текста задаёт host `LocaleContext {revision, requestedLocale, timeZone}`. Он передаётся при activation; negotiated `org.deckweave.locale_context` позволяет обновление работающего runtime без apply workspace. Более старая revision и другая конфигурация с той же revision отклоняются. Контекст локализации не переводит IDs, enum machine values, namespaced kinds или пользовательские данные.

`TranslationCatalog` выбирает текст для каждого ключа: requested locale → языковые родители → default locale пакета → явный fallback. Ключи и fallback должны быть доступны и для отсутствующего перевода. Валидация timezone строки в DTO — проверка wire bounds, не встроенная база IANA; календарный adapter отвечает за реальный timezone/DST расчёт. Persistence пользовательского выбора и загрузка переводов из package assets ещё требуют application use case перед языковым inspector.

Invocation использует host monotonic milliseconds, отдельные от wall clock/date и device source ticks. `remaining_budget_ms` передаётся provider при фактической отправке; сравнивать host ticks с локальным `Instant` другого процесса нельзя. Timer pump завершает просроченный job и при отсутствии входящих событий. Source sequence/clock/quality metadata и multi-device clock synchronization требуют самостоятельной conformance проверки, а не подстановки host wall time.

## Порядок, cleanup и безопасность

Reliable key edges, encoder deltas, terminal outcomes и cleanup не coalesce. State и явно помеченный `org.deckweave.delivery.latest` Value допускают замену последним значением. Результат scheduler и admission его команд сериализуются одним host owner; dispatcher только помещает команды в bounded queue и не вызывает event sink синхронно.

`org.deckweave.owner_cleanup` передаёт epoch/run owner и уникальный cleanup ID; provider отвечает receipt. Отсутствие feature, overflow, negative ACK и timeout не являются успешным release. Stop ждёт receipts в пределах budget; неподтверждённый release виден как fault. Cancel/release допускаются после закрытия обычного dispatcher. После crash процесса подтверждения от него не существует: native held effects требуют host-owned lease/recovery adapter. Простое завершение процесса не доказывает, что ОС отпустила синтетическую клавишу. Этот acceptance остаётся обязательным до physical input/output plugin.

Permissions проверяются для principal/package/selector/access, а не объединяются в глобальный список разрешённых имён. Production compiler получает доступ по конкретному endpoint; runtime повторно проверяет текущую grant revision. `ScopedAuthorizationStore` начинается пустым; installation/import ничего не разрешают. Grant-management persistence/UI и credential mapping должны быть отдельными host use cases. CredentialRef — ссылка, не secret. Coordinator выполняет read-only preflight через CredentialBroker с principal/package/instance scope до запуска process. Default broker возвращает unresolved/needs-attention. Успешный preflight ещё не является runtime secret lease: acquire/use/renew/revoke adapter проходит отдельную acceptance.

Nested service call получает host-issued owner, выбранный role binding, deadline, call path и пересечение grants caller/provider. Pure resolver и delegation/subscription ports уже фиксируют missing/optional/cycle/artifact-conflict, bounded depth, cancellation owner и resync при sequence gap. Реальный broker должен поддержать эти правила до первого зависимого service plugin; прямые сокеты между plugins не заменяют его.

## Что будет визуализировать Desktop UI

Редактор показывает immutable catalog contracts, configured instances/settings revisions, required features и permissions, graph trigger/value bindings, registry availability, runtime health/trace, desired scene/slots и unresolved diagnostics. Он не хранит собственную копию execution truth и не делает вид, что неподключённая feature уже выполняется.

Collection item адресуется package + instance + collection + item ID + item revision. Click относится к presentation revision; press захватывает ItemRef, release/cancel использует захваченную ссылку после reorder. Slot selections независимы для устройств/кнопок; количество consumers определяет необходимость render/tick. Scene/slot switch — runtime command соответствующего owner, а не запись нового workspace на каждое нажатие.

Invocation-scoped `InteractionSession` завершается выбором/отменой/deadline и ограничивает retained history. Query fence включает session generation, activation epoch, provider generation и query generation. `DesktopViewSession` имеет отдельный lease и документ: закрытие редактора заметки не удаляет заметку и не отменяет все другие окна. Plugin-owned documents/assets изолируются authenticated namespace; их revision/CAS/retention не откатываются молча при workspace rollback. Реальные presenters/storage adapters проходят отдельную acceptance после contract fakes.
