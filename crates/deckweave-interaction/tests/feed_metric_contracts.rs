// SPDX-License-Identifier: GPL-3.0-or-later
//! Plugin conformance calculations live in fixtures, not service-specific branches in core.
use deckweave_interaction::presentation::*;
use serde_json::json;
use std::{collections::BTreeMap, error::Error};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Unit {
    Messages,
    Threads,
    Chats,
}
struct Feed {
    account: &'static str,
    unit: Unit,
    count: Option<u64>,
    stale: bool,
    items: Vec<ItemRef>,
}
struct Projection {
    page: CollectionPresentation,
    counts: BTreeMap<Unit, u64>,
    partial: bool,
}
fn combine(feeds: &[Feed]) -> Result<Projection, Box<dyn Error>> {
    let mut counts = BTreeMap::new();
    let mut partial = false;
    let mut items = BTreeMap::new();
    for feed in feeds {
        partial |= feed.stale || feed.count.is_none();
        if let Some(count) = feed.count {
            *counts.entry(feed.unit).or_default() += count;
        }
        for item in &feed.items {
            assert_eq!(item.instance_id.as_str(), feed.account);
            // Dedup identity, never title/snippet text across providers/accounts.
            items.insert(
                (
                    item.package_id.clone(),
                    item.instance_id.clone(),
                    item.collection.clone(),
                    item.item_id.clone(),
                ),
                item.clone(),
            );
        }
    }
    let page = CollectionPresentation {
        fence: serde_json::from_value(
            json!({"activationEpoch":1,"providerGeneration":1,"revision":1}),
        )?,
        items: items.into_values().collect(),
        next_cursor: None,
    };
    page.validate()?;
    Ok(Projection {
        page,
        counts,
        partial,
    })
}

#[test]
fn mixed_unread_preserves_units_accounts_partial_state_and_remote_read_state()
-> Result<(), Box<dyn Error>> {
    let item = |package: &str, account: &str| -> Result<ItemRef, Box<dyn Error>> {
        Ok(serde_json::from_value(
            json!({"packageId":package,"instanceId":account,"collection":"unread","itemId":"same-id","revision":"v1"}),
        )?)
    };
    let mail = item("org.fixture.mail", "mail-a")?;
    let feeds = [
        Feed {
            account: "mail-a",
            unit: Unit::Messages,
            count: Some(3),
            stale: false,
            items: vec![mail.clone(), mail],
        },
        Feed {
            account: "mail-b",
            unit: Unit::Messages,
            count: Some(2),
            stale: false,
            items: vec![item("org.fixture.mail", "mail-b")?],
        },
        Feed {
            account: "mattermost",
            unit: Unit::Threads,
            count: Some(4),
            stale: false,
            items: vec![item("org.fixture.mattermost", "mattermost")?],
        },
        Feed {
            account: "telegram",
            unit: Unit::Chats,
            count: None,
            stale: true,
            items: vec![],
        },
    ];
    let projection = combine(&feeds)?;
    assert_eq!(projection.counts[&Unit::Messages], 5);
    assert_eq!(projection.counts[&Unit::Threads], 4);
    assert!(!projection.counts.contains_key(&Unit::Chats));
    assert!(projection.partial);
    assert_eq!(projection.page.items.len(), 3);
    // A presenter receives only this immutable projection, no remote read/ack command handle.
    for index in 0..30 {
        projection.page.hit(&projection.page.fence, index % 3)?;
    }
    assert_eq!(feeds[0].count, Some(3));
    assert_eq!(feeds[1].count, Some(2));
    Ok(())
}

#[derive(Clone)]
struct Measurement {
    source: &'static str,
    id: &'static str,
    revision: u64,
    start: i64,
    end: i64,
    value: u64,
}
fn steps(records: &[Measurement], selected: &str, day_start: i64, day_end: i64) -> Option<u64> {
    let mut latest = BTreeMap::<&str, &Measurement>::new();
    for record in records.iter().filter(|r| r.source == selected) {
        if latest
            .get(record.id)
            .is_none_or(|r| r.revision < record.revision)
        {
            latest.insert(record.id, record);
        }
    }
    let mut intervals = latest.into_values().collect::<Vec<_>>();
    intervals.sort_by_key(|r| r.start);
    let mut end = day_start;
    let mut sum = 0u64;
    for record in intervals {
        if record.start < end || record.end > day_end || record.start >= record.end {
            return None;
        }
        sum = sum.checked_add(record.value)?;
        end = record.end;
    }
    Some(sum)
}
#[test]
fn corrections_overlap_and_civil_day_boundaries_do_not_invent_exact_metrics() {
    // The timezone adapter supplies this 25-hour civil-day interval. Core does not assume 86400 s.
    let day = (0, 90_000);
    let watch = Measurement {
        source: "watch",
        id: "walk",
        revision: 1,
        start: 85_000,
        end: 89_000,
        value: 1200,
    };
    let mut records = vec![
        watch.clone(),
        Measurement {
            source: "phone",
            value: 1000,
            ..watch.clone()
        },
        Measurement {
            revision: 2,
            value: 1100,
            ..watch.clone()
        },
    ];
    assert_eq!(steps(&records, "watch", day.0, day.1), Some(1100));
    assert_eq!(steps(&records, "phone", day.0, day.1), Some(1000));
    assert_eq!(steps(&records, "watch", day.0, 86_400), None);
    records.push(Measurement {
        id: "overlap",
        start: 88_000,
        end: 89_500,
        ..watch
    });
    assert_eq!(steps(&records, "watch", day.0, day.1), None);
    // Calendar countdown uses adapter-supplied local day ordinals, duration uses instants.
    let today_ordinal = 100;
    let target_ordinal = 101;
    assert_eq!(target_ordinal - today_ordinal, 1);
    assert_eq!(day.1 - day.0, 25 * 3600);
    let remaining_steps = 2000u64.saturating_sub(1100);
    let required_per_hour = |now| {
        if now < day.1 {
            Some(remaining_steps as f64 / ((day.1 - now) as f64 / 3600.0))
        } else {
            None
        }
    };
    assert!(required_per_hour(89_000).is_some());
    assert!(required_per_hour(90_000).is_none());
}
