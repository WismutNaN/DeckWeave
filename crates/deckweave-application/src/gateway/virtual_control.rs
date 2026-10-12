// SPDX-License-Identifier: GPL-3.0-or-later
use super::*;
use crate::virtual_control::*;
impl ApplicationGateway {
    #[must_use]
    pub fn with_virtual_control(mut self, port: Arc<dyn VirtualControlPort>) -> Self {
        self.virtual_control = port;
        self
    }
    pub(super) fn query_virtual_surface(
        &self,
        context: &RequestContext,
    ) -> Result<AppQueryResult, ApiError> {
        self.authorize(context, ApiAuthScope::Read)?;
        self.virtual_control
            .snapshot()
            .map(AppQueryResult::VirtualSurface)
            .map_err(|e| map_error(context, e))
    }
    pub(super) fn click_virtual(
        &self,
        context: &RequestContext,
        request: &VirtualClickRequest,
        dry_run: bool,
        replayed: bool,
    ) -> Result<AppCommandResult, ApiError> {
        if dry_run {
            return Err(api_error(
                context,
                ApiErrorCode::InvalidCommand,
                "api.error.virtual_live_only",
                false,
                BTreeMap::new(),
            ));
        }
        // Claim precedes effects. A repeated claim can mean a crash after dispatch; never inject again.
        if replayed {
            return Ok(AppCommandResult::VirtualClick {
                accepted: false,
                replayed: true,
            });
        }
        self.virtual_control
            .click(request)
            .map_err(|e| map_error(context, e))?;
        Ok(AppCommandResult::VirtualClick {
            accepted: true,
            replayed: false,
        })
    }
}
fn map_error(context: &RequestContext, error: VirtualControlError) -> ApiError {
    let (code, key) = match error {
        VirtualControlError::Stale => (ApiErrorCode::RevisionConflict, "api.error.virtual_stale"),
        VirtualControlError::Unsupported => (
            ApiErrorCode::InvalidCommand,
            "api.error.virtual_unsupported",
        ),
        VirtualControlError::Unavailable => {
            (ApiErrorCode::Unavailable, "api.error.virtual_unavailable")
        }
    };
    api_error(context, code, key, false, BTreeMap::new())
}
