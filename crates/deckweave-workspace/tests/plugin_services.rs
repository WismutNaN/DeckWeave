// SPDX-License-Identifier: GPL-3.0-or-later
use deckweave_domain::{ids::*, schema::*};
use deckweave_workspace::plugin_services::*;
use std::{collections::BTreeMap, error::Error};

fn instance(id: &str) -> Result<ServiceInstance, Box<dyn Error>> {
    Ok(ServiceInstance {
        instance_id: PluginInstanceId::try_new(id)?,
        package_id: PackageId::try_new(format!("org.fixture.{id}"))?,
        locked_version: "1.0.0".into(),
        artifact_sha256: "a".repeat(64),
        provides: vec![contract()?],
        requires: BTreeMap::new(),
        bindings: BTreeMap::new(),
    })
}
fn contract() -> Result<SchemaRef, Box<dyn Error>> {
    Ok(SchemaRef {
        id: NamespacedKind::try_new("org.fixture.service")?,
        version: SchemaVersion { major: 1, minor: 0 },
    })
}
fn depend(
    consumer: &mut ServiceInstance,
    provider: &ServiceInstance,
) -> Result<(), Box<dyn Error>> {
    let role = NamespacedKind::try_new("org.fixture.role")?;
    consumer.requires.insert(
        role.clone(),
        ServiceRequirement {
            contract: contract()?,
            optional: false,
        },
    );
    consumer.bindings.insert(
        role,
        ServiceBinding {
            provider_instance: provider.instance_id.clone(),
            contract: contract()?,
        },
    );
    Ok(())
}
#[test]
fn explicit_transitive_closure_missing_optional_cycle_and_artifact_conflict()
-> Result<(), Box<dyn Error>> {
    let mut layout = instance("layout")?;
    let mut windows = instance("windows")?;
    let platform = instance("platform")?;
    depend(&mut layout, &windows)?;
    depend(&mut windows, &platform)?;
    let roots = vec![layout.instance_id.clone()];
    let mut all: BTreeMap<_, _> = [layout.clone(), windows.clone(), platform.clone()]
        .into_iter()
        .map(|i| (i.instance_id.clone(), i))
        .collect();
    assert_eq!(
        resolve_services(&all, &roots)?,
        vec![
            platform.instance_id.clone(),
            windows.instance_id.clone(),
            layout.instance_id.clone()
        ]
    );
    all.remove(&platform.instance_id);
    assert!(matches!(
        resolve_services(&all, &roots),
        Err(ServiceResolutionError::Missing(_))
    ));
    let role = NamespacedKind::try_new("org.fixture.role")?;
    windows.requires.get_mut(&role).ok_or("role")?.optional = true;
    windows.bindings.clear();
    all.insert(windows.instance_id.clone(), windows.clone());
    assert_eq!(resolve_services(&all, &roots)?.len(), 2);
    depend(&mut windows, &layout)?;
    all.insert(windows.instance_id.clone(), windows.clone());
    assert!(matches!(
        resolve_services(&all, &roots),
        Err(ServiceResolutionError::Cycle(_))
    ));
    windows.bindings.clear();
    windows.requires.clear();
    windows.package_id = layout.package_id.clone();
    windows.artifact_sha256 = "b".repeat(64);
    all.insert(windows.instance_id.clone(), windows);
    assert!(matches!(
        resolve_services(&all, &roots),
        Err(ServiceResolutionError::Version(_))
    ));
    Ok(())
}
