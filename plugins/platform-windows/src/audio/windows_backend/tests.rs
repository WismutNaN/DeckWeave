// SPDX-License-Identifier: GPL-3.0-or-later
use super::*;

#[test]
fn known_hresult_values_map_to_stable_failures() {
    assert_eq!(
        classify_hresult(0x8007_0490_u32 as i32),
        AudioFailure::NoDefaultEndpoint
    );
    assert_eq!(
        classify_hresult(0x8889_0004_u32 as i32),
        AudioFailure::TransientRebind
    );
    assert_eq!(classify_hresult(-1), AudioFailure::OsApiUnavailable);
}

#[test]
fn callback_burst_is_bounded_and_requests_one_refresh() {
    let (sender, receiver) = mpsc::sync_channel(1);
    let overflow = Arc::new(AtomicBool::new(false));
    let queue = CallbackQueue {
        sender,
        overflow: Arc::clone(&overflow),
    };
    for _ in 0..128 {
        queue.push(AdapterEvent::VolumeChanged);
    }
    assert!(overflow.load(Ordering::Acquire));
    assert_eq!(
        drain_events(&receiver, &overflow),
        Maintenance {
            changed: true,
            rebind: true
        }
    );
}

#[test]
fn default_endpoint_event_requests_rebind_outside_the_callback() {
    let (sender, receiver) = mpsc::sync_channel(1);
    let overflow = Arc::new(AtomicBool::new(false));
    CallbackQueue {
        sender,
        overflow: Arc::clone(&overflow),
    }
    .push(AdapterEvent::DefaultEndpointChanged);
    assert_eq!(
        drain_events(&receiver, &overflow),
        Maintenance {
            changed: true,
            rebind: true
        }
    );
}

#[test]
fn a_default_switch_dropped_behind_volume_events_still_rebinds() {
    let (sender, receiver) = mpsc::sync_channel(1);
    let overflow = Arc::new(AtomicBool::new(false));
    let queue = CallbackQueue {
        sender,
        overflow: Arc::clone(&overflow),
    };
    queue.push(AdapterEvent::VolumeChanged);
    queue.push(AdapterEvent::DefaultEndpointChanged);
    assert!(drain_events(&receiver, &overflow).rebind);
    assert_eq!(drain_events(&receiver, &overflow), Maintenance::default());
}
