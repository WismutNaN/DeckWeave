# ADR-0007: Лицензирование приложения и SDK

**Дата:** 2026-10-11  
**Статус:** Принято

## Контекст

DeckWeave является публичным проектом, хочет использовать идеи и, где совместимо, готовый код GPL/MIT/MPL/LGPL проектов. Одновременно plugin authors не должны принимать copyleft только ради использования protocol types и SDK.

## Рассматриваемые варианты

1. **Всё GPL-3.0-or-later** — единообразно, но повышает барьер для независимых plugin packages.
2. **Всё MIT OR Apache-2.0** — permissive, но запрещает прямое GPL-заимствование в core.
3. **GPL application/core + permissive protocol/SDK/examples** — copyleft продукта и низкий барьер внешним plugins.

## Решение

Выбрали **`GPL-3.0-or-later` для приложения, core и first-party runtime implementations**. Публичные protocol schemas, generated protocol types, SDK client libraries, conformance interfaces и минимальные examples публикуются как отдельные packages под **`MIT OR Apache-2.0`**. Каждый package содержит собственный license metadata.

## Последствия

**Становится проще:** контролируемо использовать GPL-compatible code в продукте и писать независимые plugins через permissive SDK.  
**Становится сложнее:** границы packages, notices и provenance должны строго поддерживаться; техническая process boundary не является автоматическим юридическим заключением.  
**Закрывает дверь на:** неявное единое лицензирование всего monorepo и заимствования без проверки.

