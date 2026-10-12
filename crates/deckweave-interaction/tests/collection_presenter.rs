// SPDX-License-Identifier: GPL-3.0-or-later
//! One fake presenter consumes independent providers without provider-specific core branches.
use deckweave_interaction::presentation::*;
use serde_json::json;
use std::{collections::BTreeMap, error::Error};

#[derive(Default)]
struct Presenter {
    pages: BTreeMap<String, CollectionPresentation>,
    navigation: BTreeMap<String, Vec<CollectionPresentation>>,
    slots: SlotSelections,
}
impl Presenter {
    fn show(&mut self, view: &str, page: CollectionPresentation) -> Result<(), PresentationError> {
        page.validate()?;
        if let Some(previous) = self.pages.insert(view.into(), page) {
            let history = self.navigation.entry(view.into()).or_default();
            if history.len() == 8 {
                history.remove(0);
            }
            history.push(previous);
        }
        Ok(())
    }
    fn back(&mut self, view: &str) {
        if let Some(mut previous) = self.navigation.get_mut(view).and_then(Vec::pop) {
            // Back is a new presentation, never reuse a historical hit fence.
            previous.fence.revision = self
                .pages
                .get(view)
                .map_or(1, |page| page.fence.revision + 1);
            self.pages.insert(view.into(), previous);
        }
    }
    fn draw(&self, widget: &str) -> usize {
        self.slots.consumers(widget)
    }
}

#[test]
fn tasks_windows_and_notes_share_navigation_and_capture_targets_before_reorder_or_slot_switch()
-> Result<(), Box<dyn Error>> {
    let mut presenter = Presenter::default();
    for (provider, collection) in [
        ("tasks", "today"),
        ("windows", "visible"),
        ("notes", "private"),
    ] {
        let page: CollectionPresentation = serde_json::from_value(json!({
            "fence":{"activationEpoch":1,"providerGeneration":1,"revision":1},
            "items":[{"packageId":format!("org.fixture.{provider}"), "instanceId":"account-a",
                "collection":collection,"itemId":"first","revision":"etag-1"},
                {"packageId":format!("org.fixture.{provider}"), "instanceId":"account-a",
                "collection":collection,"itemId":"second","revision":"etag-1"}], "nextCursor":null
        }))?;
        for view in ["device-a", "device-b"] {
            presenter.show(view, page.clone())?;
        }
        let held = page.hit(&page.fence, 0)?;
        let mut updated = page.clone();
        updated.fence.revision = 2;
        updated.items.reverse();
        presenter.show("device-a", updated.clone())?;
        assert_eq!(presenter.pages["device-b"], page);
        assert_eq!(held.item_id, "first");
        assert_eq!(updated.hit(&page.fence, 0), Err(PresentationError::Stale));
        // Deletion/reordering cannot turn a delayed release into an action on the second item.
        updated.items.retain(|i| i.item_id != held.item_id);
        assert!(!updated.items.contains(&held));
        presenter.show("device-a", updated)?;
        presenter.back("device-a");
        assert_eq!(presenter.pages["device-a"].fence.revision, 3);
        presenter.back("device-a");
        assert_eq!(presenter.pages["device-a"].items, page.items);
        assert_eq!(presenter.pages["device-a"].fence.revision, 4);
    }
    for slot in ["a:key-1", "b:key-1"] {
        presenter.slots.define(
            slot.into(),
            SlotDefinition {
                widgets: vec!["clock".into(), "countdown".into(), "notes".into()],
            },
        )?;
    }
    assert_eq!(presenter.draw("clock"), 2);
    let held_widget = presenter.slots.selected("a:key-1")?.to_owned();
    presenter.slots.next("a:key-1")?;
    presenter.slots.next("a:key-1")?;
    assert_eq!(presenter.draw("clock"), 1);
    assert_eq!(presenter.draw("countdown"), 0);
    assert_eq!(presenter.draw("notes"), 1);
    assert_eq!(presenter.slots.selected("a:key-1")?, "notes");
    assert_eq!(presenter.slots.selected("b:key-1")?, "clock");
    assert_eq!(held_widget, "clock");
    presenter.slots.remove("a:key-1");
    assert_eq!(presenter.draw("notes"), 0);
    Ok(())
}
