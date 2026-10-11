# Граница media runtime

Статус: требования к будущему контракту, production media остаётся P2. В Фазе 2 проверяется descriptor vocabulary; конкретный buffer ABI/backend определяется отдельным benchmark и ADR. Основание: [GStreamer, PipeWire и CLAP](../research/implementation-review.md#медиа-и-удалённые-узлы).

## Control и data plane

MediaNodeProvider публикует static templates и session descriptors для sources, filters и sinks. Control IPC передаёт plan, negotiated formats, parameter changes и bounded meters/health. PCM/video buffers, clocks и synchronization primitives принадлежат специализированному data plane. Один пакет может иметь control и media contributions с отдельными runtime groups и trust scopes.

Нельзя обещать одновременно zero-copy, произвольную process isolation и любой codec/backend: выбранный media adapter документирует места копирования и crash boundary. Встраиваемый native DSP выполняется внутри отдельного media-host; crash этого процесса затрагивает все узлы в нём. Per-node isolation — отдельная опция с измеренной стоимостью, не следствие plugin metadata.

## Порты и negotiation

Stream template задаёт direction, availability (`always`, dynamic, requestable), поддержанные media types/formats, layout, rates, buffer bounds и latency. Instance порта имеет стабильный semantic ID, session и descriptor revision. Sidechain или дополнительный output создаются explicit request с resource lease и освобождаются owner-ом.

Compiler выбирает точные совместимые caps по всему route. Изменение rate/channel layout требует converter node (resample/mix/codec), latency report и ресурсов; скрытая конверсия или угадывание канала запрещены. Отказ negotiation возвращает конфликт конкретных constraints. Dynamic port removal отзывает связанные routes; renumbering не переназначает их молча.

Prepared plan не запускает capture. Start проверяет grants и resources, затем публикует stream generation. Reconfigure — prepare, quiesce/drain, negotiated swap, generation change; обещание бесшовности допустимо только при поддержке backend. Старые buffers/parameters не применяются к новой конфигурации.

## Владение buffers и время

Будущий wire/ABI должен определить allocator/owner, memory type, read/write access, maximum bytes, transfer/release protocol, alignment, lifetime, bounded in-flight buffers и crash cleanup. Shared memory handle не является произвольным адресом plugin; размер/offset проверяется до доступа. Отзыв grant останавливает новые buffers и закрывает endpoints, но не стирает уже скопированные данные из стороннего процесса.

Stream descriptor содержит clock domain, sample/frame position, rate, discontinuity markers, latency и clock-drift policy. Audio parameter automation передаёт sample offset в выбранной stream generation; обычные события keyboard используют host ingress clock. Bridge выполняет явное преобразование clock domains. Media thread не ожидает ответа automation scheduler или browser.

Real-time callback не выполняет allocations, blocking locks, filesystem/network, logging, plugin process spawn или unbounded work. Parameters приходят через bounded bridge; meters могут coalesce, а sample-accurate events сохраняют порядок и имеют overflow fault policy. Underrun/overrun, xruns, discontinuities и backpressure наблюдаемы. Video drop и audio underrun имеют разные policies.

## Жизненный цикл и отказ

Native bridge обязан отобразить create/init → configure/activate → start/process → stop → deactivate/destroy и поддерживаемые optional extensions. CLAP pointer lifetime и thread rules не меняются от того, что DeckWeave вызывает его через adapter. State restore допускается в разрешённом lifecycle этапе и не заменяет runtime buffer state.

Failure policy определяется для каждого route: silence/stop, last video frame с stale indicator либо bypass. Bypass возможен только при совместимых formats и живом bypass path; crash всего media-host не гарантирует работающий обход. Capture, preview и виртуальный microphone endpoint имеют разные permissions/leases. Восстановление не включает скрыто microphone/camera после отзыва OS permission.

## Запись для транскрибации

Конечная запись в WAV может выполняться отдельным trusted capture provider, который выдаёт `RuntimeArtifactRef`. Это job, не low-latency DSP graph; его можно добавить раньше production media runtime. Max duration/bytes, capture start/stop, exclusive endpoint policy и cleanup обязательны. Transcription provider получает artifact по [automation contract](automation.md), а не подписывается на raw PCM через stdio. Конверсия WAV явно объявлена adapter-ом.

## Gate до выбора backend

Сравнить GStreamer-host, platform-native graph и подходящий CLAP bridge на одном fixture: microphone → filter → monitor/virtual endpoint, reconfigure, missing sidechain, revoke, crash и device hotplug. Измерить round-trip latency, xrun rate, memory copies и CPU/RSS. PipeWire рассматривается как Linux integration, не переносимый Windows engine. Решение включает поддержку и распространение virtual endpoint per OS; заголовки ABI сами её не обеспечивают.
