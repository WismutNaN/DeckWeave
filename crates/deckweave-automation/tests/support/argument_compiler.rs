// SPDX-License-Identifier: GPL-3.0-or-later
use super::*;
use deckweave_automation::{
    ActionArguments, IngressContext, IngressItem, IngressKind, IngressReliability, SchedulerIntent,
    SchedulerState, ValueBinding, transition,
};
use deckweave_domain::{
    ids::{ActivationEpoch, HostBootId, IngressOrdinal},
    schema::PayloadSchema,
};
use serde_json::json;

pub(super) fn binding_fixture() -> Result<(GraphDocument, GraphPlanCompiler), Box<dyn Error>> {
    let original = catalog(PortAvailability::Available, false)?;
    let mut schemas = original.schemas().to_vec();
    let mut endpoints = original.endpoints().to_vec();
    let record = schema("org.deckweave.schema.task")?;
    let shape: PayloadSchema = serde_json::from_value(json!({"type":"record","fields":{
        "title":{"schema":{"type":"string","maximumBytes":40}},
        "description":{"schema":{"type":"string","maximumBytes":40}}
    }}))?;
    schemas.push(CatalogSchema {
        schema: record.clone(),
        value_kind: SchemaValueKind::Structured(shape),
        unit: None,
        range: None,
    });
    let event = schema("org.deckweave.schema.event")?;
    schemas
        .iter_mut()
        .find(|s| s.schema == event)
        .ok_or("event")?
        .value_kind = SchemaValueKind::Structured(serde_json::from_value(
        json!({"type":"record","fields":{"text":{"schema":{"type":"string","maximumBytes":40}}}}),
    )?);
    let target = endpoints
        .iter_mut()
        .find(|e| e.operation.is_some())
        .ok_or("action")?;
    target.port.schema = record.clone();
    let operation = target.operation.as_mut().ok_or("operation")?;
    operation.input_schema = record;
    operation.limits.maximum_input_bytes = 512;
    let compiler = GraphPlanCompiler::new(
        DescriptorCatalog::try_new(schemas, endpoints)?,
        GraphLimits::default(),
    )?;
    let mut graph = graph()?;
    let action = GraphNodeId::try_new("action")?;
    graph.graph.action_arguments.insert(
        action.clone(),
        ActionArguments {
            trigger_port: GraphPortId::try_new("trigger")?,
            fields: BTreeMap::from([
                (
                    "title".into(),
                    ValueBinding::RunResult {
                        node_id: GraphNodeId::try_new("source")?,
                        pointer: "/text".into(),
                    },
                ),
                (
                    "description".into(),
                    ValueBinding::Constant {
                        value: json!("voice"),
                    },
                ),
            ]),
        },
    );
    Ok((graph, compiler))
}

#[test]
fn named_arguments_are_compiled_and_concurrent_run_results_remain_separate()
-> Result<(), Box<dyn Error>> {
    let (mut graph, compiler) = binding_fixture()?;
    let action = GraphNodeId::try_new("action")?;
    let plan = compiler.compile_document(
        &serde_json::to_value(&graph)?,
        &package_lock()?,
        "lock",
        &fence()?,
    )?;
    let executable = plan.executable_graph().ok_or("graph")?;
    let epoch = ActivationEpoch::try_new(1)?;
    let boot = HostBootId::try_new("bindings-boot")?;
    let mut state = SchedulerState::new(boot.clone(), epoch);
    let mut inputs = Vec::new();
    for (index, text) in ["first", "second"].iter().enumerate() {
        let result = transition(
            executable,
            state,
            IngressItem {
                context: IngressContext {
                    host_boot_id: boot.clone(),
                    activation_epoch: epoch,
                    ordinal: IngressOrdinal::try_new(index as u64 + 1)?,
                },
                kind: IngressKind::Signal {
                    source: endpoint("button", "pressed")?,
                    payload: json!({"text":text}),
                    source_ticks: 0,
                    reliability: IngressReliability::Reliable,
                },
            },
            600_000 + index as u64,
            &GraphLimits::default(),
        )?;
        for intent in result.intents {
            if let SchedulerIntent::Invoke { input, .. } = intent {
                inputs.push(input);
            }
        }
        state = result.state;
    }
    assert_eq!(
        inputs,
        vec![
            json!({"title":"first","description":"voice"}),
            json!({"title":"second","description":"voice"})
        ]
    );
    graph
        .graph
        .action_arguments
        .get_mut(&action)
        .ok_or("args")?
        .fields
        .insert("title".into(), ValueBinding::Constant { value: json!(123) });
    assert!(matches!(
        compiler.compile_document(
            &serde_json::to_value(&graph)?,
            &package_lock()?,
            "lock",
            &fence()?
        ),
        Err(GraphCompileError::InvalidValue { .. })
    ));
    graph
        .graph
        .action_arguments
        .get_mut(&action)
        .ok_or("args")?
        .fields
        .insert(
            "title".into(),
            ValueBinding::RunResult {
                node_id: GraphNodeId::try_new("source")?,
                pointer: "/absent".into(),
            },
        );
    assert!(matches!(
        compiler.compile_document(
            &serde_json::to_value(&graph)?,
            &package_lock()?,
            "lock",
            &fence()?
        ),
        Err(GraphCompileError::InvalidValue { .. })
    ));
    graph
        .graph
        .action_arguments
        .get_mut(&action)
        .ok_or("args")?
        .fields
        .insert(
            "title".into(),
            ValueBinding::Trigger {
                pointer: "/text".into(),
            },
        );
    assert!(matches!(
        compiler.compile_document(
            &serde_json::to_value(&graph)?,
            &package_lock()?,
            "lock",
            &fence()?
        ),
        Err(GraphCompileError::InvalidValue { .. })
    ));
    Ok(())
}
