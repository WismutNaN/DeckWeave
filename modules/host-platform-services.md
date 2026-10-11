# Модуль: Host Platform Services

**Ответственность:** предоставляет минимальные OS services, необходимые для запуска, хранения доверия и остановки host независимо от plugins.
**Расположение:** `crates/deckweave-platform-host/`; service ports в `crates/deckweave-application/`, wiring в desktop composition root.

## Публичный интерфейс

| Символ | Тип | Описание |
|---|---|---|
| `HostPlatform` | trait | Data roots, instance locks, autostart, metadata notifications |
| `CredentialBroker` | service port | Scoped resolution/use локального secret |
| `AuthorizationStore` | service port | Principals, grants, revisions и revoke |
| `ChildProcessGuard` | resource guard | Bounded cleanup дочернего process tree |

## Зависимости

| Модуль | Что использует |
|---|---|
| Application contracts | Ports и principal/resource types |
| OS APIs | Locks, credential store, autostart, process cleanup |
| Diagnostics | Redacted errors/metrics |

## Инварианты

- Никаких device vendor protocols или пользовательских automation actions.
- Platform implementation подключается composition root; portable domain/profile crates не импортируют OS APIs.
- Credential broker не зависит от запуска package, которому выдаёт scoped доступ.
- Grants не входят в profile и не восстанавливаются при rollback/import.
- Lock и canonical data-root validation предшествуют Store/API/hardware startup.
- Unavailable OS credential store даёт явный статус, без plaintext fallback.

## Намеренно НЕ обрабатывает

- Volume/media/window/input commands — ответственность isolated Platform Adapters.
- Произвольную sandbox native plugin.
- User-facing permission UI и vendor discovery.

## Заметки для агента

> Начать с locks/data roots/process cleanup в Фазе 1 и authorization/credential ports в Фазах 3–4. OS autostart подключить до daily milestone. [ADR-0013](../docs/adr/0013-dependency-and-host-service-boundaries.md) задаёт направление зависимостей; [SECURITY.md](../docs/SECURITY.md) — границы выдачи credentials.
