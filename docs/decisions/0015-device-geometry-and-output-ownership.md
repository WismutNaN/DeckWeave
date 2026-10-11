# ADR-0015: Геометрия устройств и единственный владелец вывода

**Статус:** принято для спецификации, hardware свойства требуют fixtures.
**Дата:** 2026-10-12.

## Контекст

Input matrix, физическое расположение keys, LED topology и экранные pixels часто не совпадают. Wave, reactive dim/fade, input layers, preview и onboard effects могут одновременно менять один output. Исследованные QMK/OpenRGB показывают пользу отдельных geometry/mode models; N4 SDK показывает риск считать coordinate sample полноценным touch lifecycle.

## Решение

Provider описывает stable units, coordinate frames, element mappings и revision геометрии. Пользовательский canvas transform хранится отдельно. Surface Compositor компонует light scenes и выдаёт одному sink конечные frames с epoch/session/topology fencing. Simple light commands используют тот же arbiter. Host-rendered, volatile onboard preset и persistent program — отдельные capability modes.

Coordinate sample без доказанных contact phases публикуется как `pointer.sample`; `touch.points` требует достоверного lifecycle. UI не предлагает hold/drag там, где их нельзя наблюдать. Full canvas — абстракция сцены; physical target и transform подтверждаются provider fixture.

Детали: [lighting](../contracts/lighting.md), [capabilities](../contracts/capabilities.md), [N4 provider](../architecture/modules/n4pro-plugin.md).

## Альтернативы и последствия

Один LED на key, индекс как координата и last-command-wins проще, но ломаются на shared LED/underglow и конфликтующих feedback rules. Выбранная модель требует topology fixtures и явных merge policies. Общий renderer не делает EEPROM writes и не обещает аппаратный offload каждой host animation. Production persistent programming остаётся P2.
