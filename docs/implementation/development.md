# Разработка и запуск

Скелет фазы 1, contracts фазы 2 и plugin runtime фазы 3 используют Rust 1.98.1 из rust-toolchain.toml, Node.js 22.12+ и npm.
Windows требует MSVC Build Tools и WebView2 Runtime. Требования ОС: [документация Tauri](https://v2.tauri.app/start/prerequisites/).

## Установка зависимостей

Из корня репозитория:

```powershell
npm ci
rustup show active-toolchain
```

## Запуск

```powershell
npm run dev
```

Запуск portable-режима с явным root:

```powershell
npm run dev -- -- -- --portable --data-root G:\DeckWeave-data
```

У окна есть close-to-tray. Трей содержит «Открыть DeckWeave» и «Выход»; второй запуск с тем же root
открывает существующее окно. Другой root показывает конфликт, не вытесняя первый host.

Автозапуск выключен по умолчанию. Переключатель справа регистрирует текущий executable с
аргументами --background и --data-root только по явному действию пользователя.
Для ежедневного автозапуска используйте постоянное расположение release executable; dev-сборку можно переместить или удалить.
В portable-режиме регистрация недоступна.

Без --data-root installed host использует пользовательский data_local_dir/DeckWeave, portable —
DeckWeave-data рядом с executable. WebView cache находится в выбранном root.
Session lock находится в пользовательском DeckWeave/run независимо от portable root; это служебная блокировка,
профили разных roots не смешиваются. Сами lock-файлы могут оставаться после выхода; ОС освобождает блокировку при закрытии handle и crash.

## Сборка

```powershell
npm run build
```

Создаётся target/release/deckweave-desktop.exe. Installer пока не создаётся.
Для быстрой debug-сборки:

```powershell
npm run build -- --debug
```

## Проверки

После изменения `.proto` или JSON Schema обновите generated Rust/TypeScript boundary:

```powershell
npm run generate:contracts
npm run check:generated
```

Rust Protobuf types генерируются `prost-build` в `OUT_DIR`, Rust document types — `typify` при компиляции,
а отслеживаемые TypeScript types — Buf Protobuf-ES и `json-schema-to-typescript`.
`check:generated` повторяет генерацию и отклоняет нестабильный либо забытый diff.
Общий wire fixture `protocol/fixtures/wire/heartbeat-envelope.hex` читают обе реализации.

```powershell
npm run check
cargo fmt --all -- --check
cargo test --locked
cargo clippy --locked --all-targets -- -D warnings
cargo clippy --locked -p deckweave-desktop --all-targets --all-features -- -D warnings
```

cargo test по умолчанию тестирует платформенно-независимые crates и host services.
Пакеты desktop/GTK не входят в default-members, поэтому Linux core CI не требует desktop libraries.
Cargo.lock и package-lock.json обязательны.

Проверка dependency policies:

```powershell
cargo install cargo-deny --version 0.20.2 --locked --root .local/tools
.local/tools/bin/cargo-deny check licenses sources bans advisories
```

## Проверка настоящего desktop lifecycle

Завершите другой DeckWeave через «Выход». Тест не вытесняет работающий host.

```powershell
npm run build:web
cargo build --locked -p deckweave-desktop --features smoke-test
./scripts/smoke-desktop.ps1
```

Тест запускает настоящий Tauri/WebView2, проверяет frontend → host IPC, наличие tray, hide on close,
восстановление окна вторым запуском и конфликт другого root, затем выполняет тот же путь выхода, что tray menu.
Результат и stderr сохраняются в test-results. Тестовые hooks отсутствуют в обычной сборке.
PowerShell harness также проверяет отсутствие WebView2 processes с уникальным тестовым data-root после выхода.

ОС освобождение writer lock после принудительного завершения дочернего процесса проверяется
отдельным Rust integration test. Plugin runtime использует Windows Job Object или Unix process group;
fault fixture дополнительно создаёт grandchild и проверяет отсутствие orphan после terminate.
Hardware и API listener пока не запускаются.

## Границы реализации

В фазах 1–3 доступны host lifecycle, каталоги, блокировки, opt-in автозапуск, read-only HostSnapshot,
versioned contracts, статический package index, isolated native runtime и Capability Registry.
Главный экран показывает состояние настоящего host, но workspace, bindings и список plugins в UI пока не реализованы.
Control envelope, framing helpers и document schemas зафиксированы как эволюционирующая граница;
полный SDK ещё не стабилизирован.
Remote/media пока представлены только descriptors; transport и raw buffer ABI намеренно отсутствуют.

Permissive reference plugin находится в `examples/virtual-plugin`; он зависит только от
`deckweave-protocol`. OpenAction C0 scanner и pinned fixture находятся в `runtimes/openaction-host`
и `fixtures/compatibility`. C1 WebSocket translation не реализована и не смешивается с native stdio ABI.

Правила зависимостей проверяет scripts/check-boundaries.mjs согласно
[ADR-0013](../decisions/0013-dependency-and-host-service-boundaries.md).
Новые contracts добавляются по [плану](roadmap.md), не как private Tauri-only features.

## Локальная проверка 2026-10-12

- Windows: debug executable собран через Tauri CLI; Svelte/TypeScript — 0 ошибок и предупреждений.
- Проверен также dev-запуск с Vite и portable arguments; native smoke проходит и в этом режиме, dev server завершается вместе с запуском.
- Rust: platform-host, protocol, registry, supervisor, virtual example и OpenAction scanner компилируются и проходят unit/integration tests.
- Настоящий Tauri/WebView2 smoke: IPC, tray, close-to-tray, same-root focus, different-root conflict, общий Exit handler и отсутствие WebView2 orphans.
- Plugin runtime: staging/index без исполнения, pre-launch hash recheck, bounded Protobuf stdio,
  negotiated limits, heartbeat/start/stop deadlines, discovery expiry, restart/backoff/circuit breaker,
  grant revoke, session invalidation и process-tree cleanup.
- Pinned OpenAction Counter manifest проходит C0 scanner; C1 отмечается недоступным без platform artifact и отдельного WebSocket host.
- Format/Clippy, docs-check, Node tests, package boundaries и license/provenance checks проходят.
- cargo-deny проходит с одним документированным исключением unmaintained GTK build dependency в [THIRD_PARTY](../../THIRD_PARTY.md#проверенные-исключения-dependency-policy).

Пока не проверены: ручной клик Exit в native tray menu, реальный автозапуск после входа Windows,
Linux/macOS desktop, release packaging и удалённые GitHub Actions. Никаких регистраций автозапуска
при разработке и тестах не выполнялось. В Windows переключатель отражает HKCU-регистрацию текущего
executable/root; системный Task Manager может дополнительно запретить её исполнение. DeckWeave не снимает этот запрет.
Non-Windows autostart adapter требует platform acceptance в соответствующем milestone.
