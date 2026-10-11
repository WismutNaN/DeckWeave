# ADR-0017: Обычная клавиатура как bundled Platform Input provider

**Статус:** принято  
**Дата:** 2026-10-12

## Контекст

DeckWeave должен работать на ноутбуке без специальной панели: штатная клавиатура является источником triggers. При этом наблюдение, подавление и синтетический ввод имеют разные разрешения, гарантии и platform implementations. Встраивание keyboard-specific logic в core сделало бы модель зависимой от ОС и усложнило будущие HID/analog providers.

## Решение

Поставлять first-party Platform Input package вместе с приложением и показывать его как базовую возможность. Package использует общий plugin/capability contract и OS-specific adapters. Безопасный default — observe/pass-through. `observe`, `intercept` и `emit` являются разными capabilities/grants. Provider честно объявляет гранулярность: конкретное физическое устройство только при наличии надёжной identity, иначе агрегированный системный keyboard source.

## Последствия

- пользователь может назначить обычную клавишу без установки стороннего plugin;
- Linux/macOS получают собственные adapters без изменения automation core;
- отсутствие privileged helper понижает режим до observe, не ломая системный ввод;
- first-party package проходит те же lifecycle, permission и conformance rules;
- selective replacement остаётся отдельной P2-возможностью.

Модуль: [Platform Input](../architecture/modules/platform-input.md).
