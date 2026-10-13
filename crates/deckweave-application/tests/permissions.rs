// SPDX-License-Identifier: GPL-3.0-or-later
use deckweave_application::{permissions::*, *};
use deckweave_domain::{ids::*, security::*};
use deckweave_platform::{PermissionPreferenceStore, PermissionPreferences, PermissionSelection};
use std::sync::{Arc, Mutex};

#[derive(Default)]
struct Store(Mutex<PermissionPreferences>, Mutex<bool>);
impl PermissionPreferenceStore for Store {
    fn load(&self) -> Result<PermissionPreferences, PreferenceError> {
        if *self
            .1
            .lock()
            .map_err(|e| PreferenceError::Unavailable(e.to_string()))?
        {
            return Err(PreferenceError::Invalid);
        }
        Ok(self
            .0
            .lock()
            .map_err(|e| PreferenceError::Unavailable(e.to_string()))?
            .clone())
    }
    fn compare_exchange(
        &self,
        expected: u64,
        next: &PermissionPreferences,
    ) -> Result<(), PreferenceError> {
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
struct Catalog(PermissionOffer);
impl PermissionCatalogSource for Catalog {
    fn permission_offers(&self, _: &PrincipalId) -> Result<Vec<PermissionOffer>, String> {
        Ok(vec![self.0.clone()])
    }
}
#[test]
fn explicit_grants_restore_revoke_and_fail_closed_on_changed_package_or_corrupt_store()
-> Result<(), Box<dyn std::error::Error>> {
    let principal = PrincipalId::try_new("desktop").map_err(|e| format!("{e:?}"))?;
    let context = RequestContext {
        principal_id: principal.clone(),
        trace_id: TraceId::try_new("permissions").map_err(|e| format!("{e:?}"))?,
    };
    let offer = PermissionOffer {
        selection: PermissionSelection {
            package_id: PackageId::try_new("org.test.provider").map_err(|e| format!("{e:?}"))?,
            manifest_sha256: "a".repeat(64),
            permission_index: 0,
        },
        request: PermissionRequest {
            principal_id: principal.clone(),
            package_id: PackageId::try_new("org.test.provider").map_err(|e| format!("{e:?}"))?,
            contribution_id: None,
            scope: NamespacedKind::try_new("input.observe").map_err(|e| format!("{e:?}"))?,
            resource_selector: serde_json::json!({"aggregate": true}),
            access_mode: PermissionAccessMode::Observe,
            reason: "Observe keyboard".into(),
            extensions: Default::default(),
        },
        runtime_group_ids: vec![RuntimeGroupId::try_new("input").map_err(|e| format!("{e:?}"))?],
    };
    let store = Arc::new(Store::default());
    let auth = Arc::new(InMemoryApiAuthorizer::new());
    let grants = Arc::new(ScopedAuthorizationStore::default());
    let service = PluginPermissionService::new(
        store.clone(),
        &Catalog(offer.clone()),
        principal.clone(),
        grants.clone(),
        auth.clone(),
    )
    .map_err(|e| format!("{e:?}"))?;
    assert!(
        service
            .change(&context, 1, vec![offer.selection.clone()])
            .is_err()
    );
    auth.grant(
        principal.clone(),
        [ApiAuthScope::Read, ApiAuthScope::Preferences],
    );
    assert!(grants.authorize(&offer.request).is_err());
    service
        .change(&context, 1, vec![offer.selection.clone()])
        .map_err(|e| format!("{e:?}"))?;
    assert!(grants.authorize(&offer.request).is_ok());
    let revision = grants.revision().map_err(|e| format!("{e:?}"))?;
    service.get(&context).map_err(|e| format!("{e:?}"))?;
    service
        .change(&context, 1, vec![offer.selection.clone()])
        .map_err(|e| format!("{e:?}"))?;
    assert_eq!(revision, grants.revision().map_err(|e| format!("{e:?}"))?);
    let restored = PluginPermissionService::new(
        store.clone(),
        &Catalog(offer.clone()),
        principal.clone(),
        grants.clone(),
        auth.clone(),
    )
    .map_err(|e| format!("{e:?}"))?;
    assert!(grants.authorize(&offer.request).is_ok());
    restored
        .change(&context, 2, vec![])
        .map_err(|e| format!("{e:?}"))?;
    assert!(grants.authorize(&offer.request).is_err());
    assert!(
        restored
            .change(&context, 1, vec![offer.selection.clone()])
            .is_err()
    );
    restored
        .change(&context, 3, vec![offer.selection.clone()])
        .map_err(|e| format!("{e:?}"))?;
    *store.1.lock().map_err(|e| format!("{e:?}"))? = true;
    assert!(restored.get(&context).is_err());
    assert!(grants.authorize(&offer.request).is_err());
    *store.1.lock().map_err(|e| format!("{e:?}"))? = false;
    let mut newer = offer.clone();
    newer.selection.manifest_sha256 = "b".repeat(64);
    let updated =
        PluginPermissionService::new(store, &Catalog(newer), principal, grants.clone(), auth)
            .map_err(|e| format!("{e:?}"))?;
    assert!(grants.authorize(&offer.request).is_err());
    assert!(updated.change(&context, 4, vec![offer.selection]).is_err());
    Ok(())
}
