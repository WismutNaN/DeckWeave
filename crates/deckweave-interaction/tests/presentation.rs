// SPDX-License-Identifier: GPL-3.0-or-later
use deckweave_interaction::presentation::*;
use std::error::Error;

#[test]
fn persistent_views_reject_late_close_and_release_only_their_instance() -> Result<(), Box<dyn Error>>
{
    let mut views = deckweave_interaction::DesktopViews::default();
    let session: DesktopViewSession = serde_json::from_value(serde_json::json!({
        "viewId":"notes","instanceId":"notes-a","fence":{"activationEpoch":1,"providerGeneration":1,"revision":1},
        "leaseExpiresHostTicks":100,"documentId":"note-1"
    }))?;
    views.open(session.clone(), 0)?;
    let mut replaced = session.clone();
    replaced.fence.revision = 2;
    views.open(replaced.clone(), 0)?;
    assert_eq!(
        views.close("notes", &session.fence),
        Err(PresentationError::Stale)
    );
    let mut other = session.clone();
    other.view_id = "other-notes".into();
    other.instance_id = deckweave_domain::ids::PluginInstanceId::try_new("notes-b")?;
    views.open(other.clone(), 0)?;
    assert_eq!(views.revoke_instance(&session.instance_id), vec![replaced]);
    assert!(views.expire(99).is_empty());
    assert_eq!(views.expire(100), vec![other]);
    // Closing a view returns its document reference; this component has no delete-data port.
    views.open(session.clone(), 0)?;
    assert_eq!(
        views.close("notes", &session.fence)?.document_id.as_deref(),
        Some("note-1")
    );
    Ok(())
}

#[test]
fn list_reorder_does_not_retarget_press_and_slots_have_independent_consumers()
-> Result<(), Box<dyn Error>> {
    let fence: PresentationFence = serde_json::from_value(
        serde_json::json!({"activationEpoch":1,"providerGeneration":1,"revision":1}),
    )?;
    let item = |id: &str| -> Result<ItemRef, Box<dyn Error>> {
        Ok(serde_json::from_value(serde_json::json!({
            "packageId":"org.fixture.tasks", "instanceId":"account-a", "collection":"today", "itemId":id, "revision":"etag-1"
        }))?)
    };
    let mut page = CollectionPresentation {
        fence: fence.clone(),
        items: vec![item("first")?, item("second")?],
        next_cursor: None,
    };
    let held = page.hit(&fence, 0)?;
    page.items.reverse();
    page.fence.revision = 2;
    assert_eq!(held.item_id, "first");
    assert_eq!(page.hit(&fence, 0), Err(PresentationError::Stale));
    assert_eq!(page.hit(&page.fence, 0)?.item_id, "second");
    let mut slots = SlotSelections::default();
    for slot in ["deck-a:key-1", "deck-b:key-1"] {
        slots.define(
            slot.into(),
            SlotDefinition {
                widgets: vec!["clock".into(), "countdown".into()],
            },
        )?;
    }
    assert_eq!(slots.consumers("clock"), 2);
    slots.next("deck-a:key-1")?;
    assert_eq!(slots.consumers("clock"), 1);
    assert_eq!(slots.consumers("countdown"), 1);
    slots.back("deck-a:key-1")?;
    assert_eq!(slots.consumers("countdown"), 0);
    slots.remove("deck-b:key-1");
    assert_eq!(slots.consumers("clock"), 1);
    Ok(())
}
