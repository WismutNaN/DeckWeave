// SPDX-License-Identifier: GPL-3.0-or-later
use deckweave_domain::ids::{InteractionSessionId, InvocationId, ProviderId};
use deckweave_interaction::{
    CancelReason, InteractionError, InteractionItem, InteractionPage, InteractionPresenter,
    InteractionProvider, InteractionRequest, InteractionResult, InteractionSession,
    InteractionSessions, InteractionViewKind,
};
use std::{error::Error, sync::Mutex};

struct FakeProvider;

impl InteractionProvider for FakeProvider {
    fn page(
        &self,
        request: &InteractionRequest,
        query: &serde_json::Value,
        cursor: Option<&str>,
    ) -> Result<InteractionPage, InteractionError> {
        Ok(InteractionPage {
            session_id: request.session_id.clone(),
            items: vec![InteractionItem {
                item_id: cursor.unwrap_or("first").into(),
                label: query["text"].as_str().unwrap_or_default().into(),
                value: serde_json::json!({"provider": request.provider_id.as_str()}),
            }],
            next_cursor: None,
        })
    }
}

#[derive(Default)]
struct FakePresenter {
    events: Mutex<Vec<String>>,
}

impl InteractionPresenter for FakePresenter {
    fn open(&self, session: &InteractionSession) -> Result<(), InteractionError> {
        self.events
            .lock()
            .map_err(|_| InteractionError::Adapter("poisoned".into()))?
            .push(format!("open:{}", session.request.session_id));
        Ok(())
    }

    fn update(&self, session: &InteractionSession) -> Result<(), InteractionError> {
        self.events
            .lock()
            .map_err(|_| InteractionError::Adapter("poisoned".into()))?
            .push(format!("update:{}", session.request.session_id));
        Ok(())
    }

    fn close(&self, session_id: &InteractionSessionId) -> Result<(), InteractionError> {
        self.events
            .lock()
            .map_err(|_| InteractionError::Adapter("poisoned".into()))?
            .push(format!("close:{session_id}"));
        Ok(())
    }
}

fn request(
    session: &str,
    invocation: &str,
    deadline: u64,
) -> Result<InteractionRequest, Box<dyn Error>> {
    Ok(InteractionRequest {
        session_id: InteractionSessionId::try_new(session)?,
        owner_invocation_id: InvocationId::try_new(invocation)?,
        provider_id: ProviderId::try_new("fixture-provider")?,
        view: InteractionViewKind::SearchList,
        initial_query: serde_json::json!({"text": "rock"}),
        result_schema: "fixture.item-ref.v1".into(),
        deadline_unix_ms: deadline,
    })
}

#[test]
fn concurrent_search_pages_and_results_remain_isolated() -> Result<(), Box<dyn Error>> {
    let mut sessions = InteractionSessions::new();
    let first = request("session-a", "invocation-a", 200)?;
    let second = request("session-b", "invocation-b", 300)?;
    sessions.open(first.clone())?;
    sessions.open(second.clone())?;
    sessions.publish_page(InteractionPage {
        session_id: first.session_id.clone(),
        items: vec![InteractionItem {
            item_id: "a".into(),
            label: "Article A".into(),
            value: serde_json::json!({"key": "a"}),
        }],
        next_cursor: Some("cursor-a".into()),
    })?;
    sessions.publish_page(InteractionPage {
        session_id: second.session_id.clone(),
        items: vec![InteractionItem {
            item_id: "b".into(),
            label: "Article B".into(),
            value: serde_json::json!({"key": "b"}),
        }],
        next_cursor: None,
    })?;
    sessions.select(&first.session_id, serde_json::json!({"key": "a"}))?;
    sessions.cancel(&second.session_id, CancelReason::Escape)?;
    assert_eq!(
        sessions
            .get(&first.session_id)
            .and_then(|item| item.result.clone()),
        Some(InteractionResult::Selected(serde_json::json!({"key": "a"})))
    );
    assert_eq!(
        sessions
            .get(&second.session_id)
            .and_then(|item| item.result.clone()),
        Some(InteractionResult::Cancelled(CancelReason::Escape))
    );
    assert_eq!(
        sessions.cancel(&first.session_id, CancelReason::UserClosed),
        Err(InteractionError::AlreadyTerminal)
    );
    Ok(())
}

#[test]
fn timeout_is_terminal_and_reports_only_its_owner() -> Result<(), Box<dyn Error>> {
    let mut sessions = InteractionSessions::new();
    let expired = request("session-expired", "invocation-expired", 10)?;
    let live = request("session-live", "invocation-live", 20)?;
    sessions.open(expired.clone())?;
    sessions.open(live.clone())?;
    assert_eq!(
        sessions.expire(10),
        vec![(expired.owner_invocation_id, expired.session_id.clone())]
    );
    assert_eq!(
        sessions
            .get(&expired.session_id)
            .and_then(|item| item.result.clone()),
        Some(InteractionResult::TimedOut)
    );
    assert!(
        sessions
            .get(&live.session_id)
            .is_some_and(|item| item.result.is_none())
    );
    Ok(())
}

#[test]
fn provider_pages_and_presenter_lifecycle_stay_behind_ports() -> Result<(), Box<dyn Error>> {
    let request = request("session-ports", "invocation-ports", 100)?;
    let provider = FakeProvider;
    let presenter = FakePresenter::default();
    let mut sessions = InteractionSessions::new();
    sessions.open(request.clone())?;
    presenter.open(
        sessions
            .get(&request.session_id)
            .ok_or(InteractionError::NotFound)?,
    )?;
    let page = provider.page(&request, &request.initial_query, Some("second"))?;
    sessions.publish_page(page)?;
    presenter.update(
        sessions
            .get(&request.session_id)
            .ok_or(InteractionError::NotFound)?,
    )?;
    sessions.select(&request.session_id, serde_json::json!({"key": "selected"}))?;
    presenter.close(&request.session_id)?;
    assert_eq!(
        *presenter
            .events
            .lock()
            .map_err(|_| InteractionError::Adapter("poisoned".into()))?,
        vec![
            "open:session-ports",
            "update:session-ports",
            "close:session-ports"
        ]
    );
    Ok(())
}
