// SPDX-License-Identifier: GPL-3.0-or-later
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use deckweave_application::HostSnapshot;
use deckweave_host::HostRuntime;
use deckweave_platform_host::{LaunchOptions, NativePlatform, installed_root};
use std::{path::Path, sync::Arc};
use tauri::{
    Emitter, Manager,
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
};

mod composition;

#[tauri::command]
fn host_snapshot(host: tauri::State<'_, Arc<HostRuntime>>) -> HostSnapshot {
    host.snapshot()
}

#[tauri::command]
fn autostart_enabled(host: tauri::State<'_, Arc<HostRuntime>>) -> Result<bool, String> {
    host.autostart_enabled().map_err(|error| error.to_string())
}

#[tauri::command]
fn set_autostart(enabled: bool, host: tauri::State<'_, Arc<HostRuntime>>) -> Result<(), String> {
    host.set_autostart(enabled)
        .map_err(|error| error.to_string())
}

fn show_main(app: &tauri::AppHandle) -> tauri::Result<()> {
    if let Some(window) = app.get_webview_window("main") {
        window.unminimize()?;
        window.show()?;
        window.set_focus()?;
    }
    Ok(())
}

fn quit(app: &tauri::AppHandle) {
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
fn quit_app(app: tauri::AppHandle) {
    quit(&app);
}

fn second_instance(app: &tauri::AppHandle, args: Vec<String>, cwd: String) {
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

fn run(options: LaunchOptions) -> Result<(), Box<dyn std::error::Error>> {
    #[cfg(not(feature = "smoke-test"))]
    let handler: fn(tauri::ipc::Invoke<tauri::Wry>) -> bool =
        tauri::generate_handler![host_snapshot, autostart_enabled, set_autostart, quit_app];
    #[cfg(feature = "smoke-test")]
    let handler: fn(tauri::ipc::Invoke<tauri::Wry>) -> bool = tauri::generate_handler![
        host_snapshot,
        autostart_enabled,
        set_autostart,
        quit_app,
        smoke::ready
    ];
    let builder = tauri::Builder::default()
        // Первым: второй executable завершается до bootstrap и writer locks.
        .plugin(tauri_plugin_single_instance::init(second_instance))
        .invoke_handler(handler)
        .setup(move |app| {
            let platform = NativePlatform::new(
                &options,
                std::env::current_exe()?,
                &std::env::current_dir()?,
            )?;
            let root = deckweave_application::HostPlatform::data_root(&platform).to_owned();
            let host = Arc::new(HostRuntime::start(Arc::new(platform))?);
            let services = composition::compose(&root, Arc::clone(&host))?;
            app.manage(host);
            app.manage(services.runtime);
            app.manage(services.gateway);
            app.manage(services.locale);

            // WebView cache принадлежит root; portable не пишет его в installed data directory.
            let window = tauri::WebviewWindowBuilder::new(
                app,
                "main",
                tauri::WebviewUrl::App("index.html".into()),
            )
            .title("DeckWeave")
            .inner_size(1160.0, 760.0)
            .min_inner_size(760.0, 520.0)
            .visible(false)
            .data_directory(root.join("webview"))
            .build()?;

            let open = MenuItem::with_id(app, "open", "Открыть DeckWeave", true, None::<&str>)?;
            let exit = MenuItem::with_id(app, "exit", "Выход", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&open, &exit])?;
            let icon = app
                .default_window_icon()
                .ok_or("Отсутствует иконка приложения")?
                .clone();
            TrayIconBuilder::with_id("main-tray")
                .icon(icon)
                .tooltip("DeckWeave — работает в фоне")
                .menu(&menu)
                .show_menu_on_left_click(false)
                .on_menu_event(|app, event| match event.id.as_ref() {
                    "open" => {
                        if let Err(error) = show_main(app) {
                            eprintln!("{error}");
                        }
                    }
                    "exit" => quit(app),
                    _ => {}
                })
                .on_tray_icon_event(|tray, event| {
                    if matches!(
                        event,
                        TrayIconEvent::Click {
                            button: MouseButton::Left,
                            button_state: MouseButtonState::Up,
                            ..
                        }
                    ) && let Err(error) = show_main(tray.app_handle())
                    {
                        eprintln!("{error}");
                    }
                })
                .build(app)?;
            if !options.background {
                window.show()?;
            }
            #[cfg(feature = "smoke-test")]
            smoke::start(app.handle().clone());
            Ok(())
        })
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                // При ошибке скрытия не блокируем штатное закрытие.
                if window.hide().is_ok() {
                    api.prevent_close();
                }
            }
        });
    #[cfg(feature = "smoke-test")]
    let builder = builder.plugin(smoke::plugin());
    let app = builder.build(tauri::generate_context!())?;
    app.run(|app, event| {
        if let tauri::RunEvent::Exit = event {
            if let Some(runtime) = app
                .try_state::<Arc<deckweave_plugin_supervisor::host::ProductionRuntimeCoordinator>>()
            {
                let _ignored = runtime.shutdown();
            }
            app.state::<Arc<HostRuntime>>().stop();
        }
    });
    Ok(())
}

fn main() {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.iter().any(|arg| arg == "--help") {
        println!(
            "DeckWeave\n  --portable  Данные рядом с executable, без автозапуска\n  --data-root PATH  Явный каталог данных\n  --background  Начать в трее"
        );
        return;
    }
    let result = LaunchOptions::parse(args)
        .map_err(|error| -> Box<dyn std::error::Error> { error.into() })
        .and_then(run);
    if let Err(error) = result {
        eprintln!("DeckWeave: {error}");
        std::process::exit(1);
    }
}

#[cfg(feature = "smoke-test")]
mod smoke;
