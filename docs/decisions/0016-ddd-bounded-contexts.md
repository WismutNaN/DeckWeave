# ADR-0016: DDD bounded contexts в модульном монолите

**Статус:** принято  
**Дата:** 2026-10-12

## Контекст

Ранние вертикальные срезы доказали storage, plugin lifecycle и application boundary, но несколько crates начали совмещать доменные модели, use cases и adapters. Простое ограничение длины файлов не устраняет неоднозначное владение инвариантами и может только размазать связанную ответственность.

## Решение

DeckWeave остаётся Rust modular monolith, но организуется по bounded contexts: Workspace Configuration, Host Lifecycle, Application Gateway, Automation Execution, Capability Registry, Plugin Runtime, Surface Composition, Platform Input и Interaction UI. Внутри контекста зависимости направлены от adapters к application/domain. Общие типы попадают в минимальный Shared Kernel только при устойчивой семантике и нескольких потребителях.

Архитектурные проверки контролируют запрещённые dependencies и ratchet размеров. Миграция существующего кода выполняется постепенно, начиная с characterization tests, с временными re-export и без big-bang rewrite.

## Последствия

- владелец каждого инварианта становится явным;
- application service маршрутизирует use cases, но не поглощает их domain logic;
- profile filesystem code перестаёт определять модель draft;
- добавление provider или UI transport не меняет domain;
- потребуется несколько промежуточных adapters и перенос тестов;
- Shared Kernel нельзя использовать как каталог удобных DTO.

Нормативные правила: [DDD и границы контекстов](../architecture/ddd.md).
