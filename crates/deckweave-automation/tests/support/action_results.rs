// SPDX-License-Identifier: GPL-3.0-or-later
use super::*;
use deckweave_automation::{
    ActionArguments, IngressContext, IngressItem, IngressKind, IngressReliability, SchedulerIntent,
    SchedulerState, ValueBinding, transition,
};
use deckweave_domain::{
    automation::{CommandOutcome, InvocationReply},
    ids::{ActivationEpoch, HostBootId, IngressOrdinal},
    schema::PayloadSchema,
};
use serde_json::json;

#[test]
fn concurrent_task_creation_results_feed_the_matching_completion_action()
-> Result<(), Box<dyn Error>> {
    let (mut graph, initial) = argument_compiler::binding_fixture()?;
    let mut schemas = initial.catalog().schemas().to_vec();
    let mut endpoints = initial.catalog().endpoints().to_vec();
    let task_id = schema("org.deckweave.schema.task-id")?;
    let shape: PayloadSchema = serde_json::from_value(
        json!({"type":"record","fields":{"id":{"schema":{"type":"string","maximumBytes":40}}}}),
    )?;
    schemas.push(CatalogSchema {
        schema: task_id.clone(),
        value_kind: SchemaValueKind::Structured(shape),
        unit: None,
        range: None,
    });
    let create = endpoints
        .iter_mut()
        .find(|e| e.operation.is_some())
        .ok_or("create")?;
    create.operation.as_mut().ok_or("operation")?.result_schema = task_id.clone();
    let mut complete = create.clone();
    complete.endpoint = endpoint("tasks", "complete")?;
    complete.port.port_id = complete.endpoint.port.port_id.clone();
    complete.port.schema = task_id.clone();
    let complete_operation = complete.operation.as_mut().ok_or("complete")?;
    complete_operation.operation_id = OperationId::try_new("complete-task")?;
    complete_operation.input_schema = task_id.clone();
    let operation_id = complete_operation.operation_id.clone();
    let target = complete.endpoint.clone();
    endpoints.push(complete);
    graph.graph.nodes.retain(|n| n.id.as_str() != "feedback");
    graph
        .graph
        .nodes
        .iter_mut()
        .find(|n| n.id.as_str() == "action")
        .ok_or("action")?
        .outputs[0]
        .schema = task_id.clone();
    graph
        .graph
        .edges
        .retain(|e| e.target.node_id.as_str() != "feedback");
    let complete_node = GraphNodeId::try_new("complete")?;
    graph.graph.nodes.push(GraphNode {
        id: complete_node.clone(),
        kind: GraphNodeKind::Action {
            endpoint: target,
            operation_id,
            availability: EndpointAvailabilityPolicy::Required,
        },
        inputs: vec![graph_port(
            "input",
            GraphPortKind::Value,
            GraphPortRole::Input,
            task_id.clone(),
        )?],
        outputs: vec![graph_port(
            "result",
            GraphPortKind::Value,
            GraphPortRole::Result,
            task_id.clone(),
        )?],
        extensions: BTreeMap::new(),
    });
    graph.graph.edges.push(edge(
        "create-complete",
        GraphEdgeKind::Data,
        "action",
        "result",
        "complete",
        "input",
    )?);
    graph.graph.action_arguments.insert(
        complete_node,
        ActionArguments {
            trigger_port: GraphPortId::try_new("input")?,
            fields: BTreeMap::from([(
                "id".into(),
                ValueBinding::RunResult {
                    node_id: GraphNodeId::try_new("action")?,
                    pointer: "/id".into(),
                },
            )]),
        },
    );
    let compiler = GraphPlanCompiler::new(
        DescriptorCatalog::try_new(schemas, endpoints)?,
        GraphLimits::default(),
    )?;
    let compiled = compiler.compile_document(
        &serde_json::to_value(graph)?,
        &package_lock()?,
        "lock",
        &fence()?,
    )?;
    let plan = compiled.executable_graph().ok_or("graph")?;
    let boot = HostBootId::try_new("results")?;
    let epoch = ActivationEpoch::try_new(1)?;
    let ingress = |ordinal, kind| -> Result<IngressItem, Box<dyn Error>> {
        Ok(IngressItem {
            context: IngressContext {
                host_boot_id: boot.clone(),
                activation_epoch: epoch,
                ordinal: IngressOrdinal::try_new(ordinal)?,
            },
            kind,
        })
    };
    let mut state = SchedulerState::new(boot.clone(), epoch);
    let mut creates = Vec::new();
    for (ordinal, text) in [(1, "first"), (2, "second")] {
        let step = transition(
            plan,
            state,
            ingress(
                ordinal,
                IngressKind::Signal {
                    source: endpoint("button", "pressed")?,
                    payload: json!({"text":text}),
                    source_ticks: 0,
                    reliability: IngressReliability::Reliable,
                },
            )?,
            10,
            &GraphLimits::default(),
        )?;
        for intent in step.intents {
            if let SchedulerIntent::Invoke {
                invocation_id,
                run_id,
                ..
            } = intent
            {
                creates.push((invocation_id, run_id));
            }
        }
        state = step.state;
    }
    assert_eq!(creates.len(), 2);
    // Replies arrive in reverse order: no shared mutable "last result" may cross the runs.
    for (index, ordinal) in [(1, 3), (0, 4)] {
        let (id, run) = &creates[index];
        let expected = json!({"id":format!("task-{index}")});
        let step = transition(
            plan,
            state,
            ingress(
                ordinal,
                IngressKind::Invocation {
                    reply: InvocationReply::Terminal {
                        invocation_id: id.clone(),
                        outcome: CommandOutcome::Succeeded {
                            result: expected.clone(),
                        },
                    },
                },
            )?,
            11,
            &GraphLimits::default(),
        )?;
        assert!(step.faults.is_empty(), "{:?}", step.faults);
        assert!(step.intents.iter().any(|i| matches!(i, SchedulerIntent::Invoke { node_id, run_id, input, .. } if node_id.as_str() == "complete" && run_id == run && input == &expected)));
        state = step.state;
    }
    assert_eq!(state.pending.len(), 2);
    Ok(())
}
