// SPDX-License-Identifier: GPL-3.0-or-later
use super::*;
impl RuntimeController {
    pub(crate) fn granted_request(&self, scope: &str) -> Option<&PermissionRequest> {
        self.grants
            .iter()
            .map(|grant| &grant.request)
            .find(|request| request.scope.as_str() == scope)
    }
    pub fn accept_envelope(
        &mut self,
        envelope: &ProtocolEnvelope,
        now_host_ticks: u64,
    ) -> Result<(), SupervisorError> {
        self.validate_envelope(envelope)?;
        let session = self
            .session
            .as_mut()
            .ok_or(SupervisorError::MissingSession)?;
        session.last_heartbeat_host_ticks = now_host_ticks;
        Ok(())
    }

    pub fn validate_envelope(&self, envelope: &ProtocolEnvelope) -> Result<(), SupervisorError> {
        let session = self
            .session
            .as_ref()
            .ok_or(SupervisorError::MissingSession)?;
        if envelope.session_generation != session.identity.generation.get() {
            return Err(SupervisorError::LateSessionMessage);
        }
        // The base v1 envelope remains valid after additive negotiation. A
        // newer minor or different major cannot acquire semantics implicitly.
        if !session
            .negotiated_protocol
            .as_ref()
            .zip(envelope.wire_version.as_ref())
            .is_some_and(|(selected, wire)| {
                wire.major == selected.major && wire.minor <= selected.minor
            })
        {
            return Err(SupervisorError::InvalidWireVersion);
        }
        Ok(())
    }
}
