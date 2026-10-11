# Модуль: Media Runtime

**Ответственность:** в будущей фазе исполняет negotiated low-latency audio/video graph отдельно от control plane.
**Расположение:** `runtimes/media-host/`, protocol descriptors in domain now.

## Публичный интерфейс

| Символ | Тип | Описание |
|---|---|---|
| `MediaGraphPlan` | immutable plan | Sources/filters/sinks/formats |
| `StreamEndpoint` | handle | Negotiated data-plane endpoint |
| `MediaNodeDescriptor` | descriptor | Formats/latency/thread constraints |
| `ParameterBridge` | control interface | Bounded parameter changes/meters |
| `MediaHealth` | snapshot | XRuns/latency/bypass/failure |

## Зависимости

| Модуль | Что использует |
|---|---|
| Domain & Protocol | Stream descriptors only |
| Host Runtime | Lifecycle/health |
| Platform media adapters | Capture/playback/virtual endpoints |
| Future CLAP/GStreamer bridges | Optional plugin ecosystems |

## Инварианты

- Raw buffers не проходят plugin control protocol.
- Real-time callback не allocates/blocks/logs/network.
- Graph plan immutable during run; changes use prepared swap.
- Crash boundary separate from desktop host.
- Failure policy explicit bypass/stop.
- Python/web plugins never execute on media thread.

## Намеренно НЕ обрабатывает

- ordinary volume/mute commands;
- automation graph execution;
- UI widgets;
- v1 functionality.

## Заметки для агента

> Не создавать runtime до отдельного benchmark/ADR выбора backend и virtual endpoints per OS. Current work limited to stream descriptors and extension boundary.

