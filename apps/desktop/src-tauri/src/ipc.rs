// SPDX-License-Identifier: GPL-3.0-or-later
//! Тонкий Tauri adapter; блокирующие use cases выполняются вне UI thread.
use deckweave_application::{
    editor::EditorWorkspaceService,
    locale::{LocalePreferencesService, LocaleUpdate},
    *,
};
use deckweave_domain::{ids::TraceId, localization::LocaleContext};
use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
};
use tauri::{State, WebviewWindow};
static REQUEST: AtomicU64 = AtomicU64::new(0);

fn error(code: ApiErrorCode, key: &str) -> ApiError {
    ApiError {
        code,
        message_key: key.into(),
        details: Default::default(),
        retryable: false,
        trace_id: TraceId::try_new("desktop-adapter").unwrap_or_else(|_| unreachable!()),
    }
}
fn context(window: &WebviewWindow) -> Result<RequestContext, ApiError> {
    if window.label() != "main" {
        return Err(error(
            ApiErrorCode::PermissionDenied,
            "api.error.permission_denied",
        ));
    }
    Ok(RequestContext {
        principal_id: PrincipalId::try_new("desktop-ui")
            .map_err(|_| error(ApiErrorCode::Internal, "api.error.identity"))?,
        trace_id: TraceId::try_new(format!(
            "desktop-{}",
            REQUEST.fetch_add(1, Ordering::Relaxed)
        ))
        .map_err(|_| error(ApiErrorCode::Internal, "api.error.trace"))?,
    })
}
async fn blocking<T: Send + 'static>(
    work: impl FnOnce() -> Result<T, ApiError> + Send + 'static,
) -> Result<T, ApiError> {
    tauri::async_runtime::spawn_blocking(work)
        .await
        .map_err(|_| error(ApiErrorCode::Internal, "api.error.worker"))?
}
#[tauri::command]
pub async fn app_query(
    window: WebviewWindow,
    service: State<'_, Arc<ApplicationGateway>>,
    query: AppQuery,
) -> Result<AppQueryResult, ApiError> {
    let request = context(&window)?;
    let service = service.inner().clone();
    blocking(move || service.query(&request, query)).await
}
#[tauri::command]
pub async fn app_command(
    window: WebviewWindow,
    service: State<'_, Arc<ApplicationGateway>>,
    envelope: CommandEnvelope,
) -> Result<AppCommandResult, ApiError> {
    let request = context(&window)?;
    let service = service.inner().clone();
    blocking(move || service.command(&request, envelope)).await
}
#[tauri::command]
pub async fn mutation_window(
    window: WebviewWindow,
    service: State<'_, Arc<ApplicationGateway>>,
) -> Result<MutationWindow, ApiError> {
    let request = context(&window)?;
    let service = service.inner().clone();
    blocking(move || service.open_mutation_window(&request)).await
}
#[tauri::command]
pub async fn open_workspace(
    window: WebviewWindow,
    service: State<'_, Arc<EditorWorkspaceService>>,
) -> Result<DraftView, ApiError> {
    let request = context(&window)?;
    let service = service.inner().clone();
    blocking(move || service.open(&request)).await
}
#[tauri::command]
pub async fn locale_get(
    window: WebviewWindow,
    service: State<'_, Arc<LocalePreferencesService>>,
) -> Result<LocaleContext, ApiError> {
    let request = context(&window)?;
    let service = service.inner().clone();
    blocking(move || {
        service
            .get(&request)
            .map_err(|error| error.into_api_error(&request))
    })
    .await
}
#[tauri::command]
pub async fn locale_change(
    window: WebviewWindow,
    service: State<'_, Arc<LocalePreferencesService>>,
    expected: u64,
    next: LocaleContext,
) -> Result<LocaleUpdate, ApiError> {
    let request = context(&window)?;
    let service = service.inner().clone();
    blocking(move || {
        service
            .change(&request, expected, next)
            .map_err(|error| error.into_api_error(&request))
    })
    .await
}

#[tauri::command]
pub async fn locale_reconcile(
    window: WebviewWindow,
    service: State<'_, Arc<LocalePreferencesService>>,
) -> Result<LocaleUpdate, ApiError> {
    let request = context(&window)?;
    let service = service.inner().clone();
    blocking(move || {
        service
            .reconcile(&request)
            .map_err(|error| error.into_api_error(&request))
    })
    .await
}

#[tauri::command]
pub async fn permissions_get(
    window: WebviewWindow,
    service: State<'_, Arc<permissions::PluginPermissionService>>,
) -> Result<permissions::PermissionSnapshot, ApiError> {
    let request = context(&window)?;
    let service = service.inner().clone();
    blocking(move || service.get(&request)).await
}
#[tauri::command]
pub async fn permissions_change(
    window: WebviewWindow,
    service: State<'_, Arc<permissions::PluginPermissionService>>,
    expected: u64,
    selected: Vec<PermissionSelection>,
) -> Result<permissions::PermissionSnapshot, ApiError> {
    let request = context(&window)?;
    let service = service.inner().clone();
    blocking(move || service.change(&request, expected, selected)).await
}
