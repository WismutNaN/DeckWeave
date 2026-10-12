// SPDX-License-Identifier: GPL-3.0-or-later
use deckweave_application::HostSnapshot;
use deckweave_host::HostRuntime;
use deckweave_platform_host::{LaunchOptions, installed_root};
use std::{path::Path, sync::Arc};
use tauri::{Emitter, Manager};
#[tauri::command]
pub(crate) fn host_snapshot(host: tauri::State<'_, Arc<HostRuntime>>) -> HostSnapshot {
    host.snapshot()
}

#[tauri::command]
pub(crate) fn autostart_enabled(host: tauri::State<'_, Arc<HostRuntime>>) -> Result<bool, String> {
    host.autostart_enabled().map_err(|error| error.to_string())
}

#[tauri::command]
pub(crate) fn set_autostart(
    enabled: bool,
    host: tauri::State<'_, Arc<HostRuntime>>,
) -> Result<(), String> {
    host.set_autostart(enabled)
        .map_err(|error| error.to_string())
}

pub(crate) fn show_main(app: &tauri::AppHandle) -> tauri::Result<()> {
    if let Some(window) = app.get_webview_window("main") {
        window.unminimize()?;
        window.show()?;
        window.set_focus()?;
    }
    Ok(())
}

pub(crate) fn quit(app: &tauri::AppHandle) {
    if let Some(runtime) =
        app.try_state::<Arc<deckweave_plugin_supervisor::host::ProductionRuntimeCoordinator>>()
        && let Err(error) = runtime.shutdown()
    {
        eprintln!("Не удалось завершить plugin runtime: {error}");
    }
    app.state::<Arc<HostRuntime>>().stop();
    app.exit(0);
}

#[tauri::command]
pub(crate) fn quit_app(app: tauri::AppHandle) {
    quit(&app);
}

pub(crate) fn second_instance(app: &tauri::AppHandle, args: Vec<String>, cwd: String) {
    let result = (|| -> Result<Option<String>, Box<dyn std::error::Error>> {
        let host = app
            .try_state::<Arc<HostRuntime>>()
            .ok_or("Host ещё запускается")?;
        let options = LaunchOptions::parse(args.iter().skip(1).map(Into::into))?;
        let executable = args.first().ok_or("Нет пути приложения")?;
        let requested =
            options.requested_root(Path::new(executable), Path::new(&cwd), &installed_root()?)?;
        let active = host.snapshot().data_root;
        // Несуществующий root не создаётся вторым запуском.
        let same_root = requested.canonicalize().is_ok_and(|root| root == active);
        Ok(if same_root {
            None
        } else {
            Some(format!(
                "Уже работает DeckWeave с данными в {}. Завершите его через «Выход», чтобы открыть {}.",
                active.display(),
                requested.display()
            ))
        })
    })();
    let notice = match result {
        Ok(notice) => notice,
        Err(error) => Some(error.to_string()),
    };
    if let Some(host) = app.try_state::<Arc<HostRuntime>>() {
        host.set_notice(notice);
        if let Err(error) = app.emit("host-changed", ()) {
            eprintln!("{error}");
        }
    }
    if let Err(error) = show_main(app) {
        eprintln!("Не удалось открыть окно: {error}");
    }
}
