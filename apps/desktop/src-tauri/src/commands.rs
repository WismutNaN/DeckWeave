// SPDX-License-Identifier: GPL-3.0-or-later
#[cfg(feature = "smoke-test")]
use crate::smoke;
use crate::{ipc, lifecycle::*};
pub(crate) fn handler() -> fn(tauri::ipc::Invoke<tauri::Wry>) -> bool {
    #[cfg(not(feature = "smoke-test"))]
    let handler: fn(tauri::ipc::Invoke<tauri::Wry>) -> bool = tauri::generate_handler![
        host_snapshot,
        autostart_enabled,
        set_autostart,
        quit_app,
        ipc::app_query,
        ipc::app_command,
        ipc::mutation_window,
        ipc::open_workspace,
        ipc::locale_get,
        ipc::locale_change,
        ipc::locale_reconcile
    ];
    #[cfg(feature = "smoke-test")]
    let handler: fn(tauri::ipc::Invoke<tauri::Wry>) -> bool = tauri::generate_handler![
        host_snapshot,
        autostart_enabled,
        set_autostart,
        quit_app,
        ipc::app_query,
        ipc::app_command,
        ipc::mutation_window,
        ipc::open_workspace,
        ipc::locale_get,
        ipc::locale_change,
        ipc::locale_reconcile,
        smoke::ready
    ];
    handler
}
