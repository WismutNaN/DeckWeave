# ADR-0009: Непрерывная surface scene и независимые hit regions

**Дата:** 2026-10-11  
**Статус:** Принято

## Контекст

Целевой UX N4 Pro предполагает единое изображение и coordinate-based interaction. Legacy grid с четырьмя zones удобна как preset, но ограничивает композицию. Уточнение source audit 2026-10-12: основной framebuffer, touch strip и их transform требуют отдельной hardware проверки; coordinate sample не доказывает down/up.

## Рассматриваемые варианты

1. **Фиксированные cells** — просто, но теряет возможности hardware.
2. **Один bitmap без object model** — полный экран, но неудобное редактирование и feedback.
3. **Retained scene graph + независимые hit regions** — свободный layout, composition и точный touch.

## Решение

Выбрали **backend-neutral retained scene graph** и отдельный interaction layer. Grid существует только как snap/template. Scene рендерится в формат конкретного FrameSink, а coordinates проходят обратную transform и hit testing.

## Последствия

Уточнение 2026-10-12: [frame contract](../contracts/lifecycle.md#frames-и-preview) разрешает latest-frame-wins для полных кадров; diff требует подтверждённой базы и full resync после потери. Preview имеет отдельный lease/TTL, а touch lifecycle — edge semantics.

[ADR-0015](0015-device-geometry-and-output-ownership.md) разделяет физическую geometry, light topology и UI layout. Sample-only pointer допускает hit test, но hold/drag требуют подтверждённого lifecycle. Scene abstraction не является доказательством возможностей конкретной прошивки.

**Становится проще:** единый фон, текст вне сетки, widgets, animations и разные surface geometries.  
**Становится сложнее:** нужны compositor, transforms, dirty regions и отдельный editor.  
**Закрывает дверь на:** представление общего display как обязательного набора независимых кнопочных картинок.
