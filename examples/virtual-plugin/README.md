# DeckWeave virtual plugin example

Минимальный permissive native plugin, зависящий только от permissive
`deckweave-protocol`. Он демонстрирует:

- stdio handshake с одноразовым session token;
- публикацию виртуальной кнопки и простого action descriptor;
- heartbeat;
- синхронный typed terminal result для invocation;
- корректную деактивацию.

Host передаёт identity и limits через переменные окружения, а control messages —
как length-delimited Protobuf в stdin/stdout. В stdout нельзя писать логи: для
диагностики используется stderr.

Это reference source, а не готовый установленный package. Packaging tool позже
собирает platform artifact, вычисляет SHA-256 и создаёт неизменяемый manifest.
