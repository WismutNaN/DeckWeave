// SPDX-License-Identifier: GPL-3.0-or-later
use super::*;
use deckweave_automation::{ActionArguments, ValueBinding};
use deckweave_domain::schema::PayloadSchema;

fn structured_plan() -> Result<ExecutableGraph, Box<dyn Error>> {
    let mut plan = plan()?;
    let record = schema("org.deckweave.schema.task")?;
    let shape: PayloadSchema = serde_json::from_value(json!({"type":"record","fields":{
        "title":{"schema":{"type":"string","maximumBytes":40}},
        "description":{"schema":{"type":"string","maximumBytes":40}}
    }}))?;
    plan.schema_contracts.push(CompiledSchemaContract {
        schema: record.clone(),
        value_kind: SchemaValueKind::Structured(shape),
    });
    plan.endpoint_contracts[1]
        .operation
        .as_mut()
        .ok_or("operation")?
        .input_schema = record;
    plan.endpoint_contracts[1]
        .operation
        .as_mut()
        .ok_or("operation")?
        .limits
        .maximum_input_bytes = 512;
    plan.schema_contracts[0].value_kind = SchemaValueKind::Json;
    plan.nodes
        .iter_mut()
        .find(|node| node.id.as_str() == "action")
        .ok_or("action")?
        .inputs[0]
        .kind = GraphPortKind::Value;
    plan.nodes
        .iter_mut()
        .find(|node| node.id.as_str() == "action")
        .ok_or("action")?
        .inputs[0]
        .role = GraphPortRole::Input;
    plan.edges = vec![edge(
        "source-action",
        GraphEdgeKind::Data,
        ("source", "value"),
        ("action", "trigger"),
    )?];
    plan.action_arguments.insert(
        GraphNodeId::try_new("action")?,
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
    Ok(plan)
}

#[test]
fn concurrent_transcripts_assemble_isolated_named_arguments_and_missing_field_never_invokes()
-> Result<(), Box<dyn Error>> {
    let plan = structured_plan()?;
    let limits = GraphLimits::default();
    let first = transition(
        &plan,
        state()?,
        signal(
            1,
            endpoint("button", "pressed")?,
            json!({"text":"first"}),
            IngressReliability::Reliable,
        )?,
        100,
        &limits,
    )?;
    let second = transition(
        &plan,
        first.state,
        signal(
            2,
            endpoint("button", "pressed")?,
            json!({"text":"second"}),
            IngressReliability::Reliable,
        )?,
        101,
        &limits,
    )?;
    let inputs = first
        .intents
        .iter()
        .chain(&second.intents)
        .filter_map(|intent| match intent {
            SchedulerIntent::Invoke { input, .. } => Some(input.clone()),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(
        inputs,
        vec![
            json!({"title":"first","description":"voice"}),
            json!({"title":"second","description":"voice"})
        ]
    );
    assert_eq!(second.state.run_results.len(), 2);
    let bad = transition(
        &plan,
        second.state,
        signal(
            3,
            endpoint("button", "pressed")?,
            json!({"missing":"text"}),
            IngressReliability::Reliable,
        )?,
        102,
        &limits,
    )?;
    assert!(bad.faults.contains(&SchedulerFault::MalformedInput));
    assert!(
        !bad.intents
            .iter()
            .any(|intent| matches!(intent, SchedulerIntent::Invoke { .. }))
    );
    assert_eq!(bad.state.pending.len(), 2);
    let closed = transition(
        &plan,
        bad.state,
        IngressItem {
            context: context(4)?,
            kind: IngressKind::Barrier,
        },
        103,
        &limits,
    )?;
    assert!(closed.state.run_results.is_empty());
    Ok(())
}
