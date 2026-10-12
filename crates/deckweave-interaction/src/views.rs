// SPDX-License-Identifier: GPL-3.0-or-later
//! Persistent view leases are independent of short interactions and plugin documents.
use crate::presentation::{DesktopViewSession, PresentationError, PresentationFence};
use deckweave_domain::ids::PluginInstanceId;
use std::collections::BTreeMap;

#[derive(Default)]
pub struct DesktopViews {
    views: BTreeMap<String, DesktopViewSession>,
}

impl DesktopViews {
    pub fn open(&mut self, session: DesktopViewSession, now: u64) -> Result<(), PresentationError> {
        if session.view_id.is_empty()
            || session.view_id.len() > 256
            || session.fence.revision == 0
            || session.lease_expires_host_ticks <= now
            || session.lease_expires_host_ticks.saturating_sub(now) > 3_600_000
            || session
                .document_id
                .as_ref()
                .is_some_and(|id| id.is_empty() || id.len() > 512)
        {
            return Err(PresentationError::Invalid);
        }
        if let Some(old) = self.views.get(&session.view_id) {
            if old.instance_id != session.instance_id
                || (
                    session.fence.activation_epoch,
                    session.fence.provider_generation,
                    session.fence.revision,
                ) <= (
                    old.fence.activation_epoch,
                    old.fence.provider_generation,
                    old.fence.revision,
                )
            {
                return Err(PresentationError::Stale);
            }
        } else if self.views.len() >= 128 {
            return Err(PresentationError::Capacity);
        }
        self.views.insert(session.view_id.clone(), session);
        Ok(())
    }
    pub fn close(
        &mut self,
        id: &str,
        fence: &PresentationFence,
    ) -> Result<DesktopViewSession, PresentationError> {
        let session = self.views.get(id).ok_or(PresentationError::Missing)?;
        if &session.fence != fence {
            return Err(PresentationError::Stale);
        }
        self.views.remove(id).ok_or(PresentationError::Missing)
    }
    pub fn renew(
        &mut self,
        id: &str,
        fence: &PresentationFence,
        deadline: u64,
        now: u64,
    ) -> Result<(), PresentationError> {
        let session = self.views.get_mut(id).ok_or(PresentationError::Missing)?;
        if !session.accepts(fence, now) {
            return Err(PresentationError::Stale);
        }
        if deadline <= now || deadline.saturating_sub(now) > 3_600_000 {
            return Err(PresentationError::Invalid);
        }
        session.lease_expires_host_ticks = deadline;
        Ok(())
    }
    pub fn expire(&mut self, now: u64) -> Vec<DesktopViewSession> {
        self.remove_matching(|s| s.lease_expires_host_ticks <= now)
    }
    pub fn revoke_instance(&mut self, id: &PluginInstanceId) -> Vec<DesktopViewSession> {
        self.remove_matching(|s| &s.instance_id == id)
    }
    fn remove_matching(
        &mut self,
        matches: impl Fn(&DesktopViewSession) -> bool,
    ) -> Vec<DesktopViewSession> {
        let ids: Vec<_> = self
            .views
            .values()
            .filter(|s| matches(s))
            .map(|s| s.view_id.clone())
            .collect();
        ids.into_iter()
            .filter_map(|id| self.views.remove(&id))
            .collect()
    }
}
