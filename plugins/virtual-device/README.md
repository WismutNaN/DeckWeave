# Bundled virtual devices

`org.deckweave.virtual` — first-party memory-only provider for the normal desktop build. It uses the same package manifest, hashes, handshake, supervisor process, activation barrier and wire routing as external plugins.

- `devices`: Virtual Deck A and B, 15 press/release controls each, writable text and observed text readback. Geometry is descriptor metadata.
- `text`: echo action with independent configured instances (`prefix`), typed input/result and a 2048-byte UTF-8 bound. An unknown instance never falls back to the default account.
- No OS, hardware or network permissions. Heartbeats and owner cleanup use the shared protocol. This package is not a Surface renderer or a universal device simulator.

From the repository root, `npm run prepare:desktop` builds a debug package; `npm run dev` and `npm run build` prepare the appropriate bundle automatically. Keep `bundled/virtual-device` next to the desktop executable. On first launch the host validates and stages it in the data-root. Existing pinned bytes are preserved; updating installed packages belongs to the packaging phase.

`metadata.mjs` is the descriptor/catalog/schema source. `assets/descriptors.json` is checked in for Rust builds; `npm run check:generated` detects drift. See [desktop acceptance](../../docs/implementation/phase-6-desktop.md) and [development commands](../../docs/implementation/development.md).
