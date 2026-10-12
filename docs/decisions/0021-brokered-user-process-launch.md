# ADR-0021: запуск пользовательских процессов через Host broker

**Статус:** принято
**Дата:** 2026-10-13

## Контекст

Windows runtime плагина запускается внутри Job Object с `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE`. Это гарантирует bounded cleanup всего дерева при crash, revoke и shutdown. Если control plugin вызовет `Command::spawn` напрямую, пользовательское приложение унаследует job и завершится вместе с runtime. Разрешение `BREAKAWAY_OK` для job исправило бы lifetime приложения, но одновременно позволило бы любому коду плагина покинуть контролируемое дерево процессов.

`ShellExecuteExW` не заменяет exact process launch: Shell выбирает handler и принимает отдельную модель параметров. Запуск через Explorer/COM также не даёт общего доказательства exact executable + argv semantics. Поэтому нельзя объявлять `launch_process` готовым только потому, что дочерний process успешно создан внутри plugin runtime.

## Решение

Сохранить семантику, schemas и automation endpoint запуска у platform provider, но выполнять сам OS effect через минимальный Host-owned broker:

- `launch_process` передаёт executable, argv и cwd отдельными typed полями; shell concatenation запрещена;
- `open_target` является другой операцией и выполняется отдельным STA Shell adapter;
- Host повторно проверяет caller identity, актуальный grant, resource selector, deadline и input limits до effect;
- broker не добавляет запущенное пользовательское приложение в Job Object runtime плагина;
- delivery после возможного вызова OS API возвращает terminal success только по подтверждённому receipt, иначе `unknown_outcome`; автоматический retry launch запрещён;
- до появления negotiated broker feature оба endpoint остаются `unavailable`, а control runtime не получает право ослаблять собственный Job Object.

Broker является узким Host Platform Service, а не вторым automation engine. Platform plugin по-прежнему владеет пользовательской моделью действий и переводом результата в capability contracts; Host владеет только безопасной границей жизненного цикла процесса.

## Последствия

- запущенное приложение может жить независимо от lease/crash Windows provider;
- isolation остальных plugin runtimes не ослабляется;
- direct child process остаётся допустим только для внутренних helper-ов, которые должны завершаться вместе с runtime;
- Phase 7.3 разделяется на готовый window state/catalog/focus slice и отдельный brokered launch/open slice;
- service-broker protocol, fault tests around spawn receipt и Windows manual matrix становятся обязательными до публикации launch/open как `available`.

Связанные документы: [Фаза 7](../implementation/phase-7-windows.md), [границы Host services](0013-dependency-and-host-service-boundaries.md), [Windows runtime groups](0020-windows-provider-runtime-groups.md).

