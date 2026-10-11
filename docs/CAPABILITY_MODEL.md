# Модель возможностей DeckWeave

Capability model — общий язык hardware, software services, virtual targets и remote agents. Она описывает наблюдаемое и управляемое, но не диктует, в каком процессе или по какому протоколу это реализовано.

## Иерархия

```text
Node
└── Device или SoftwareService
    └── Element
        └── Capability
            └── Port
```

- **Node** — локальная, виртуальная или удалённая среда исполнения.
- **Device** — физический/виртуальный экземпляр оборудования.
- **SoftwareService** — OBS, active-window provider, weather source, SprutHub.
- **Element** — понятная пользователю часть: key, encoder, screen, microphone channel.
- **Capability** — семантическая функция элемента.
- **Port** — типизированная точка связи automation/media graph.

Категории «клавиатура», «геймпад» и «панель» являются UI tags/templates, а не закрытыми базовыми классами.

## Виды портов

| Port kind | Направление от provider | Семантика | Backpressure |
|---|---|---|---|
| `EventOut` | output | Дискретное edge event | Не coalesce; overflow является fault |
| `ValueOut` | output | Текущее непрерывное значение | Latest-value coalescing допустим |
| `StateOut` | output | Последнее наблюдаемое состояние | Snapshot + revision + freshness |
| `CommandIn` | input | Именованная операция с outcome | Bounded concurrency/deadline |
| `ValueIn` | input | Желаемое значение параметра | Latest desired value или explicit ramp |
| `FrameIn` | input | Full/diff frame для surface | Latest full-frame wins; diff требует подтверждённой базы |
| `HapticIn` | input | Tactile pattern | Bounded pattern queue/replace policy |
| `StreamOut` | output | Negotiated high-rate stream | Отдельный data plane |
| `StreamIn` | input | Sink negotiated stream | Отдельный data plane |

`StreamPort` передаёт descriptor и endpoint handle через control plane, но не raw audio/video buffers.

## Общий descriptor порта

Каждый port содержит:

- stable `PortId` и namespaced `kind`;
- direction и port semantics;
- `SchemaRef` и совместимый version range;
- физические units, range, precision и coordinate space;
- rate/latency hints;
- required permission scopes;
- availability и reason;
- session generation;
- optional tags для поиска/UI, не участвующие в identity.

## Базовые capability families

### Дискретный и позиционный ввод

| Capability | Payload | Примечание |
|---|---|---|
| `button.digital` | pressed/released/repeat | Physical и synthetic origin различаются |
| `switch.position` | position ID | Стабильные положения, не относительное движение |
| `encoder.relative` | signed steps | Resolution и optional acceleration |
| `encoder.press` | pressed/released | Может принадлежать тому же element |
| `gesture.discrete` | gesture ID + phase | Vocabulary объявляет provider |
| `contact.presence` | entered/left | Например, proximity/touch presence |

