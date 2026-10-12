// SPDX-License-Identifier: GPL-3.0-or-later
//! Общий read model редактора и идемпотентное открытие рабочего черновика.
use crate::gateway::errors::{api_error, map_store_error};
use crate::{
    ApiAuthScope, ApiAuthorizationError, ApiAuthorizer, ApiError, ApiErrorCode, RequestContext,
};
use deckweave_domain::{capability::NodeDescriptor, ids::PackageId};
use deckweave_workspace::{
    ActiveSelection, DraftRepository, DraftRepositoryError, DraftView, WorkspaceDraft,
};
use serde::Serialize;
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstalledPluginView {
    pub package: crate::plugin_catalog::PluginCatalogRequest,
    pub manifest_sha256: String,
    pub templates: Vec<NodeDescriptor>,
}
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EditorNodeView {
    pub package_id: PackageId,
    pub generation: u64,
    pub descriptor: NodeDescriptor,
}
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EditorRuntimeView {
    pub package_id: PackageId,
    pub status: String,
    pub detail: Option<String>,
}
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EditorSnapshot {
    pub registry_revision: u64,
    pub installed: Vec<InstalledPluginView>,
    pub nodes: Vec<EditorNodeView>,
    pub runtimes: Vec<EditorRuntimeView>,
    pub active: Option<ActiveSelection>,
}
pub trait EditorReadSource: Send + Sync {
    fn editor_snapshot(&self) -> Result<EditorSnapshot, String>;
}
pub struct UnavailableEditorSource;
impl EditorReadSource for UnavailableEditorSource {
    fn editor_snapshot(&self) -> Result<EditorSnapshot, String> {
        Err("editor source unavailable".into())
    }
}
pub struct EditorWorkspaceService {
    repository: Arc<dyn DraftRepository>,
    authorizer: Arc<dyn ApiAuthorizer>,
    initial: WorkspaceDraft,
    order: Mutex<()>,
}
impl EditorWorkspaceService {
    pub fn new(
        repository: Arc<dyn DraftRepository>,
        authorizer: Arc<dyn ApiAuthorizer>,
        initial: WorkspaceDraft,
    ) -> Self {
        Self {
            repository,
            authorizer,
            initial,
            order: Mutex::new(()),
        }
    }
    /// Ensure имеет устойчивый ID: повторное открытие возвращает текущую revision.
    /// Это не query и не повтор прежней document mutation.
    pub fn open(&self, context: &RequestContext) -> Result<DraftView, ApiError> {
        self.authorizer
            .authorize(&context.principal_id, ApiAuthScope::Draft)
            .map_err(|error| match error {
                ApiAuthorizationError::Denied => api_error(
                    context,
                    ApiErrorCode::PermissionDenied,
                    "api.error.permission_denied",
                    false,
                    Default::default(),
                ),
                ApiAuthorizationError::Unavailable(_) => api_error(
                    context,
                    ApiErrorCode::Unavailable,
                    "api.error.authorization_unavailable",
                    true,
                    Default::default(),
                ),
            })?;
        let _order = self.order.lock().map_err(|e| {
            map_store_error(context, DraftRepositoryError::Unavailable(e.to_string()))
        })?;
        match self.repository.get_draft(&self.initial.draft_id) {
            Err(DraftRepositoryError::NotFound) => self.repository.ensure_draft(&self.initial),
            result => result,
        }
        .map_err(|error| map_store_error(context, error))
    }
}
