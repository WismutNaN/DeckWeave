# Модуль: Remote Agent

**Ответственность:** подключает paired Android/Steam Deck/other-PC nodes, объявляет их capabilities и управляет revocable control/media sessions.
**Расположение:** `crates/deckweave-remote/`, future agent applications.

## Публичный интерфейс

| Символ | Тип | Описание |
|---|---|---|
| `PairingService` | service | Mutual identity bootstrap |
| `RemoteNodeSession` | runtime handle | Control connection/session generation |
| `RemoteCapabilitySet` | descriptor | Available hardware/software |
| `RemoteGrant` | struct | Revocable capability permission |
| `RemoteTransport` | trait | Carrier-independent control |
| `RemoteStreamOffer` | descriptor | Negotiated media endpoint |

## Зависимости

| Модуль | Что использует |
|---|---|
| Domain & Protocol | Capability model |
| Capability Registry | Remote topology |
| Media Runtime | Future stream endpoints |
| Security/credentials | Pairing keys |

## Инварианты

- IP address не является identity.
- Pairing не выдаёт camera/mic/screen grants.
- Control и media channels разделены.
- Disconnect инвалидирует session handles.
- Carrier change не меняет semantic descriptor.
- Source timestamp имеет ClockDomainId/uncertainty; host ordering/freshness не вычисляется прямым сравнением чужого monotonic clock.
- Reconnect не переигрывает старые button events/commands, held state требует resync/rearm.

## Намеренно НЕ обрабатывает

- master workspace editing on agent;
- implicit media capture;
- virtual display driver;
- cloud relay in first implementation.

## Заметки для агента

> Первая vertical slice — widget surface + touch coordinates over local network. Cable support рассматривается как другой carrier. Camera/mic/display follow only after OS permission UX and threat model.
