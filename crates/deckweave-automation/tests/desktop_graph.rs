// SPDX-License-Identifier: GPL-3.0-or-later
//! Fixture is produced by the desktop's actual assignment code; TS checks prevent drift.
use deckweave_automation::*;
use deckweave_domain::{automation::OperationDescriptor, capability::PortDescriptor, ids::*};
use serde_json::{Value, json};
use std::error::Error;

#[test]
fn desktop_assignment_compiles_and_only_press_dispatches_the_first_step()
-> Result<(), Box<dyn Error>> {
    let fixture: Value = serde_json::from_str(include_str!(
        "../../../ui/desktop/tests/fixtures/assignment.json"
    ))?;
    let catalog = &fixture["catalog"];
    let schemas = catalog["schemas"]
        .as_array()
        .ok_or("schemas")?
        .iter()
        .map(|s| {
            Ok(CatalogSchema {
                schema: serde_json::from_value(s["schema"].clone())?,
                value_kind: serde_json::from_value(s["shape"].clone())?,
                unit: None,
                range: None,
            })
        })
        .collect::<Result<Vec<_>, serde_json::Error>>()?;
    let endpoints=catalog["endpoints"].as_array().ok_or("endpoints")?.iter().map(|e| ->Result<_,Box<dyn Error>> {
        let action=!e["operation"].is_null();
        let mut port=e["port"].clone();
        let object=port.as_object_mut().ok_or("port")?;
        object.insert("direction".into(),json!(if action {"input"} else {"output"}));
        object.insert("semantics".into(),json!(if action {"command"} else {"event"}));
        object.insert("compatibleVersions".into(),json!({"minimum":{"major":1,"minor":0},"maximum":{"major":1,"minor":0}}));
        object.insert("requiredPermissions".into(),json!([]));
        object.insert("lifetime".into(),json!("always"));
        object.insert("descriptorRevision".into(),json!(1));
        let operation:Option<OperationDescriptor>=if action {
            let mut op=e["operation"].clone();let fields=op.as_object_mut().ok_or("op")?;
            fields.insert("version".into(),json!(1));
            fields.insert("execution".into(),json!({"kind":"short","progress":false,"cancellation":false,"reconciliation":false}));
            fields.insert("effects".into(),json!([]));fields.insert("resources".into(),json!([]));
            fields.insert("limits".into(),json!({"deadlineMs":1000,"maximumInputBytes":4096,"maximumOutputBytes":4096,"maximumConcurrentInvocations":4,"maximumQueueDepth":16,"maximumPerMinute":600}));
            fields.insert("dataPolicy".into(),json!({"sensitiveFields":[],"externalRecipients":[],"traceAllowed":true,"retentionSeconds":60}));
            Some(serde_json::from_value(op)?)
        }else{None};
        Ok(CatalogEndpoint{endpoint:serde_json::from_value(e["endpoint"].clone())?,port:serde_json::from_value::<PortDescriptor>(port)?,operation,
            runtime_requirement:RuntimeRequirement{configuration:None,package_id:serde_json::from_value(catalog["package"]["packageId"].clone())?,package_version:"1.0.0".into(),artifact_sha256:"a".repeat(64),runtime_group_id:serde_json::from_value(e["runtimeGroupId"].clone())?,contribution_id:serde_json::from_value(e["contributionId"].clone())?}})
    }).collect::<Result<Vec<_>,_>>()?;
    let source = endpoints[0].endpoint.clone();
    let compiler = GraphPlanCompiler::new(
        DescriptorCatalog::try_new(schemas, endpoints)?,
        GraphLimits::default(),
    )?;
    let lock: Vec<deckweave_workspace::PackageLockEntry> =
        serde_json::from_value(fixture["draft"]["packageLock"].clone())?;
    let plan = compiler.compile_document(
        &fixture["draft"]["document"],
        &lock,
        "desktop-lock",
        &CompileFence {
            registry_revision: RegistryRevision::try_new(1)?,
            grant_revision: GrantRevision::try_new(1)?,
        },
    )?;
    let graph = plan.executable_graph().ok_or("executable")?;
    let boot = HostBootId::try_new("desktop-test")?;
    let epoch = ActivationEpoch::try_new(1)?;
    for pressed in [false, true] {
        let result = transition(
            graph,
            SchedulerState::new(boot.clone(), epoch),
            IngressItem {
                context: IngressContext {
                    host_boot_id: boot.clone(),
                    activation_epoch: epoch,
                    ordinal: IngressOrdinal::try_new(1)?,
                },
                kind: IngressKind::Signal {
                    source: source.clone(),
                    payload: json!(pressed),
                    source_ticks: 0,
                    reliability: IngressReliability::Reliable,
                },
            },
            1,
            &GraphLimits::default(),
        )?;
        let calls = result
            .intents
            .iter()
            .filter_map(|i| {
                if let SchedulerIntent::Invoke { input, .. } = i {
                    Some(input)
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();
        assert_eq!(calls.len(), usize::from(pressed));
        if pressed {
            assert_eq!(calls[0], &json!({"text":"Hello from the editor"}));
        }
    }
    Ok(())
}
