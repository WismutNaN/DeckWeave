# ADR-0023: Windows как набор самостоятельных плагинов

**Статус:** принято; миграция не выполнена
**Дата:** 2026-10-13
**Уточняет:** [ADR-0020](0020-windows-provider-runtime-groups.md) в части состава package/groups

## Контекст

Пользователь ожидает самостоятельные низкоуровневые plugins, которыми можно пользоваться напрямую и на основе которых строить другие plugins. Установка Windows-набора должна быть удобной, но использование громкости не должно требовать включения запуска программ и наблюдения окон. Существующий package с двумя groups не обеспечивает такое разделение внутри `control`.

## Решение

Windows-набор является коллекцией устанавливаемых plugins, а не одним обязательным runtime. Первые границы по пользовательским возможностям:

| Плагин | Ответственность | Зависимости |
|---|---|---|
| Audio | Громкость, mute, audio state | Без windows/window dependency |
| Media | Allowlisted команды воспроизведения | Без keyboard-observe dependency |
| Windows | Список/активное окно, icons, focus | Самостоятельный provider для правил и раскладок окон |
| Applications | Exact launch/open и рецепт «перейти или запустить» | Exact launch использует Host broker; рецепт использует Windows service |
| Keyboard observation | Aggregate input в pass-through mode | Не включает interception/emission |

Названия в таблице функциональные; окончательные stable package IDs фиксируются вместе с migration map и manifests. Отдельный process на каждую кнопку или команду не создаётся. Общий Rust-код adapters/helpers переиспользуется как библиотека без требования запускать соседний plugin. Runtime dependency нужна для совместного живого сервиса — например, layout consumer использует один каталог окон.

Низкоуровневый provider публикует доступные пользователю actions/states и versioned service contract для consumers. Required/optional dependencies, selection конкретного provider/instance, scoped grants и leases проходят существующий [контракт композиции](../contracts/plugin-composition.md). Высокоуровневый plugin не получает разрешения своего dependency автоматически. Host обнаруживает missing/version conflict/cycle до activation, не передаёт произвольные IPC addresses или handles.

Поставка общим набором не запускает все plugins. Runtime удерживают только активные consumers; выключение последнего consumer освобождает его lease. Отказ необязательного provider затрагивает соответствующие функции, а не весь набор.

## Миграция и приёмка

До закрытия Фазы 7 разделить текущий `org.deckweave.platform.windows` и manifests/permissions/catalogs. Не перезаписывать pinned packages и не выдавать grants новым package IDs молча. Для старых drafts/locks нужен явный migration path с согласованной заменой endpoint references; старое содержимое сохраняется для rollback. Не оставлять два активных наблюдателя одного источника после миграции.

До первого production consumer подключить live service broker: имеющиеся fake conformance fixtures сами по себе не подтверждают живую межплагинную связь. Проверить standalone provider, двух consumers одной subscription, независимую отмену, restart/revoke и отсутствие дублированных запросов/наблюдателей. Измерить память и время включения реально используемого набора; lazy activation не оправдывает лишние постоянно работающие процессы.

Границы OS adapters, native trust model и Host launch broker из ADR-0020/0021 сохраняются. Изменяется упаковка возможностей и зависимостей, а не владение бизнес-логикой ядром.
