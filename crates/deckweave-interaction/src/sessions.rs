// SPDX-License-Identifier: GPL-3.0-or-later
use crate::{
    CancelReason, InteractionPage, InteractionRequest, InteractionResult, InteractionSession,
};
use deckweave_domain::ids::{InteractionSessionId, InvocationId};
use std::collections::BTreeMap;
use thiserror::Error;

const MAXIMUM_PAGE_ITEMS: usize = 100;

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum InteractionError {
    #[error("interaction session already exists")]
    DuplicateSession,
    #[error("interaction session was not found")]
    NotFound,
    #[error("interaction session is already terminal")]
    AlreadyTerminal,
    #[error("interaction update belongs to another session")]
    SessionMismatch,
    #[error("interaction page exceeds 100 items")]
    PageTooLarge,
    #[error("interaction result schema is empty")]
    EmptyResultSchema,
    #[error("interaction adapter failed: {0}")]
    Adapter(String),
}

#[derive(Default)]
pub struct InteractionSessions {
    sessions: BTreeMap<InteractionSessionId, InteractionSession>,
}

impl InteractionSessions {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn open(&mut self, request: InteractionRequest) -> Result<(), InteractionError> {
        if request.result_schema.trim().is_empty() {
            return Err(InteractionError::EmptyResultSchema);
        }
        if self.sessions.contains_key(&request.session_id) {
            return Err(InteractionError::DuplicateSession);
        }
        self.sessions.insert(
            request.session_id.clone(),
            InteractionSession {
                request,
                page: None,
                result: None,
            },
        );
        Ok(())
    }

    pub fn publish_page(&mut self, page: InteractionPage) -> Result<(), InteractionError> {
        if page.items.len() > MAXIMUM_PAGE_ITEMS {
            return Err(InteractionError::PageTooLarge);
        }
        let session = self
            .sessions
            .get_mut(&page.session_id)
            .ok_or(InteractionError::NotFound)?;
        ensure_open(session)?;
        if session.request.session_id != page.session_id {
            return Err(InteractionError::SessionMismatch);
        }
        session.page = Some(page);
        Ok(())
    }

    pub fn select(
        &mut self,
        session_id: &InteractionSessionId,
        value: serde_json::Value,
    ) -> Result<InteractionResult, InteractionError> {
        self.finish(session_id, InteractionResult::Selected(value))
    }

    pub fn cancel(
        &mut self,
        session_id: &InteractionSessionId,
        reason: CancelReason,
    ) -> Result<InteractionResult, InteractionError> {
        self.finish(session_id, InteractionResult::Cancelled(reason))
    }

    pub fn expire(&mut self, now_unix_ms: u64) -> Vec<(InvocationId, InteractionSessionId)> {
        let expired: Vec<_> = self
            .sessions
            .values()
            .filter(|session| {
                session.result.is_none() && session.request.deadline_unix_ms <= now_unix_ms
            })
            .map(|session| {
                (
                    session.request.owner_invocation_id.clone(),
                    session.request.session_id.clone(),
                )
            })
            .collect();
        for (_, session_id) in &expired {
            if let Some(session) = self.sessions.get_mut(session_id) {
                session.result = Some(InteractionResult::TimedOut);
            }
        }
        expired
    }

    #[must_use]
    pub fn get(&self, session_id: &InteractionSessionId) -> Option<&InteractionSession> {
        self.sessions.get(session_id)
    }

    fn finish(
        &mut self,
        session_id: &InteractionSessionId,
        result: InteractionResult,
    ) -> Result<InteractionResult, InteractionError> {
        let session = self
            .sessions
            .get_mut(session_id)
            .ok_or(InteractionError::NotFound)?;
        ensure_open(session)?;
        session.result = Some(result.clone());
        Ok(result)
    }
}

fn ensure_open(session: &InteractionSession) -> Result<(), InteractionError> {
    if session.result.is_some() {
        Err(InteractionError::AlreadyTerminal)
    } else {
        Ok(())
    }
}
