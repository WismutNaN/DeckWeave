# Модуль: Surface Compositor

**Ответственность:** компилирует display/light scenes, связывает state, выполняет layout/hit testing, рендеринг и единое владение visual outputs.
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
| `LightScene`, `LightTopology` | scene/descriptor | Spatial geometry, stable units и key→LED mapping |
| `EffectEnvelope`, `LightFrame` | runtime/frame | Wave/cooldown/fade, clock и конечные per-unit colors |
| `VisualOutputArbiter` | owner | Scene layers, preview и hardware modes для одного output channel |

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
- Light frames также имеют topology revision; matrix index не заменяет физические координаты.
- Один channel имеет одного owner; light commands, frames и onboard presets проходят тот же arbiter.
- Animation не пишет EEPROM; dropped frame не растягивает envelope timing.

Общие collection presenters, navigation и scene selection следуют [presentation contract](../../contracts/presentation.md). Slot click связан с displayed item/revision; repaint списка не меняет target уже начатого жеста. Domain logic tasks/windows/notes остаётся в providers. Пипетка и game входят в [каталог conformance-сценариев](../../plugins/productivity.md).

## Намеренно НЕ обрабатывает

- USB frame transfer;
- widget JavaScript;
- desktop canvas editor state;
- media video streams.

## Заметки для агента

> Renderer backend выбирается spike/benchmark; public scene types не должны зависеть от wgpu/Skia. N4 Pro full screen является одной surface, а legacy cells — template.

Нормативны [frames/preview contracts](../../contracts/lifecycle.md#frames-и-preview).

Spatial geometry, composition и host/onboard rendering определены в [lighting contract](../../contracts/lighting.md). Нужны отдельные fixtures для irregular layout, shared/missing LEDs, cooldown/retrigger и preview conflicts. Coordinate sample допускает hit test, но не создаёт contact lifecycle без evidence.
