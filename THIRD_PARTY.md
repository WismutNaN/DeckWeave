# Источники и зависимости

Фазы 1–2 не включают vendored код драйверов, OpenDeck или других приложений.
Использованы опубликованные API библиотек; реализации DeckWeave написаны в этом репозитории.
Точные версии, registry sources и checksums Rust-зависимостей находятся в [Cargo.lock](Cargo.lock),
JavaScript-зависимостей — в [package-lock.json](package-lock.json).

| Зависимость / инструмент | Источник | Использование |
|---|---|---|
| Tauri 2, tauri-build, single-instance | [tauri-apps](https://github.com/tauri-apps) | Окно, WebView, трей и передача второго запуска первому |
| Svelte, Vite | [Svelte](https://github.com/sveltejs/svelte), [Vite](https://github.com/vitejs/vite) | UI и frontend build |
| auto-launch | [auto-launch](https://github.com/zzzgydi/auto-launch) | Non-Windows adapter автозапуска; desktop acceptance этих ОС отложен |
| directories | [directories-rs](https://github.com/dirs-dev/directories-rs) | Пользовательский каталог данных |
| fs2 | [fs2](https://github.com/danburkert/fs2-rs) | OS-backed файловые блокировки; не PID-файлы |
| windows, windows-core, windows-sys, windows-registry | [windows-rs](https://github.com/microsoft/windows-rs) | Core Audio, Raw Input, media/window APIs, Host STA Shell broker, Windows session и HKCU-регистрация автозапуска |
| serde, thiserror | [serde](https://github.com/serde-rs/serde), [thiserror](https://github.com/dtolnay/thiserror) | Application snapshot и типизированные ошибки |
| Prost, protoc-bin-vendored | [tokio-rs/prost](https://github.com/tokio-rs/prost), [protobuf-src/protoc-bin-vendored](https://github.com/protobuf-src/protoc-bin-vendored) | Rust Protobuf types и воспроизводимый protoc для control protocol |
| Buf CLI, Protobuf-ES | [bufbuild/buf](https://github.com/bufbuild/buf), [bufbuild/protobuf-es](https://github.com/bufbuild/protobuf-es) | Lint `.proto` и TypeScript wire types; install scripts разрешены только для закреплённых версий |
| Typify | [oxidecomputer/typify](https://github.com/oxidecomputer/typify) | Rust document types из JSON Schema; schema-valued `additionalProperties` сохраняют расширения |
| JSON Schema to TypeScript, Ajv | [bcherny/json-schema-to-typescript](https://github.com/bcherny/json-schema-to-typescript), [ajv-validator/ajv](https://github.com/ajv-validator/ajv) | TypeScript document types и Draft 2020-12 fixture validation |
| tempfile | [tempfile](https://github.com/Stebalien/tempfile) | Только тестовые каталоги |
| github-slugger | [github-slugger](https://github.com/Flet/github-slugger) | Проверка Markdown anchors |
| cargo-deny | [cargo-deny](https://github.com/EmbarkStudios/cargo-deny) | CI-проверка лицензий, источников и advisories |

Иконка приложения создана для DeckWeave как [SVG](ui/desktop/public/deckweave.svg).
PNG/ICO получены штатной командой Tauri icon; исходники изображений сторонних проектов не использовались.

Тексты GPL-3.0 и Apache-2.0 получены из [SPDX License List Data](https://github.com/spdx/license-list-data/tree/main/text).
Корневой LICENSE задаёт GPL-3.0-or-later для приложения; permissive protocol имеет собственные LICENSE-MIT и LICENSE-APACHE.
Реестр зависимостей не заменяет индивидуальные notices при будущем переносе исходного кода.

При добавлении vendored фрагмента записываются: repository URL, commit, paths, license/notices, изменения,
стратегия обновления и тест. Изменение состава dependencies фиксируется lockfile и проходит cargo-deny.
Для npm `check-boundaries` проверяет известные license expressions, registry.npmjs.org и sha512 integrity.
Проверены составные выражения `(Apache-2.0 AND BSD-3-Clause)` у `@bufbuild/protobuf` и
`Python-2.0` у транзитивного `argparse`; оба являются разрешительными лицензиями, исходный код в DeckWeave не копировался.

## Проверенные исключения dependency policy

- `target-lexicon 0.12.16`, транзитивная build dependency GTK, использует
  `Apache-2.0 WITH LLVM-exception`. Это выражение явно разрешено в deny.toml;
  текст исключения — [SPDX](https://spdx.org/licenses/LLVM-exception.html).
- `proc-macro-error 1.0.4` приходит через `glib-macros`/`gtk3-macros` в Linux desktop ветку Tauri 2.
  [RUSTSEC-2024-0370](https://rustsec.org/advisories/RUSTSEC-2024-0370.html) — уведомление **unmaintained**,
  без исправленной версии; это не свидетельство отсутствия рисков. Для скелета от 2026-10-12 принято
  точечное исключение этого ID. Перед Linux desktop milestone и при обновлении Tauri его нужно пересмотреть.
  Остальные advisories, неизвестные licenses/registries и wildcard dependencies продолжают блокировать CI.
