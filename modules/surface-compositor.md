# Модуль: Surface Compositor

**Ответственность:** компилирует retained scene, связывает state, выполняет layout/hit testing, растеризацию и frame scheduling для display surfaces.
**Расположение:** `crates/deckweave-surface/`.

## Публичный интерфейс

| Символ | Тип | Описание |
|---|---|---|
| `SurfaceScene` | document/runtime model | Visual tree |
| `SceneNode` | enum | Group/shape/image/icon/text/widget/timeline |
| `HitRegion` | struct | Independent interaction geometry |
| `SurfaceDescriptor` | struct | Hardware geometry/formats/FPS |
| `FrameIntent` | struct | Logical render request |
| `FrameScheduler` | owner | Dirty/latest-frame/FPS policy |
| `HitTestResult` | struct | Coordinates, region и transform |

## Зависимости

| Модуль | Что использует |
|---|---|
| Domain & Protocol | State/Frame ports |
| State snapshot port | Immutable bindings/feedback без зависимости от scheduler implementation |
| Widget frame port | Optional bounded frames, без импорта widget-host |
| Diagnostics | FPS/dirty/drop metrics |

## Инварианты

- Visual objects не определяют touch regions.
- Logical coordinates отделены от device pixels.
- Slow sink не создаёт unbounded frame queue.
- Event-driven render является default.
- Preview имеет deadline/stop и не меняет applied scene.
- Latest-frame-wins относится к полным кадрам; diff имеет base frame ID, потеря ACK вызывает full resync.
- Frame bytes идут bounded chunks с control latency budget; audio/video stream остаётся отдельным data plane.
- Preview TTL/stop восстанавливает текущую applied scene; apply отзывает lease, preview touch не запускает production actions.
- Frames и contacts ограждены session/activation epoch; старый renderer не пишет после cutover.

## Намеренно НЕ обрабатывает

- USB frame transfer;
- widget JavaScript;
- desktop canvas editor state;
- media video streams.

## Заметки для агента

> Renderer backend выбирается spike/benchmark; public scene types не должны зависеть от wgpu/Skia. N4 Pro full screen является одной surface, а legacy cells — template.

Нормативны [frames/preview contracts](../docs/RUNTIME_CONTRACTS.md#frames-и-preview).
