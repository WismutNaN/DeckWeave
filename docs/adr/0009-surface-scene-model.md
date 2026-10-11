# ADR-0009: Непрерывная surface scene и независимые hit regions

**Дата:** 2026-10-11  
**Статус:** Принято

## Контекст

N4 Pro имеет общий сенсорный экран, способный показывать единое изображение и возвращать coordinates. Legacy grid с четырьмя zones удобна как preset, но создаёт искусственные полосы и не позволяет свободную композицию.

## Рассматриваемые варианты

1. **Фиксированные cells** — просто, но теряет возможности hardware.
2. **Один bitmap без object model** — полный экран, но неудобное редактирование и feedback.
3. **Retained scene graph + независимые hit regions** — свободный layout, composition и точный touch.

## Решение

Выбрали **backend-neutral retained scene graph** и отдельный interaction layer. Grid существует только как snap/template. Scene рендерится в формат конкретного FrameSink, а coordinates проходят обратную transform и hit testing.

## Последствия

**Становится проще:** единый фон, текст вне сетки, widgets, animations и разные surface geometries.  
**Становится сложнее:** нужны compositor, transforms, dirty regions и отдельный editor.  
**Закрывает дверь на:** представление общего display как обязательного набора независимых кнопочных картинок.

