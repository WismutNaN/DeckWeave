# ADR-0020: Windows provider как package с раздельными runtime groups

**Статус:** уточнено [ADR-0023](0023-composable-windows-plugins.md); описанный ниже двухгрупповой package — текущая реализация до миграции, не целевой состав набора
**Дата:** 2026-10-12

## Контекст

Фаза 7 добавляет управление Windows и наблюдение штатной клавиатуры. Эти возможности используют разные OS lifecycles: Core Audio COM workers, window hooks/Shell STA, Raw Input message loop и synthetic media output. Один встроенный core service дал бы Windows-коду скрытый доступ и связал бы независимые permissions/отказы; отдельный process на каждое действие создавал бы лишнюю память и lifecycle noise.

`observe`, `intercept` и `emit` имеют разные риски. Базовый keyboard source обязан пропускать исходный ввод. Windows может отклонить focus или synthetic input, а callback APIs не должны блокироваться на plugin IPC.

## Решение

Поставлять `org.deckweave.platform.windows` как first-party package через общий plugin lifecycle. Package объявляет две runtime groups:

1. `control` — Core Audio, media transport, window state/catalog/focus и typed launch/open operations.
2. `input-observe` — Raw Input keyboard observation в pass-through mode.

Groups получают отдельные artifacts/process sessions, leases и permissions. Неиспользуемая group не запускается. Interception, generic input emission и scripts не входят неявно ни в одну group.

Windows APIs инкапсулируются infrastructure adapters. Callback/message threads выполняют только bounded capture и передают работу application worker-у. Plugin protocol содержит platform-neutral schemas, typed outcomes и ephemeral references; Win32/COM handles не пересекают boundary. Первый keyboard source объявляет `system_aggregate`; более точная granularity требует evidence.

## Последствия

- crash Raw Input source не выключает volume/window actions, и наоборот;
- пользователь видит минимальные grants по фактически используемой группе;
- baseline память ограничена lazy activation двумя осмысленными process boundaries, а не process-per-action;
- Windows-specific dependency не попадает в core/shared SDK;
- прямое переиспользование reference code требует file-level provenance/license; общие идеи и официальные APIs не считаются скопированным implementation;
- intercept-helper или generic output могут появиться отдельной runtime group без изменения automation/domain model;
- package всё ещё исполняется с правами OS user: process isolation локализует crash, но не является sandbox.

План и API evidence: [Фаза 7](../implementation/phase-7-windows.md), [Windows research](../research/phase-7-windows.md).
