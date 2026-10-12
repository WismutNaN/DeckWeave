// SPDX-License-Identifier: GPL-3.0-or-later
use deckweave_application::{AuthorizationError, AuthorizationStore, ScopedAuthorizationStore};
use deckweave_domain::{ids::PackageId, security::PermissionRequest};
use std::error::Error;
#[test]
fn grants_never_leak_to_another_package_resource_or_access_mode() -> Result<(), Box<dyn Error>> {
    let store = ScopedAuthorizationStore::default();
    let request: PermissionRequest = serde_json::from_value(
        serde_json::json!({"principalId":"desktop","packageId":"org.fixture.tasks",
        "scope":"org.fixture.http","resourceSelector":{"origin":"https://example.invalid"},"accessMode":"read","reason":"tasks"}),
    )?;
    assert_eq!(store.authorize(&request), Err(AuthorizationError::Denied));
    let revision = store.replace(vec![request.clone()])?;
    assert_eq!(store.authorize(&request)?.revision, revision);
    let mut different = request.clone();
    different.package_id = PackageId::try_new("org.fixture.other")?;
    assert_eq!(store.authorize(&different), Err(AuthorizationError::Denied));
    different = request.clone();
    different.resource_selector = serde_json::json!({"origin":"https://different.invalid"});
    assert_eq!(store.authorize(&different), Err(AuthorizationError::Denied));
    different = request.clone();
    different.access_mode = deckweave_domain::security::PermissionAccessMode::Write;
    assert_eq!(store.authorize(&different), Err(AuthorizationError::Denied));
    assert_ne!(store.replace(vec![])?, revision);
    assert_eq!(store.authorize(&request), Err(AuthorizationError::Denied));
    Ok(())
}
