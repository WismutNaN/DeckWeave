# Геометрия, подсветка и аппаратное исполнение

Статус: проектируемый контракт. Descriptor входит в Фазу 2, базовый renderer — в Фазу 9, spatial effects и reactive envelopes — в Фазу 11 [плана](../implementation/roadmap.md). Основание: разделение matrix/point/flags в QMK и LEDs/zones/modes в OpenRGB ([исследование](../research/implementation-review.md)).

## Независимые модели

| Модель | Что описывает |
|---|---|
| Input matrix / usage | Как hardware адресует input; не физическое расстояние |
| Physical layout | Контуры keys/encoders и координаты элементов устройства |
| Light topology | Адресуемые LEDs/zones, их положение и связь с elements |
| UI layout | Расположение карточек/групп в редакторе; пользовательское представление |
| Display surface | Прямоугольные pixels/canvas и отдельные hit regions |

Клавиша может не иметь LED, иметь несколько LEDs или разделять один LED с другими. Underglow может не соответствовать ни одной клавише. `ElementLightMap` поэтому является явным many-to-many mapping со stable IDs, а не формулой `key index = LED index`. Пустая ячейка matrix не превращается в светодиод. Отсутствующая geometry не выдумывается из порядка USB reports.

## Light descriptor

`LightTopology` содержит `TopologyRevision`, stable unit/group IDs, coordinate frame, units (mm либо явно normalized), origin, axes, optional 2D/3D positions, bounds и element mappings. Provider задаёт полноту/точность координат и source evidence. Device-local geometry отделена от пользовательского transform группы устройств; при отсутствии физического масштаба cross-device wave требует явной пользовательской калибровки.

Sink объявляет color model/space, channel depth, gamut, global/per-unit brightness, max frame bytes/FPS, min interval, поддерживаемые bulk/diff updates, volatile/persistent effects и safe state. RGB с общей яркостью не выдаётся за независимую яркость каждого LED. Изменение topology атомарно меняет descriptor revision и требует revalidation scene mappings; старые индексы не направляются новым units.

Для адресуемого массива `FrameIn` принимает `LightFrame` — bounded набор цветов по units с topology/frame/session/epoch IDs. Display `PixelFrame` и `LightFrame` имеют разные schemas; FrameIn обозначает семантику полной замены или diff, а не обязательную картинку. Простые `light.color`/brightness commands проходят тот же arbitration path. До первого ACK diff запрещён; reconnect требует full frame ([lifecycle](lifecycle.md#frames-и-preview)).

## Один владелец физического вывода

Surface Compositor владеет visual output arbitration: display scenes и light scenes используют общие clock/frame budgets, но разные renderer implementations. Engine отдаёт semantic intents/layer changes; он не отправляет USB-команды на каждом animation tick. Plugin преобразует конечный кадр в protocol устройства.

Для одного resource/channel существует один output owner. Две независимые программы с USB и OpenRGB aliases не могут одновременно получить exclusive lease. Внутри одной light scene слои имеют stable IDs, явный порядок/priority и operation: replace, add-with-clamp, alpha-over или brightness-multiply. Compiler проверяет color-space conversion и ограничения channels. Default — base scene, затем ordered overlays; равный priority разрешается стабильным scene order. Последовательность поступления async commands не служит скрытым правилом смешивания.

Preview имеет отдельный ограниченный lease; конфликтующие production writes приостанавливаются на уровне arbiter, а stop/TTL восстанавливает текущую applied scene. Смена input layer меняет selected light layers одним scheduler snapshot. Overlay state scoped к epoch; disconnect/apply/cancel завершает effect и очищает timers. Safe state после физического unplug гарантируется только hardware, которое умеет его выполнять автономно.

## Пространственные и реактивные эффекты

Spatial effect — bounded функция `color(unit_position, logical_time, parameters)` поверх выбранной группы. Для wave задаются direction, speed с units, wavelength и palette. Для ripple — origin, distance metric и lifetime. Scene compiler проверяет geometry; без неё доступны static/group effects либо явно выбранная пользовательская раскладка. Расширение vocabulary начинается с декларативных primitives; plugin-generated light frames проходят тот же quota/arbitration, произвольный code не вызывается из core frame loop.

Сценарий «после нажатия потухнуть, подождать и плавно загореться» задаётся per-element envelope:

```text
press → dim (brightness = 0)
      → cooldown(duration)
      → fade(0 → 1, duration, easing)
      → base scene
```

Default: новое press перезапускает cooldown только связанных units, repeat игнорируется; пользователь может выбрать hold-until-release, extend или ignore-while-active. `hold-until-release` доступен только при достоверном release. Brightness envelope умножается на текущий base color, поэтому wave продолжает двигаться во время затемнения. Для shared LED правило merge задаётся явно (например, minimum brightness активных envelopes). Fade хранит start time, не накопленную сумму frame deltas: пропуск кадров не растягивает cooldown и не меняет итоговое время.

Rate определяется самым строгим из sink bandwidth, scene budget и host policy. Scheduler хранит current + latest full desired frame; медленный sink пропускает промежуточную анимацию с метрикой, не накапливает очередь. Все frames несут epoch/session/topology revision. Неактивная scene не требует polling loop.

## Host rendering и программа внутри устройства

Наличие чипа не доказывает программируемость. Provider раздельно объявляет host-rendered frames, hardware preset selection, volatile program upload и persistent provisioning. Program descriptor перечисляет поддерживаемые primitives, maximum steps/memory, clock resolution, stop/replace semantics, checksum/readback и сохранение после unplug. Host выбирает offload только для полностью поддержанного подмножества; approximate результат требует явного выбора и degradation report.

Для первоначальной версии допускаются frames и известные volatile presets. Произвольный offload input layers/macros и persistent programs отложены. Перед offload нужна проверка equivalence на fixture: одинаковые trigger/retrigger/release, порядок, clock policy и cleanup. Устройство, способное автономно отправлять input после падения host, требует собственной stop/watchdog policy; программируемость не обходит запрет firmware flashing.

Запись flash/EEPROM всегда относится к отдельному provisioning workflow: diff, checksum, write budget/endurance, confirmation, readback/recovery. Ordinary workspace apply, preview и animation никогда не пишут persistent memory. Если остановить автономную программу надёжно нельзя, это явно отображается и такая программа не используется для held synthetic output в v1.

## Fixtures

- Неровная клавиатура, matrix hole, underglow, два LEDs на key и shared LED.
- Wave одинаково ориентирована после rotation/scale transform; unknown scale не принимается как mm.
- Press → cooldown → fade с fake clock, repeated press, dropped frames и layer switch.
- Hardware preset и host frames конфликтуют на одном channel до dispatch.
- Old topology/session frame отвергается; disconnect/preview TTL восстанавливает допустимое состояние.
- Неподдерживаемый hardware effect получает unsupported либо выбранный fallback; EEPROM не пишется в ходе обычного apply.
