// SPDX-License-Identifier: GPL-3.0-or-later
use crate::{InteractionError, InteractionPage, InteractionRequest, InteractionSession};
use deckweave_domain::ids::InteractionSessionId;

pub trait InteractionPresenter: Send + Sync {
    fn open(&self, session: &InteractionSession) -> Result<(), InteractionError>;
    fn update(&self, session: &InteractionSession) -> Result<(), InteractionError>;
    fn close(&self, session_id: &InteractionSessionId) -> Result<(), InteractionError>;
}

pub trait InteractionProvider: Send + Sync {
    fn page(
        &self,
        request: &InteractionRequest,
        query: &serde_json::Value,
        cursor: Option<&str>,
    ) -> Result<InteractionPage, InteractionError>;
}
