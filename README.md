# DeckWeave

Локальная модульная платформа для устройств, приложений и автоматизаций. Плагины предоставляют ввод, действия с типизированными результатами, состояние, экраны, подсветку и будущие media streams. Ядро связывает их через общие contracts, не содержит протоколов конкретного оборудования и не зависит от открытого окна UI.

Первый практический результат — ежедневно пригодная конфигурация Mirabox N4 Pro на Windows. Расширяемость заранее проверяется на разных keyboard/gamepad/surface descriptors, цепочках заданий, geometry-based lighting и media boundaries.

## Статус

Реализованы desktop shell, protocol/plugin foundations, revision/draft store и Application Gateway; завершена структурная DDD migration R1–R4. Фаза 4 уже идёт. [Аудит реализации](docs/research/pre-apply-review.md) определил [условия перед durable apply](docs/implementation/roadmap.md#pre-apply-gate): identity/ownership, общий mutation ledger, runtime coordination и recovery. Реальные устройства и automation engine ещё не подключены. Локальные проверки Windows проходят; ручная приёмка меню трея и первый запуск GitHub CI ещё впереди.

## Запуск для разработки

```powershell
npm ci
npm run dev
```

Нужны Rust/MSVC и WebView2. [Инструкция запуска и проверок](docs/implementation/development.md) описывает зависимости, portable data-root, сборку и границы проверенного. Закрытие окна оставляет host в трее; для завершения выберите «Выход».

## Документация

- [Карта документации и правила её расширения](docs/README.md).
- [Сквозные сценарии](docs/product/scenarios.md): Handy → текст → браузер, mail/Telegram, подсветка и слои.
- [Требования](docs/product/requirements.md) и [архитектура](docs/architecture/overview.md).
- [Референсы с pinned commits](docs/research/REFERENCES.md) и [анализ реализаций, альтернатив и переиспользования](docs/research/implementation-review.md).
- [План реализации](docs/implementation/roadmap.md) и [принятые решения](docs/decisions/README.md).

## Лицензирование

Приложение и core — `GPL-3.0-or-later`; protocol — `MIT OR Apache-2.0`, та же стратегия предусмотрена для будущих SDK/examples. У каждого существующего package есть license files и SPDX headers. [Источники и dependency policy](THIRD_PARTY.md) фиксируют provenance и точечные исключения; будущие заимствования требуют отдельной проверки.
