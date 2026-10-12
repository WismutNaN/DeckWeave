// SPDX-License-Identifier: GPL-3.0-or-later
use super::*;
use crate::plugin_catalog::{PluginCatalogError, PluginCatalogRequest};

impl ApplicationGateway {
    #[must_use]
    pub fn with_editor_source(mut self, editor: Arc<dyn crate::editor::EditorReadSource>) -> Self {
        self.editor = editor;
        self
    }
    #[must_use]
    pub fn with_plugin_catalog(mut self, catalog: Arc<dyn PluginCatalogSource>) -> Self {
        self.catalog = catalog;
        self
    }
    pub(super) fn query_plugin_catalog(
        &self,
        context: &RequestContext,
        package: &PluginCatalogRequest,
    ) -> Result<AppQueryResult, ApiError> {
        self.authorize(context, ApiAuthScope::Read)?;
        if !package.is_bounded() {
            return Err(api_error(
                context,
                ApiErrorCode::InvalidCommand,
                "api.error.invalid_catalog_request",
                false,
                BTreeMap::new(),
            ));
        }
        self.catalog
            .catalog(package)
            .map(|catalog| AppQueryResult::PluginCatalog(Box::new(catalog)))
            .map_err(|error| {
                let (code, key, retryable) = match error {
                    PluginCatalogError::NotFound => {
                        (ApiErrorCode::NotFound, "api.error.package_not_found", false)
                    }
                    PluginCatalogError::InvalidCatalog => (
                        ApiErrorCode::InvalidCommand,
                        "api.error.invalid_catalog",
                        false,
                    ),
                    PluginCatalogError::Unavailable => (
                        ApiErrorCode::Unavailable,
                        "api.error.catalog_unavailable",
                        true,
                    ),
                };
                api_error(context, code, key, retryable, BTreeMap::new())
            })
    }
}
