// SPDX-License-Identifier: GPL-3.0-or-later
use crate::{
    CancelReason, InteractionPage, InteractionRequest, InteractionResult, InteractionSession,
};
use deckweave_domain::ids::{InteractionSessionId, InvocationId};
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use thiserror::Error;

const MAXIMUM_PAGE_ITEMS: usize = 100;

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum InteractionError {
    #[error("interaction capacity exceeded")]
    Capacity,
    #[error("stale query or provider generation")]
    StaleQuery,
    #[error("interaction value does not match its declared schema")]
    InvalidValue,
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
    #[error("interaction adapter failed: {0}")]
    Adapter(String),
}

#[derive(Default)]
pub struct InteractionSessions {
    next_generation: u64,
    sessions: BTreeMap<InteractionSessionId, InteractionSession>,
    terminal_order: VecDeque<InteractionSessionId>,
}

impl InteractionSessions {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn open(&mut self, request: InteractionRequest) -> Result<(), InteractionError> {
        request
            .result_shape
            .validate_definition()
            .map_err(|_| InteractionError::InvalidValue)?;
        if request.deadline_host_ticks == 0
            || !deckweave_domain::schema::bounded_json(&request.initial_query)
        {
            return Err(InteractionError::InvalidValue);
        }
        if self.sessions.contains_key(&request.session_id) {
            return Err(InteractionError::DuplicateSession);
        }
        if self.sessions.len() >= 128 {
            return Err(InteractionError::Capacity);
        }
        self.next_generation = self
            .next_generation
            .checked_add(1)
            .ok_or(InteractionError::Capacity)?;
        self.sessions.insert(
            request.session_id.clone(),
            InteractionSession {
                generation: self.next_generation,
                request,
                page: None,
                result: None,
                query_generation: 0,
            },
        );
        Ok(())
    }

    pub fn begin_query(
        &mut self,
        session_id: &InteractionSessionId,
        now: u64,
    ) -> Result<crate::QueryFence, InteractionError> {
        self.expire(now);
        let session = self
            .sessions
            .get_mut(session_id)
            .ok_or(InteractionError::NotFound)?;
        ensure_open(session)?;
        session.query_generation = session
            .query_generation
            .checked_add(1)
            .ok_or(InteractionError::Capacity)?;
        session.page = None;
        Ok(crate::QueryFence {
            session_generation: session.generation,
            activation_epoch: session.request.activation_epoch,
            provider_generation: session.request.provider_generation,
            query_generation: session.query_generation,
        })
    }

    pub fn publish_page(
        &mut self,
        page: InteractionPage,
        now: u64,
    ) -> Result<(), InteractionError> {
        self.expire(now);
        if page.items.len() > MAXIMUM_PAGE_ITEMS
            || serde_json::to_vec(&page)
                .map_err(|_| InteractionError::InvalidValue)?
                .len()
                > 256 * 1024
        {
            return Err(InteractionError::PageTooLarge);
        }
        let session = self
            .sessions
            .get_mut(&page.session_id)
            .ok_or(InteractionError::NotFound)?;
        ensure_open(session)?;
        if page.fence.session_generation != session.generation
            || page.fence.query_generation == 0
            || page.fence.query_generation != session.query_generation
            || page.fence.activation_epoch != session.request.activation_epoch
            || page.fence.provider_generation != session.request.provider_generation
        {
            return Err(InteractionError::StaleQuery);
        }
        if session.request.session_id != page.session_id {
            return Err(InteractionError::SessionMismatch);
        }
        let mut ids = BTreeSet::new();
        if page.next_cursor.as_ref().is_some_and(|c| c.len() > 4096)
            || page.items.iter().any(|item| {
                item.item_id.is_empty()
                    || item.item_id.len() > 512
                    || !ids.insert(&item.item_id)
                    || !session.request.result_shape.accepts(&item.value)
            })
        {
            return Err(InteractionError::InvalidValue);
        }
        session.page = Some(page);
        Ok(())
    }

    pub fn select(
        &mut self,
        session_id: &InteractionSessionId,
        value: serde_json::Value,
        fence: &crate::QueryFence,
        now: u64,
    ) -> Result<InteractionResult, InteractionError> {
        self.expire(now);
        let session = self
            .sessions
            .get(session_id)
            .ok_or(InteractionError::NotFound)?;
        ensure_open(session)?;
        if fence.query_generation != session.query_generation
            || fence.session_generation != session.generation
            || fence.activation_epoch != session.request.activation_epoch
            || fence.provider_generation != session.request.provider_generation
        {
            return Err(InteractionError::StaleQuery);
        }
        if !session.request.result_shape.accepts(&value)
            || (session.request.view == crate::InteractionViewKind::SearchList
                && !session
                    .page
                    .as_ref()
                    .is_some_and(|page| page.items.iter().any(|item| item.value == value)))
        {
            return Err(InteractionError::InvalidValue);
        }
        self.finish(session_id, InteractionResult::Selected(value))
    }

    pub fn cancel(
        &mut self,
        session_id: &InteractionSessionId,
        reason: CancelReason,
    ) -> Result<InteractionResult, InteractionError> {
        self.finish(session_id, InteractionResult::Cancelled(reason))
    }

    pub fn cancel_owner(
        &mut self,
        owner: &InvocationId,
        reason: CancelReason,
    ) -> Vec<InteractionSessionId> {
        let ids: Vec<_> = self
            .sessions
            .values()
            .filter(|s| s.result.is_none() && &s.request.owner_invocation_id == owner)
            .map(|s| s.request.session_id.clone())
            .collect();
        for id in &ids {
            let _ = self.cancel(id, reason.clone());
        }
        ids
    }

    pub fn provider_lost(
        &mut self,
        provider: &deckweave_domain::ids::ProviderId,
        generation: deckweave_domain::ids::SessionGeneration,
    ) -> Vec<InteractionSessionId> {
        let ids: Vec<_> = self
            .sessions
            .values()
            .filter(|s| {
                s.result.is_none()
                    && &s.request.provider_id == provider
                    && s.request.provider_generation == generation
            })
            .map(|s| s.request.session_id.clone())
            .collect();
        for id in &ids {
            let _ = self.cancel(id, CancelReason::ProviderLost);
        }
        ids
    }

    pub fn expire(&mut self, now_host_ticks: u64) -> Vec<(InvocationId, InteractionSessionId)> {
        let expired: Vec<_> = self
            .sessions
            .values()
            .filter(|session| {
                session.result.is_none() && session.request.deadline_host_ticks <= now_host_ticks
            })
            .map(|session| {
                (
                    session.request.owner_invocation_id.clone(),
                    session.request.session_id.clone(),
                )
            })
            .collect();
        for (_, session_id) in &expired {
            let _ = self.finish(session_id, InteractionResult::TimedOut);
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
        session.page = None;
        self.terminal_order.push_back(session_id.clone());
        while self.terminal_order.len() > 64 {
            if let Some(old) = self.terminal_order.pop_front() {
                self.sessions.remove(&old);
            }
        }
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
