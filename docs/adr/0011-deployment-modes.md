# ADR-0011: Per-user tray host и portable distribution

**Дата:** 2026-10-11  
**Статус:** Принято

## Контекст

DeckWeave должен запускаться при входе Windows, работать после закрытия окна и иметь portable вариант. Отдельная system service усложнит hardware/session/UI permissions и не нужна одному пользователю в первой версии.

## Рассматриваемые варианты

1. **System service + UI client** — устойчивый background host, но сложные session/privilege boundaries.
2. **Обычное окно, закрытие завершает приложение** — просто, но ломает назначение control surface.
3. **Один per-user tray host с detachable WebView** — runtime живёт в пользовательской сессии, UI скрывается.

## Решение

Выбрали **один per-user desktop host**, владеющий tray, application core и plugin supervisor. Installed mode может включать autostart; portable mode использует отдельный data root и не включает autostart автоматически. Headless host возможен позже как другой shell того же application core.

## Последствия

Уточнение 2026-10-12: [ADR-0013](0013-dependency-and-host-service-boundaries.md) задаёт single-instance lock пользовательской сессии и writer lock data root до API/plugin startup. Другой portable root не запускает второй host одновременно. Autostart/credentials/locks реализуются host services, не lazy plugins.

**Становится проще:** доступ к user-session devices/apps, один источник состояния и portable build.  
**Становится сложнее:** crash desktop process останавливает runtime; нужен recovery.  
**Закрывает дверь на:** multi-user/system-service deployment в v1.
