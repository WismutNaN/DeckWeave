// SPDX-License-Identifier: GPL-3.0-or-later
use deckweave_application::{locale::*, *};
use deckweave_domain::{
    ids::{PrincipalId, TraceId},
    localization::LocaleContext,
};
use std::{
    error::Error,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
};

#[derive(Default)]
struct Store(Mutex<LocaleContext>);
impl LocalePreferenceStore for Store {
    fn load(&self) -> Result<LocaleContext, PreferenceError> {
        Ok(self
            .0
            .lock()
            .map_err(|e| PreferenceError::Unavailable(e.to_string()))?
            .clone())
    }
    fn compare_exchange(&self, expected: u64, next: &LocaleContext) -> Result<(), PreferenceError> {
        next.validate().map_err(|_| PreferenceError::Invalid)?;
        if expected.checked_add(1) != Some(next.revision) {
            return Err(PreferenceError::Invalid);
        }
        let mut current = self
            .0
            .lock()
            .map_err(|e| PreferenceError::Unavailable(e.to_string()))?;
        if *current == *next {
            return Ok(());
        }
        if current.revision != expected {
            return Err(PreferenceError::Conflict);
        }
        *current = next.clone();
        Ok(())
    }
}
#[derive(Default)]
struct Publisher {
    unavailable: AtomicBool,
    received: Mutex<Vec<LocaleContext>>,
}
impl LocalePublisher for Publisher {
    fn publish_locale(&self, value: LocaleContext) -> Result<(), String> {
        if self.unavailable.load(Ordering::SeqCst) {
            return Err("full".into());
        }
        self.received.lock().map_err(|e| e.to_string())?.push(value);
        Ok(())
    }
}

#[test]
fn preference_is_durable_before_delivery_and_reconcile_never_reverts_newer_selection()
-> Result<(), Box<dyn Error>> {
    let auth = Arc::new(InMemoryApiAuthorizer::new());
    let request = RequestContext {
        principal_id: PrincipalId::try_new("desktop")?,
        trace_id: TraceId::try_new("locale")?,
    };
    let store = Arc::new(Store::default());
    let publisher = Arc::new(Publisher::default());
    let service = LocalePreferencesService::new(store.clone(), publisher.clone(), auth.clone());
    let next = LocaleContext {
        revision: 2,
        requested_locale: "ru-RU".into(),
        time_zone: "UTC".into(),
    };
    assert!(matches!(
        service.change(&request, 1, next.clone()),
        Err(LocaleError::Denied)
    ));
    auth.grant(
        request.principal_id.clone(),
        [ApiAuthScope::Read, ApiAuthScope::Preferences],
    );
    publisher.unavailable.store(true, Ordering::SeqCst);
    assert!(service.change(&request, 1, next.clone())?.delivery_pending);
    assert_eq!(service.get(&request)?, next);
    let newer = LocaleContext {
        revision: 3,
        requested_locale: "de".into(),
        ..next.clone()
    };
    service.change(&request, 2, newer.clone())?;
    assert!(matches!(
        service.change(&request, 1, next),
        Err(LocaleError::Store(PreferenceError::Conflict))
    ));
    publisher.unavailable.store(false, Ordering::SeqCst);
    let resumed = LocalePreferencesService::new(store, publisher.clone(), auth);
    assert!(!resumed.reconcile(&request)?.delivery_pending);
    assert_eq!(
        *publisher.received.lock().map_err(|e| e.to_string())?,
        vec![newer]
    );
    Ok(())
}
