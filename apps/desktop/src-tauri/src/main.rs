// SPDX-License-Identifier: GPL-3.0-or-later
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use deckweave_host::HostRuntime;
use deckweave_platform_host::{LaunchOptions, NativePlatform};
use std::sync::Arc;
use tauri::{
    Manager,
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
};

mod bundled;
mod commands;
mod composition;
mod editor_seed;
mod ipc;

mod lifecycle;
use lifecycle::*;

fn run(options: LaunchOptions) -> Result<(), Box<dyn std::error::Error>> {
    let builder = tauri::Builder::default()
        // Первым: второй executable завершается до bootstrap и writer locks.
        .plugin(tauri_plugin_single_instance::init(second_instance))
        .invoke_handler(commands::handler())
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
            app.manage(services.editor);

            // WebView cache принадлежит root; portable не пишет его в installed data directory.
            let window = tauri::WebviewWindowBuilder::new(
                app,
                "main",
                tauri::WebviewUrl::App("index.html".into()),
            )
            // WebView2 needs HTML5 DnD for action/control transfers inside the editor.
            .disable_drag_drop_handler()
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
