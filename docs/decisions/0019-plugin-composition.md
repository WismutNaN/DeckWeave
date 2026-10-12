# ADR-0019: Композиция providers, configured instances и shared services

**Дата:** 2026-10-12

**Статус:** принято для проектирования; runtime extensions проходят gate до Фазы 6.

## Контекст

Window layouts требуют общего window service, Sprut.hub — MQTT, а tasks/notes/weather — одинаковых presenters. Текущие package dependencies и runtime requirements ещё не моделируют настроенные аккаунты, service binding и data ownership. Копирование всех функций в каждый connector создаёт расходящиеся semantics; прямые plugin-to-plugin RPC обходят grants и lifecycle.

## Варианты

1. Каждый plugin сам реализует transport/UI/storage: проще первый пример, дороже совместимость и cleanup.
2. Любая функция становится plugin service: слишком много обязательных процессов и bootstrap dependencies.
3. SDK helpers + graph recipes + явно выбранные host-mediated services: переиспользование соответствует реальному ресурсу и владельцу.

## Решение

Выбран вариант 3. Stateless helpers остаются libraries; общие connection/OS resources могут быть service providers. Package/version requirement, capability requirement, external dependency и host feature различаются. Workspace явно связывает configured consumer/provider instances; transitive artifacts входят в lock, required service graph ацикличен. Host проверяет delegation, grants, budgets, cancellation и subscriptions. Никакой implicit install, выбора первого provider или передачи привилегий по факту зависимости.

Configured instance не равен runtime group/process. Несколько instances/contributions multiplexed в одной группе, данные/credentials разделены по scope. Plugin-owned durable data отделены от workspace configuration, cache и artifacts. Presenters используют общий [контракт views](../contracts/presentation.md); постоянная view не растягивает короткий InteractionSession на бесконечность.

Общая визуализация реализуется host presentation service и reusable templates, которые могут быть bundled package. Data providers не получают жёсткую зависимость от конкретного renderer process/backend. Один service обслуживает несколько bounded views/slots, сохраняет единый UI и idle budgets; custom web runtime запускается лишь по необходимости. Slot-scoped selection позволяет циклически менять виджеты одной кнопки без смены всего экрана.

## Последствия

Domain plugins остаются заменяемыми, UI не знает деталей Singularity/MQTT, один failure имеет явные зависимые scopes. Цена — binding/lease/delegation model и compatibility fixtures до SDK freeze. Это расширяет ports существующих DDD contexts, не вводит новый процесс центральной шины или зависимости между implementation crates.

Нормативный документ: [plugin composition](../contracts/plugin-composition.md). Проверяемые срезы: [до Фазы 6](../implementation/pre-phase-6.md). Новые manifest/wire features ещё не считаются реализованными только на основании этого ADR.
