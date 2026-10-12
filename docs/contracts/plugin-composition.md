# Композиция плагинов и владение их данными

Статус: принятый целевой контракт; dependency/service broker, configured instances и plugin data ports ещё не реализованы полностью. Текущая граница — [аудит `2d99c88`](../research/plugin-feasibility.md). Решение: [ADR-0019](../decisions/0019-plugin-composition.md). Schema/wire поля ниже описывают логическую модель; существующее свободное поле manifest `dependencies` ещё не обеспечивает её исполнения. Unsupported required contract должен отклоняться, а не молча игнорироваться.

## Способы переиспользования

| Способ | Когда применять | Владелец исполнения |
|---|---|---|
| SDK library | Stateless HTTP/JSON/process helpers, converters, protocol encoding | Каждый package; без дополнительного runtime dependency |
| Workflow/recipe | Взять transcript → создать task; input → scene selection | Automation, через обычные typed operations |
| Service binding | Shared MQTT connection; layout provider использует window service | Host-mediated port, отдельные provider sessions и leases |
| Presenter/template | Одна task/window/note collection на разных surfaces | Scene/presentation owner; domain provider не знает hardware |

HTTP plugin нужен для произвольного запроса пользователя. Он не обязателен каждому connector, использующему HTTP library. Domain adapter не копирует Windows enumeration или MQTT connection, если требуется один общий ресурс. Не выделять отдельный процесс для каждой вспомогательной функции.

## Требования packages и выбранные bindings

Различаются четыре зависимости: exact package artifact, versioned service capability, external executable/system feature и host protocol feature. Manifest объявляет required/optional requirements без запуска кода; ServiceRequirement содержит namespaced contract/version, нужные operations/scopes и multiplicity. Package dependency не даёт права вызвать все операции установленного package.

Workspace хранит выбранный ServiceBinding: consumer configured instance/role → provider configured instance/service endpoint. Выбор по «первому установленному plugin» запрещён; ambiguity, missing и incompatible показываются до apply. Optional dependency отключает только зависимую функцию, не весь независимый package. Нельзя читать историю/credential другого аккаунта потому, что два instances разделяют процесс.

Plan разрешает transitive closure, scopes, schemas, required features и lazy activation DAG; cycles required services отклоняются с путём. Optional edge не исчезает молча для обхода cycle. Revision lock фиксирует exact artifacts всей closure. В v1 остаётся одна active version package на host: несовместимые ranges дают conflict, не скрытый второй экземпляр версии. Установка/обновление не происходит при startup/graph execution. Update показывает общий impact/permission diff; rollback восстанавливает прежний lock, но не восстанавливает отозванные grants.

Indexed package не запускает dependencies. Runtime lease появляется только для active consumer/subscription/job/view; readiness provider предшествует consumer. Reverse stop/cancel и failure propagation имеют общие budgets. Несколько consumers делят connection с reference-counted leases; shutdown одного не разрушает остальных. При restart заменяется generation, старый binding/response больше не принимается. Не вводить циклические Rust crate dependencies: Plugin Runtime реализует coordination через ports, Automation не импортирует providers.

## Настроенные instances

ConfiguredInstanceId — сохранённая identity использования contribution (account, MQTT broker, weather location), не process/session ID и не physical DeviceId. Instance содержит settings schema/version, settings revision и CredentialRef mappings; значения secrets хранятся отдельно. Runtime group multiplexes несколько configured instances и contributions в одном процессе. Wire activation/command/state однозначно адресует instance либо negotiated service/device endpoint, который его представляет.

Prepare валидирует settings до commit, получает descriptors всех нужных instances, проверяет group membership и requirements. Изменение settings не меняет чужой account/instance; unresolved credential даёт needs-attention. Динамический список окон/задач/заметок обычно является коллекцией, не тысячами новых ports. Ports меняются при изменении capabilities; collection items имеют собственные IDs/revisions.

## Вызовы и подписки через host

Первый выбор — композиция workflow. Для вложенного service call plugin не соединяется напрямую с чужим localhost/socket и не получает process handle. Scoped host port принимает consumer identity из authenticated session, role binding, operation/input, parent invocation или subscription owner, deadline и correlation. Provider/caller не могут подставить чужой owner.

Host проверяет разрешённую delegation, текущие grants обеих сторон и ресурсный scope binding. Запрос не получает больше прав, чем явно делегировано consumer для выбранной операции; у provider есть собственное разрешение выполнить её. CredentialRef не превращается в secret для caller: credential использует только broker/authorized provider. Service call получает обычный typed terminal result, limits и unknown-outcome semantics. Required service-call feature version согласуется до activation.

Call graph/depth, total fan-out/bytes и deadline ограничены; deadline дочернего вызова не дольше родительского. Await не удерживает coordinator lock и scheduler transition. Cancel/revoke распространяются к потомкам, уже выполненный effect не откатывается автоматически. Внутренний retry не скрывает повторный внешний effect. Replay использует recorded responses, не подключает реальный service.

Длительные subscriptions имеют отдельный owner/lease, snapshot+revision/sequence, freshness, bounded deltas и resync. Они не являются бессрочным invocation и не удерживают runtime после исчезновения всех consumers. Subscriber видит только scoped data; MQTT retained snapshot, task refresh и timer elapsed имеют разные event semantics.

<a id="plugin-data"></a>
## Данные плагина и срок хранения

| Вид | Владелец и правила |
|---|---|
| Настройки/bindings/preset template | Workspace revisions; versioned schema, review/apply/export |
| Notes, palettes, saved window arrangements, timer checkpoints | Plugin-owned versioned documents через scoped durable storage port; explicit private export, revision conflict и quota |
| Cache, icons, task/weather snapshots, metric ring | Rebuildable bounded storage, TTL/freshness, startup GC; stale не означает актуальное |
| Runtime artifact | Artifact broker, consumer lease, sensitivity/expiration; перенос результата между jobs |
| Credentials | Host credential broker; никогда profile/plugin document/default trace |
| Runtime scene/layer/game state | Scheduler/presentation owner; epoch/lease cleanup, persistence только по явной policy |

Host выдаёт namespace по package + configured instance + data-root/account scope, а не общий writable profile path. Data CRUD через port проверяет document revision, schema version, размер/число records и authorization. Provider выбирает domain content и migration; startup/index/import не исполняют произвольный migration code. Plugin data migration выполняется как отдельная наблюдаемая операция активированного provider со staging/backup и rollback policy; она не является workspace apply.

Удаление package не удаляет private notes молча: disable, uninstall, retention/export/purge — разные действия. Profile rollback не обещает вернуть прежнее содержимое внешнего task service или plugin data. Обновление заметки не создаёт новую workspace revision на каждую клавишу. Durable app data не хранят в web localStorage и не объявляют временным artifact.

Это логическая граница host APIs, не обещание OS sandbox для user-trusted native process: такой процесс обладает правами пользователя. Для custom web view broker остаётся фактической обязательной границей доступа.

## Приёмка

До UI Фазы 6 необходимы contract fixtures для двух аккаунтов, двух contributions одной runtime group, transitive dependency resolution, missing/optional/cyclic/version-conflict и revoke во время service call. Полный каталог/installer реализуется в Фазе 12, но UI не закрепляет модель «один package = один instance».

До первой реальной shared service integration: independent consumers/leases, provider restart, cancel/deadline, forbidden delegation, no-secret response и uncertain write. До private storage/overlays: crash during write, quota, concurrent revision conflict, export without private content и uninstall retention. Порядок — [pre-6 gate](../implementation/pre-phase-6.md).
