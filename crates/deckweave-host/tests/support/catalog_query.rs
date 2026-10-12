// SPDX-License-Identifier: GPL-3.0-or-later
use super::*;
use deckweave_application::{AppQueryResult, plugin_catalog::*};

#[derive(Default)]
struct Catalog(AtomicUsize);
impl PluginCatalogSource for Catalog {
    fn catalog(
        &self,
        request: &PluginCatalogRequest,
    ) -> Result<PluginCatalogSnapshot, PluginCatalogError> {
        self.0.fetch_add(1, Ordering::SeqCst);
        Ok(PluginCatalogSnapshot {
            format_version: 1,
            package: request.clone(),
            manifest_sha256: "f".repeat(64),
            schemas: vec![],
            endpoints: vec![],
            translations: None,
        })
    }
}
#[test]
fn catalog_query_authorizes_before_lookup_and_rejects_unbounded_selectors()
-> Result<(), Box<dyn Error>> {
    let directory = tempfile::tempdir()?;
    let store = Arc::new(CharacterizationStore::new(DraftStoreError::NotFound));
    let authorizer = Arc::new(InMemoryApiAuthorizer::new());
    let catalog = Arc::new(Catalog::default());
    let gateway = service(directory.path().into(), store.clone(), authorizer.clone())?
        .with_plugin_catalog(catalog.clone());
    let client = context("catalog-reader", "catalog-trace")?;
    let request = PluginCatalogRequest {
        package_id: deckweave_domain::ids::PackageId::try_new("org.example.plugin")?,
        version: "1.0.0".into(),
        artifact_sha256: "a".repeat(64),
    };
    let query = AppQuery::PluginCatalog {
        package: request.clone(),
    };
    assert_eq!(
        gateway
            .query(&client, query.clone())
            .err()
            .ok_or("unauthorized query succeeded")?
            .code,
        ApiErrorCode::PermissionDenied
    );
    assert_eq!(catalog.0.load(Ordering::SeqCst), 0);
    authorizer.grant(client.principal_id.clone(), [ApiAuthScope::Read]);
    let decoded: AppQuery = serde_json::from_slice(&serde_json::to_vec(&query)?)?;
    let AppQueryResult::PluginCatalog(snapshot) = gateway.query(&client, decoded)? else {
        return Err("wrong query result".into());
    };
    assert_eq!(snapshot.package, request);
    assert_eq!(catalog.0.load(Ordering::SeqCst), 1);
    let mut invalid = request;
    invalid.version = "x".repeat(129);
    assert_eq!(
        gateway
            .query(&client, AppQuery::PluginCatalog { package: invalid })
            .err()
            .ok_or("unbounded query succeeded")?
            .code,
        ApiErrorCode::InvalidCommand
    );
    assert_eq!(catalog.0.load(Ordering::SeqCst), 1);
    assert_eq!(store.calls(), 0);
    Ok(())
}