`encoder.relative` всегда `EventOut`: signed steps нельзя заменять последним значением. `touch.points` разделяет down/up/cancel (`EventOut`) и move samples (`ValueOut`, coalesce только между lifecycle edges). `StateOut` не гарантирует доставку каждого перехода; для trigger на каждый переход provider публикует отдельный event. Порядок и overflow recovery: [контракт сигналов](RUNTIME_CONTRACTS.md#порядок-сигналов-и-время).

### Непрерывный ввод и датчики

| Capability | Quantity | Обязательные метаданные |
|---|---|---|
| `axis.absolute` | normalized или физическая | min/max/center/dead zone |
| `key.travel` | длина хода | mm/µm, calibration, resolution |
| `pressure.force` | сила | N/relative, calibration |
| `touch.points` | contacts | surface size, IDs, pressure if known |
| `motion.acceleration` | vector | m/s² или normalized, sample rate |
| `motion.angular_velocity` | vector | rad/s или deg/s, sample rate |
| `orientation.pose` | quaternion/angles | reference frame, confidence |
| `audio.level` | level | dBFS/normalized, channel/window |
| `sensor.generic` | namespaced quantity | unit и schema обязательны |

Ход магнитной клавиши не называется силой, если датчик не измеряет force. Converter между quantities должен иметь физически обоснованную модель.

### State

| Capability | Пример |
|---|---|
| `device.connection` | connected/degraded/disconnected |
| `device.battery` | percentage, charging |
| `application.focus` | app/process/window identity |
| `media.playback` | playing, title, artist, position |
| `audio.mute` | muted/unmuted/unknown |
| `obs.recording` | inactive/active/paused |
| `notification.feed` | ordered summaries + unread state |
| `task.current` | task ID/title/status |
| `timer.state` | idle/running/paused/completed |
| `network.status` | connected/degraded/offline |

State содержит `observed_at`, `revision`, `freshness` и optional unavailable reason. Cache age всегда видим потребителю.

### Command и value output

| Capability | Пример |
|---|---|
| `system.media.command` | play/pause/next/previous |
| `application.command` | launch/focus/close |
| `device.session.command` | reconnect, stop preview |
| `audio.endpoint.volume` | target gain/mute |
| `light.color` | RGB/RGBA + logical brightness |
| `light.pattern` | static/blink/pulse/timeline |
| `display.brightness` | 0..1 |
| `haptic.pattern` | pulses/amplitude/duration |
| `timer.command` | start/pause/reset |
| `notification.command` | next/previous/acknowledge |

Command outcome имеет два этапа: transport acceptance и semantic completion. `unknown` означает, что side effect мог выполниться, но подтверждение потеряно; engine не делает слепой retry без idempotency guarantee.

## Визуальные поверхности

### Display surface

`display.surface` описывает непрерывный canvas:

- width/height, rotation, pixel aspect ratio и coordinate transform;
- accepted pixel formats, color space и alpha;
- preferred/max FPS;
- full-frame/partial-update/atlas capabilities;
- persistent/volatile frame semantics;
- brightness и safe frame on shutdown;
- optional hardware layers/composition.

Canvas не содержит обязательной сетки. Templates могут предлагать snap, но `HitRegion` является частью workspace scene и может иметь произвольную геометрию.

Для N4 Pro общий display объявляется одной surface. Четыре touch области из legacy UI являются только preset; raw coordinates доступны как `touch.points`.

### Display key и группы

LCD key — отдельная малая surface. Provider может также объявить `surface.group`/atlas sink, если hardware принимает один bulk frame. Compositor решает, рендерить ли отдельные scenes или общий atlas.

### Light

Light descriptor включает:

- addressable unit IDs;
- color model и gamut;
- hardware/global brightness constraints;
- update rate;
- supported hardware patterns;
- persistence после disconnect;
- safe state on host exit.

Индивидуальные кольца энкодеров являются отдельными targets, если protocol это допускает. Эмуляция цветом изображения не подменяет настоящую light capability.

### Haptic

`haptic.pattern` описывает:

- actuator selection;
- sequence amplitude/duration;
- repeat/priority/replace policy;
- возможность stop;
- hardware playback или host-timed approximation;
- minimum timing resolution.

Простое on/off устройство принимает тот же semantic pattern через provider-side degradation report.

## Media capabilities

| Capability | Направление | Negotiated properties |
|---|---|---|
| `audio.capture` | `StreamOut` | sample rate, format, channels, latency |
| `audio.playback` | `StreamIn` | sample rate, layout, latency |
| `video.capture` | `StreamOut` | raw/codec, dimensions, FPS |
| `video.display` | `StreamIn` | raw/codec, dimensions, refresh |
| `media.filter.audio` | stream in/out | accepted formats, latency, sidechain |
| `media.filter.video` | stream in/out | format, GPU/CPU constraints |
| `data.stream.generic` | stream in/out | namespaced media type and QoS |

Media descriptor резервируется сейчас; buffer ABI определяется media runtime ADR перед реализацией.

## Provisioning capability

Persistent device memory/configuration не моделируется обычным `CommandIn`. `device.provision` предоставляет:

- typed readable fields/slots;
- current snapshot и checksum/version;
- write constraints и risk level;
- dry-run diff;
- transactional support, если hardware её имеет;
- readback/verify method;
- recovery guidance;
- explicit exclusion firmware update.

Automation graph не может вызвать provisioning. Даже с confirmation token это отдельная application operation с собственным plan/readback/recovery. Provider никогда не предоставляет «raw USB report» как generic action.

## Resource capabilities

Некоторые операции владеют ограниченным ресурсом: exclusive HID interface, camera, virtual device, audio endpoint. Descriptor объявляет:

- shared/exclusive lease mode;
- max concurrent leases;
- preemption policy;
- acquisition deadline;
- cleanup/safe state;
- platform requirements.

Workspace compiler обнаруживает конфликт leases до apply, где это возможно.

Registry разрешает leases по canonical physical resource key, независимо от package/port aliases. Probe и preview используют тот же arbiter. Exclusive handoff между версиями provider может требовать downtime; v1 не обещает одновременное prepare двух exclusive owners ([контракт resources](RUNTIME_CONTRACTS.md#resources-discovery-и-версии-packages)).

## Идентичность и evidence

| Идентификатор | Срок жизни | Назначение |
|---|---|---|
| `PackageId` | Между версиями | Владелец contribution |
| `ProviderId` | Между совместимыми версиями | Discovery/protocol owner |
| `NodeId` | Между запусками после pairing | Локальная/удалённая среда |
| `DeviceIdentity` | Между reconnect, если возможно | Физический/виртуальный экземпляр |
| `DeviceSessionId` | Одно подключение | Отсечение late messages |
| `ElementId` | Descriptor compatibility lifetime | Пользовательский элемент |
| `CapabilityId` | Внутри element | Семантическая функция |
| `PortId` | Внутри capability | Connection endpoint |

VID/PID не являются уникальной identity. Provider прикладывает evidence: serial, HID path fragments, Bluetooth address policy, firmware, topology и confidence. При неоднозначности пользователь подтверждает binding.

## Временная семантика

`SignalEnvelope` содержит:

- `session_generation`;
- monotonically increasing `sequence` в объявленном session/source-stream scope;
- monotonic `captured_at` вместе с `ClockDomainId` источника;
- назначенные host `received_at`, `IngressOrdinal`, `HostBootId` и `ActivationEpoch`;
- optional wall-clock timestamp;
- `quality`: fresh/stale/estimated/degraded;
- `origin`: physical/synthetic/remote/replay;
- trace/correlation/causation IDs;
- schema-validated payload.

Tap/hold, debounce и chords v1 используют host monotonic clock после ingress; timestamps разных clock domains напрямую не сравниваются. Haptic timing, переданный устройству, относится к его объявленной timing policy. Calendar schedule использует timezone, DST и missed-fire policy. Replay записывает порядок ingress, timer firings и external outcomes ([контракт времени](RUNTIME_CONTRACTS.md#порядок-сигналов-и-время)).

## Совместимость портов

Edge разрешается, если:

1. direction совместим;
2. schema/version совпадает или есть explicit converter;
3. units и coordinate spaces совместимы;
4. rate/latency hints не нарушают hard constraints;
5. требуемые permissions известны; отсутствующие grants видны в draft, но блокируют execution обязательного route;
6. required resource lease разрешим;
7. edge не создаёт цикл без положительного delay/ожидания нового внешнего события и execution budget.

Редактирование offline draft не требует подключённого устройства. Compiler различает структурную совместимость и runtime readiness; missing provider/grant сохраняет данные и даёт report, а не удаляет edge. Unknown required schema/semantics запрещает исполнение.

Безопасные converters могут предлагаться автоматически, но всегда видны в graph. Примеры: `0..255 → 0..1`, поворот coordinates, unit conversion. Semantic guess запрещён.

## Namespaced extensions

Plugin может объявить `org.example.braille.cells` или другой namespaced kind. Core обязан:

- сохранить descriptor/settings без потери unknown optional fields;
- показать generic port и raw schema в diagnostics;
- связать его с совпадающей schema;
- не выдавать specialized editor, если UI contribution отсутствует.

Standard kind принимается в core vocabulary только после двух независимых providers либо широко принятого внешнего стандарта.

## Capability negotiation

Provider сначала объявляет максимальный descriptor, затем session возвращает фактически доступное подмножество. Причины различаются:

- unsupported by model/firmware;
- unavailable on current OS;
- permission missing;
- resource busy;
- disconnected;
- degraded fallback.

Workspace хранит требуемую semantic capability, а не случайный runtime handle. Это позволяет заменить provider или восстановиться после reconnect.

## Инварианты

- Каждый port имеет schema, direction и semantics.
- IDs не зависят от локализованного label.
- Edge events не coalesce.
- Continuous value coalesce сохраняет latest sequence и drop count.
- State всегда сообщает freshness.
- Disconnect инвалидирует session-scoped handles.
- Output сообщает outcome, а не только transport success.
- UI layout и hardware geometry связаны transform, но не тождественны.
- Permission привязано к contribution, resource scope и user grant.
- Provisioning, control command и firmware update никогда не смешиваются.
