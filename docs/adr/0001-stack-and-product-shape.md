# ADR-0001: Стек и форма продукта

**Дата:** 2026-10-11  
**Статус:** Принято

## Контекст

DeckWeave должен оставаться лёгким локальным приложением, работать с USB/Bluetooth и поддерживать сложный визуальный редактор. Он не должен наследовать устройство одного Stream Deck-подобного продукта и не нуждается в распределённой серверной инфраструктуре.

## Рассматриваемые варианты

1. **Форк OpenDeck** — быстрый доступ к plugin host, но legacy-модель устройства и GPL определяют весь продукт.
2. **Electron/Node.js application** — сильная web-экосистема, но более тяжёлый baseline и слабее границы hardware runtime.
3. **Rust modular monolith + Tauri 2/Svelte** — новое ядро, лёгкий desktop UI, plugins отдельными процессами.
4. **Набор микросервисов** — гибкая deployment-модель, но избыточная сложность для локального приложения.

## Решение

Выбрали **Rust modular monolith с Tokio и Tauri 2/Svelte UI**. Один per-user desktop process владеет application core и tray; WebView можно закрыть, не останавливая runtime. Windows 10/11 является первой реализацией и средой физической приёмки, Linux — следующей; core contracts не зависят от OS. OpenDeck используется как референс compatibility host, а не как основа fork.

## Последствия

**Становится проще:** единое владение состоянием, низкий baseline, reuse Rust-библиотек, изоляция OS adapters.  
**Становится сложнее:** compatibility host и plugin SDK нужно реализовать самостоятельно.  
**Закрывает дверь на:** отдельные микросервисы как default deployment и device-specific логику внутри core.
