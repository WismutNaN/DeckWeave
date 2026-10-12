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
        fence: &deckweave_interaction::QueryFence,
        query: &serde_json::Value,
        cursor: Option<&str>,
    ) -> Result<InteractionPage, InteractionError> {
        Ok(InteractionPage {
            fence: fence.clone(),
            session_id: request.session_id.clone(),
            items: vec![InteractionItem {
                item_id: cursor.unwrap_or("first").into(),
                label: query["text"].as_str().unwrap_or_default().into(),
                value: serde_json::json!({"key": "selected"}),
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
        activation_epoch: deckweave_domain::ids::ActivationEpoch::try_new(1)?,
        provider_generation: deckweave_domain::ids::SessionGeneration::try_new(1)?,
        view: InteractionViewKind::SearchList,
        initial_query: serde_json::json!({"text": "rock"}),
        result_schema: serde_json::from_value(
            serde_json::json!({"id":"fixture.item-ref","version":{"major":1,"minor":0}}),
        )?,
        result_shape: serde_json::from_value(
            serde_json::json!({"type":"record","fields":{"key":{"schema":{"type":"string","maximumBytes":128}}}}),
        )?,
        deadline_host_ticks: deadline,
    })
}

#[test]
fn concurrent_search_pages_and_results_remain_isolated() -> Result<(), Box<dyn Error>> {
    let mut sessions = InteractionSessions::new();
    let first = request("session-a", "invocation-a", 200)?;
    let second = request("session-b", "invocation-b", 300)?;
    sessions.open(first.clone())?;
    sessions.open(second.clone())?;
    let first_fence = sessions.begin_query(&first.session_id, 0)?;
    let second_fence = sessions.begin_query(&second.session_id, 0)?;
    sessions.publish_page(
        InteractionPage {
            fence: first_fence.clone(),
            session_id: first.session_id.clone(),
            items: vec![InteractionItem {
                item_id: "a".into(),
                label: "Article A".into(),
                value: serde_json::json!({"key": "a"}),
            }],
            next_cursor: Some("cursor-a".into()),
        },
        0,
    )?;
    sessions.publish_page(
        InteractionPage {
            fence: second_fence,
            session_id: second.session_id.clone(),
            items: vec![InteractionItem {
                item_id: "b".into(),
                label: "Article B".into(),
                value: serde_json::json!({"key": "b"}),
            }],
            next_cursor: None,
        },
        0,
    )?;
    sessions.select(
        &first.session_id,
        serde_json::json!({"key": "a"}),
        &first_fence,
        0,
    )?;
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
    let fence = sessions.begin_query(&request.session_id, 0)?;
    let page = provider.page(&request, &fence, &request.initial_query, Some("second"))?;
    sessions.publish_page(page, 0)?;
    presenter.update(
        sessions
            .get(&request.session_id)
            .ok_or(InteractionError::NotFound)?,
    )?;
    sessions.select(
        &request.session_id,
        serde_json::json!({"key": "selected"}),
        &fence,
        0,
    )?;
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

#[test]
fn stale_query_reused_session_and_lost_provider_are_fenced() -> Result<(), Box<dyn Error>> {
    let mut sessions = InteractionSessions::new();
    let first = request("reusable", "owner", 1000)?;
    sessions.open(first.clone())?;
    let old = sessions.begin_query(&first.session_id, 0)?;
    let latest = sessions.begin_query(&first.session_id, 0)?;
    let stale = FakeProvider.page(&first, &old, &first.initial_query, None)?;
    assert_eq!(
        sessions.publish_page(stale, 0),
        Err(InteractionError::StaleQuery)
    );
    sessions.publish_page(
        FakeProvider.page(&first, &latest, &first.initial_query, None)?,
        0,
    )?;
    assert_eq!(
        sessions.select(
            &first.session_id,
            serde_json::json!({"key":"selected"}),
            &old,
            0
        ),
        Err(InteractionError::StaleQuery)
    );
    sessions.cancel_owner(&first.owner_invocation_id, CancelReason::Apply);
    for n in 0..64 {
        let request = request(&format!("temporary-{n}"), "other", 1000)?;
        sessions.open(request.clone())?;
        sessions.cancel(&request.session_id, CancelReason::UserClosed)?;
    }
    assert!(sessions.get(&first.session_id).is_none());
    sessions.open(first.clone())?;
    let fresh = sessions.begin_query(&first.session_id, 0)?;
    sessions.publish_page(
        FakeProvider.page(&first, &fresh, &first.initial_query, None)?,
        0,
    )?;
    assert_eq!(old.query_generation, fresh.query_generation);
    assert_eq!(
        sessions.select(
            &first.session_id,
            serde_json::json!({"key":"selected"}),
            &old,
            0
        ),
        Err(InteractionError::StaleQuery)
    );
    assert_eq!(
        sessions.provider_lost(&first.provider_id, first.provider_generation),
        vec![first.session_id.clone()]
    );
    assert!(matches!(
        sessions
            .get(&first.session_id)
            .and_then(|s| s.result.as_ref()),
        Some(InteractionResult::Cancelled(CancelReason::ProviderLost))
    ));
    Ok(())
}
