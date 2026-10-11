// SPDX-License-Identifier: GPL-3.0-or-later
//! Включается только feature smoke-test; отсутствует в обычной сборке.
use deckweave_application::ApplicationService;
use deckweave_host::HostRuntime;
use std::{
    path::PathBuf,
    sync::{
        Mutex,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    time::{Duration, Instant},
};
use tauri::{
    Manager, Wry,
    plugin::{Builder, TauriPlugin},
};

#[derive(Default)]
pub(crate) struct SmokeState {
    ready: AtomicBool,
    detail: Mutex<String>,
}

#[tauri::command]
pub(crate) fn ready(state: tauri::State<'_, SmokeState>, rendered: bool, detail: String) {
    *state.detail.lock().unwrap_or_else(|e| e.into_inner()) = detail;
    state.ready.store(rendered, Ordering::Release);
}

pub fn plugin() -> TauriPlugin<Wry> {
    Builder::new("smoke")
        .setup(|app, _| {
            app.manage(SmokeState::default());
            Ok(())
        })
        .on_page_load(|window, payload| {
            if std::env::var_os("DECKWEAVE_SMOKE_RESULT").is_none()
                || !matches!(payload.event(), tauri::webview::PageLoadEvent::Finished)
            {
                return;
            }
            let script = r#"
                (() => {
                    let attempts = 0;
                    const check = () => {
                        const detail = document.body?.innerText ?? 'No document body';
                        const rendered = detail.includes('Host работает');
                        if (rendered || ++attempts === 200) {
                            window.__TAURI_INTERNALS__.invoke('ready', { rendered, detail });
                        } else { setTimeout(check, 100); }
                    };
                    check();
                })();
            "#;
            if let Err(error) = window.eval(script) {
                eprintln!("smoke eval: {error}");
            }
        })
        .build()
}

fn on_main<T: Send + 'static>(
    app: &tauri::AppHandle,
    action: impl FnOnce(tauri::AppHandle) -> Result<T, String> + Send + 'static,
) -> Result<T, String> {
    let (sender, receiver) = mpsc::sync_channel(1);
    let handle = app.clone();
    app.run_on_main_thread(move || {
        let _ = sender.send(action(handle));
    })
    .map_err(|e| e.to_string())?;
    receiver
        .recv_timeout(Duration::from_secs(10))
        .map_err(|e| e.to_string())?
}

fn second(root: &std::path::Path) -> Result<(), String> {
    let mut command =
        std::process::Command::new(std::env::current_exe().map_err(|e| e.to_string())?);
    command
        .args(["--portable", "--data-root"])
        .arg(root)
        .env_remove("DECKWEAVE_SMOKE_RESULT");
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000);
    }
    let mut child = command.spawn().map_err(|e| e.to_string())?;
    let deadline = Instant::now() + Duration::from_secs(15);
    loop {
        if let Some(status) = child.try_wait().map_err(|e| e.to_string())? {
            return if status.success() {
                Ok(())
            } else {
                Err(format!("Второй запуск: {status}"))
            };
        }
        if Instant::now() > deadline {
            let _ = child.kill();
            let _ = child.wait();
            return Err("Второй process не завершился".into());
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

fn verify(app: &tauri::AppHandle) -> Result<(), String> {
    let deadline = Instant::now() + Duration::from_secs(30);
    while !app.state::<SmokeState>().ready.load(Ordering::Acquire) {
        if Instant::now() > deadline {
            let state = app.state::<SmokeState>();
            let detail = state
                .detail
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .clone();
            return Err(format!(
                "WebView не показал состояние Host через IPC: {detail}"
            ));
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    let root = app.state::<HostRuntime>().snapshot().data_root;
    on_main(app, |app| {
        if app.tray_by_id("main-tray").is_none() {
            return Err("Нет tray icon".into());
        }
        let window = app.get_webview_window("main").ok_or("Нет окна")?;
        window.close().map_err(|e| e.to_string())
    })?;
    // CloseRequested обрабатывается следующей итерацией native event loop.
    std::thread::sleep(Duration::from_millis(200));
    on_main(app, |app| {
        let window = app
            .get_webview_window("main")
            .ok_or("Окно уничтожено вместо hide")?;
        if window.is_visible().map_err(|e| e.to_string())? {
            return Err("Close не скрыл окно".into());
        }
        Ok(())
    })?;
    second(&root)?;
    on_main(app, |app| {
        if !app
            .get_webview_window("main")
            .ok_or("Нет окна")?
            .is_visible()
            .map_err(|e| e.to_string())?
        {
            return Err("Второй запуск не открыл окно".into());
        }
        if app.state::<HostRuntime>().snapshot().notice.is_some() {
            return Err("Ложный конфликт того же root".into());
        }
        Ok(())
    })?;
    let other = root.join("rejected-other-root");
    second(&other)?;
    if other.exists() {
        return Err("Второй запуск создал конфликтующий root".into());
    }
    if app.state::<HostRuntime>().snapshot().notice.is_none() {
        return Err("Другой root не показал конфликт".into());
    }
    Ok(())
}

pub fn start(app: tauri::AppHandle) {
    let Some(path) = std::env::var_os("DECKWEAVE_SMOKE_RESULT").map(PathBuf::from) else {
        return;
    };
    std::thread::spawn(move || {
        let result = verify(&app);
        let (report, code) = match result {
            Ok(()) => ("PASS: WebView IPC; tray; close-to-tray; same-root focus; different-root conflict; explicit exit\n".to_owned(), 0),
            Err(error) => (format!("FAIL: {error}\n"), 1)
        };
        if let Err(error) = std::fs::write(path, report) {
            eprintln!("smoke result: {error}");
            app.exit(1);
            return;
        }
        if code == 0 {
            if let Err(error) = on_main(&app, |app| {
                super::quit(&app);
                Ok(())
            }) {
                eprintln!("{error}");
            }
        } else {
            app.exit(code);
        }
    });
}
