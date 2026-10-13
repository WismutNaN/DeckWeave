// SPDX-License-Identifier: GPL-3.0-or-later

use deckweave_automation::{
    CompiledEndpointContract, CompiledSchemaContract, ConditionOperator,
    EndpointAvailabilityPolicy, ExecutableGraph, GraphEdge, GraphEdgeId, GraphEdgeKind,
    GraphFormatVersion, GraphLimits, GraphNode, GraphNodeKind, GraphPort, GraphPortId,
    GraphPortKind, GraphPortRef, GraphPortRole, IngressContext, IngressItem, IngressKind,
    IngressQueue, IngressReliability, QueueAdmission, SchedulerFault, SchedulerIntent,
    SchedulerState, SchemaValueKind, SourceSubscription, StateSlot, WorkspaceScheduler, transition,
};
use deckweave_domain::{
    automation::{
        CommandOutcome, DataPolicy, ExecutionKind, ExecutionPolicy, InvocationReply,
        OperationDescriptor, OperationLimits,
    },
    capability::{
        AvailabilityDescriptor, PortAvailability, PortDescriptor, PortDirection, PortKind,
        PortLifetime, PortSemantics,
    },
    endpoint::{EndpointTargetRef, PortPath, StableEndpointRef},
    ids::{
        ActivationEpoch, CapabilityId, DescriptorRevision, DeviceIdentity, ElementId, GraphNodeId,
        HostBootId, IngressOrdinal, JobId, NamespacedKind, NodeId, OperationId, PackageId, PortId,
        ProviderId,
    },
    schema::{SchemaRef, SchemaVersion, SchemaVersionRange},
};
use serde_json::{Value, json};
use std::{collections::BTreeMap, error::Error};

#[path = "support/structured_arguments.rs"]
mod structured_arguments;

fn schema(id: &str) -> Result<SchemaRef, Box<dyn Error>> {
    Ok(SchemaRef {
        id: NamespacedKind::try_new(id)?,
        version: SchemaVersion { major: 1, minor: 0 },
    })
}

fn endpoint(element: &str, port: &str) -> Result<StableEndpointRef, Box<dyn Error>> {
    Ok(StableEndpointRef {
        package_id: PackageId::try_new("org.deckweave.virtual")?,
        node_id: NodeId::try_new("node")?,
        target: EndpointTargetRef::Device {
            provider_id: ProviderId::try_new("provider")?,
            device_identity: DeviceIdentity::try_new("device")?,
        },
        port: PortPath {
            element_id: ElementId::try_new(element)?,
            capability_id: CapabilityId::try_new("automation")?,
            port_id: PortId::try_new(port)?,
        },
    })
}

fn graph_port(
    id: &str,
    kind: GraphPortKind,
    role: GraphPortRole,
    schema: SchemaRef,
) -> Result<GraphPort, Box<dyn Error>> {
    Ok(GraphPort {
        id: GraphPortId::try_new(id)?,
        kind,
        role,
        schema,
        unit: None,
        range: None,
    })
}

fn node_ref(node: &str, port: &str) -> Result<GraphPortRef, Box<dyn Error>> {
    Ok(GraphPortRef {
        node_id: GraphNodeId::try_new(node)?,
        port_id: GraphPortId::try_new(port)?,
    })
}

fn edge(
    id: &str,
    kind: GraphEdgeKind,
    source: (&str, &str),
    target: (&str, &str),
) -> Result<GraphEdge, Box<dyn Error>> {
    Ok(GraphEdge {
        id: GraphEdgeId::try_new(id)?,
        kind,
        source: node_ref(source.0, source.1)?,
        target: node_ref(target.0, target.1)?,
        extensions: BTreeMap::new(),
    })
}

fn descriptor(
    endpoint: &StableEndpointRef,
    direction: PortDirection,
    semantics: PortSemantics,
    schema: SchemaRef,
) -> Result<PortDescriptor, Box<dyn Error>> {
    Ok(PortDescriptor {
        port_id: endpoint.port.port_id.clone(),
        kind: PortKind::try_new(match semantics {
            PortSemantics::Event => PortKind::EVENT_OUT,
            PortSemantics::Command => PortKind::COMMAND_IN,
            _ => PortKind::VALUE_IN,
        })?,
        direction,
        semantics,
        schema: schema.clone(),
        compatible_versions: SchemaVersionRange {
            minimum: schema.version,
            maximum: schema.version,
        },
        unit: None,
        range: None,
        coordinate_space: None,
        rate: None,
        required_permissions: vec![],
        availability: AvailabilityDescriptor {
            status: PortAvailability::Available,
            reason: None,
        },
        lifetime: PortLifetime::Always,
        session_generation: None,
        descriptor_revision: DescriptorRevision::try_new(1)?,
        template_id: None,
        tags: Default::default(),
        extensions: BTreeMap::new(),
    })
}

fn operation() -> Result<OperationDescriptor, Box<dyn Error>> {
    Ok(OperationDescriptor {
        operation_id: OperationId::try_new("echo")?,
        version: 1,
        input_schema: schema("org.deckweave.schema.unit")?,
        result_schema: schema("org.deckweave.schema.text")?,
        error_schema: schema("org.deckweave.schema.error")?,
        execution: ExecutionPolicy {
            kind: ExecutionKind::Short,
            progress: false,
            cancellation: false,
            reconciliation: false,
        },
        effects: vec![],
        idempotency: None,
        limits: OperationLimits {
            deadline_ms: 1_000,
            maximum_input_bytes: 16,
            maximum_output_bytes: 32,
            maximum_concurrent_invocations: 4,
            maximum_queue_depth: 16,
            maximum_per_minute: 600,
        },
        resources: vec![],
        data_policy: DataPolicy {
            sensitive_fields: vec![],
            external_recipients: vec![],
            trace_allowed: true,
            retention_seconds: 60,
        },
        extensions: BTreeMap::new(),
    })
}

fn plan() -> Result<ExecutableGraph, Box<dyn Error>> {
    let boolean = schema("org.deckweave.schema.boolean")?;
    let event = schema("org.deckweave.schema.event")?;
    let unit = schema("org.deckweave.schema.unit")?;
    let text = schema("org.deckweave.schema.text")?;
    let error = schema("org.deckweave.schema.error")?;
    let source_endpoint = endpoint("button", "pressed")?;
    let action_endpoint = endpoint("service", "invoke")?;
    let feedback_endpoint = endpoint("display", "text")?;
    let nodes = vec![
        GraphNode {
            id: GraphNodeId::try_new("source")?,
            kind: GraphNodeKind::Source {
                endpoint: source_endpoint.clone(),
                availability: EndpointAvailabilityPolicy::Deferred,
            },
            inputs: vec![],
            outputs: vec![graph_port(
                "value",
                GraphPortKind::Value,
                GraphPortRole::Value,
                boolean.clone(),
            )?],
            extensions: BTreeMap::new(),
        },
        GraphNode {
            id: GraphNodeId::try_new("condition")?,
            kind: GraphNodeKind::Condition {
                operator: ConditionOperator::IsTrue,
                operand: None,
            },
            inputs: vec![graph_port(
                "value",
                GraphPortKind::Value,
                GraphPortRole::Input,
                boolean.clone(),
            )?],
            outputs: vec![
                graph_port(
                    "true",
                    GraphPortKind::Flow,
                    GraphPortRole::True,
                    event.clone(),
                )?,
                graph_port(
                    "false",
                    GraphPortKind::Flow,
                    GraphPortRole::False,
                    event.clone(),
                )?,
            ],
            extensions: BTreeMap::new(),
        },
        GraphNode {
            id: GraphNodeId::try_new("action")?,
            kind: GraphNodeKind::Action {
                endpoint: action_endpoint.clone(),
                operation_id: OperationId::try_new("echo")?,
                availability: EndpointAvailabilityPolicy::Deferred,
            },
            inputs: vec![graph_port(
                "trigger",
                GraphPortKind::Flow,
                GraphPortRole::Trigger,
                event.clone(),
            )?],
            outputs: vec![
                graph_port(
                    "result",
                    GraphPortKind::Value,
                    GraphPortRole::Result,
                    text.clone(),
                )?,
                graph_port(
                    "error",
                    GraphPortKind::Value,
                    GraphPortRole::Error,
                    error.clone(),
                )?,
            ],
            extensions: BTreeMap::new(),
        },
        GraphNode {
            id: GraphNodeId::try_new("state")?,
            kind: GraphNodeKind::State {
                state_key: NamespacedKind::try_new("org.deckweave.last_result")?,
                initial_value: Some(Value::String("initial".into())),
            },
            inputs: vec![graph_port(
                "input",
                GraphPortKind::Value,
                GraphPortRole::Input,
                text.clone(),
            )?],
            outputs: vec![graph_port(
                "value",
                GraphPortKind::Value,
                GraphPortRole::Value,
                text.clone(),
            )?],
            extensions: BTreeMap::new(),
        },
        GraphNode {
            id: GraphNodeId::try_new("feedback")?,
            kind: GraphNodeKind::Feedback {
                endpoint: feedback_endpoint.clone(),
                availability: EndpointAvailabilityPolicy::Deferred,
            },
            inputs: vec![graph_port(
                "input",
                GraphPortKind::Value,
                GraphPortRole::Input,
                text.clone(),
            )?],
            outputs: vec![],
            extensions: BTreeMap::new(),
        },
    ];
    Ok(ExecutableGraph {
        action_arguments: Default::default(),
        format: GraphFormatVersion { major: 1, minor: 0 },
        nodes,
        edges: vec![
            edge(
                "source-condition",
                GraphEdgeKind::Data,
                ("source", "value"),
                ("condition", "value"),
            )?,
            edge(
                "condition-action",
                GraphEdgeKind::Flow,
                ("condition", "true"),
                ("action", "trigger"),
            )?,
            edge(
                "action-state",
                GraphEdgeKind::Data,
                ("action", "result"),
                ("state", "input"),
            )?,
            edge(
                "state-feedback",
                GraphEdgeKind::Data,
                ("state", "value"),
                ("feedback", "input"),
            )?,
        ],
        topological_order: ["source", "condition", "action", "state", "feedback"]
            .into_iter()
            .map(GraphNodeId::try_new)
            .collect::<Result<_, _>>()?,
        source_subscriptions: vec![SourceSubscription {
            node_id: GraphNodeId::try_new("source")?,
            endpoint: source_endpoint.clone(),
        }],
        state_slots: vec![StateSlot {
            index: 0,
            node_id: GraphNodeId::try_new("state")?,
            key: NamespacedKind::try_new("org.deckweave.last_result")?,
            schema: text.clone(),
            initial_value: Some(Value::String("initial".into())),
        }],
        endpoint_contracts: vec![
            CompiledEndpointContract {
                configured_instance_id: None,
                node_id: GraphNodeId::try_new("source")?,
                endpoint: source_endpoint.clone(),
                runtime_group_id: deckweave_domain::ids::RuntimeGroupId::try_new("control")?,
                contribution_id: deckweave_domain::ids::ContributionId::try_new("virtual")?,
                port: descriptor(
                    &source_endpoint,
                    PortDirection::Output,
                    PortSemantics::Event,
                    boolean.clone(),
                )?,
                operation: None,
            },
            CompiledEndpointContract {
                configured_instance_id: None,
                node_id: GraphNodeId::try_new("action")?,
                endpoint: action_endpoint.clone(),
                runtime_group_id: deckweave_domain::ids::RuntimeGroupId::try_new("control")?,
                contribution_id: deckweave_domain::ids::ContributionId::try_new("virtual")?,
                port: descriptor(
                    &action_endpoint,
                    PortDirection::Input,
                    PortSemantics::Command,
                    unit.clone(),
                )?,
                operation: Some(operation()?),
            },
            CompiledEndpointContract {
                configured_instance_id: None,
                node_id: GraphNodeId::try_new("feedback")?,
                endpoint: feedback_endpoint.clone(),
                runtime_group_id: deckweave_domain::ids::RuntimeGroupId::try_new("control")?,
                contribution_id: deckweave_domain::ids::ContributionId::try_new("virtual")?,
                port: descriptor(
                    &feedback_endpoint,
                    PortDirection::Input,
                    PortSemantics::Value,
                    text.clone(),
                )?,
                operation: None,
            },
        ],
        schema_contracts: vec![
            (boolean, SchemaValueKind::Boolean),
            (event, SchemaValueKind::Json),
            (unit, SchemaValueKind::Unit),
            (text, SchemaValueKind::String),
            (error, SchemaValueKind::Json),
        ]
        .into_iter()
        .map(|(schema, value_kind)| CompiledSchemaContract { schema, value_kind })
        .collect(),
    })
}

fn context(ordinal: u64) -> Result<IngressContext, Box<dyn Error>> {
    Ok(IngressContext {
        host_boot_id: HostBootId::try_new("boot")?,
        activation_epoch: ActivationEpoch::try_new(1)?,
        ordinal: IngressOrdinal::try_new(ordinal)?,
    })
}

fn signal(
    ordinal: u64,
    source: StableEndpointRef,
    value: Value,
    reliability: IngressReliability,
) -> Result<IngressItem, Box<dyn Error>> {
    Ok(IngressItem {
        context: context(ordinal)?,
        kind: IngressKind::Signal {
            source,
            payload: value,
            source_ticks: ordinal * 10,
            reliability,
        },
    })
}

fn state() -> Result<SchedulerState, Box<dyn Error>> {
    Ok(SchedulerState::new(
        HostBootId::try_new("boot")?,
        ActivationEpoch::try_new(1)?,
    ))
}

#[test]
fn coalescing_never_crosses_a_reliable_segment_boundary() -> Result<(), Box<dyn Error>> {
    let axis = endpoint("axis", "value")?;
    let button = endpoint("button", "pressed")?;
    let mut queue = IngressQueue::default();
    assert_eq!(
        queue.push(
            signal(1, axis.clone(), json!(0.1), IngressReliability::Coalescible)?,
            8,
        )?,
        QueueAdmission::Queued
    );
    queue.push(
        signal(2, button, json!(true), IngressReliability::Reliable)?,
        8,
    )?;
    queue.push(
        signal(3, axis, json!(0.2), IngressReliability::Coalescible)?,
        8,
    )?;
    let ordinals: Vec<_> = std::iter::from_fn(|| queue.pop())
        .map(|item| item.context.ordinal.get())
        .collect();
    assert_eq!(ordinals, vec![1, 2, 3]);
    Ok(())
}

#[test]
fn workspace_scheduler_is_the_single_bounded_owner_of_queue_and_state() -> Result<(), Box<dyn Error>>
{
    let mut scheduler = WorkspaceScheduler::new(
        plan()?,
        HostBootId::try_new("boot")?,
        ActivationEpoch::try_new(1)?,
        GraphLimits::default(),
    )?;
    assert_eq!(
        scheduler
            .state()
            .values
            .get(&GraphNodeId::try_new("state")?),
        Some(&json!("initial"))
    );
    scheduler.admit(signal(
        1,
        endpoint("button", "pressed")?,
        json!(true),
        IngressReliability::Reliable,
    )?)?;
    assert_eq!(scheduler.queued_items(), 1);
    let step = scheduler
        .step(10)?
        .ok_or_else(|| std::io::Error::other("expected scheduler step"))?;
    assert!(
        step.intents
            .iter()
            .any(|intent| matches!(intent, SchedulerIntent::Invoke { .. }))
    );
    assert_eq!(scheduler.state().pending.len(), 1);
    assert_eq!(scheduler.queued_items(), 0);
    Ok(())
}

#[test]
fn reliable_encoder_deltas_are_never_coalesced() -> Result<(), Box<dyn Error>> {
    let encoder = endpoint("encoder", "delta")?;
    let mut queue = IngressQueue::default();
    for (ordinal, delta) in [(1, 1), (2, 1), (3, -1)] {
        queue.push(
            signal(
                ordinal,
                encoder.clone(),
                json!(delta),
                IngressReliability::Reliable,
            )?,
            8,
        )?;
    }
    let values: Vec<_> = std::iter::from_fn(|| queue.pop())
        .map(|item| match item.kind {
            IngressKind::Signal { payload, .. } => payload,
            _ => Value::Null,
        })
        .collect();
    assert_eq!(values, vec![json!(1), json!(1), json!(-1)]);
    Ok(())
}

#[test]
fn overflow_returns_cleanup_without_using_queue_capacity() -> Result<(), Box<dyn Error>> {
    let source = endpoint("button", "pressed")?;
    let mut queue = IngressQueue::default();
    queue.push(
        signal(1, source.clone(), json!(true), IngressReliability::Reliable)?,
        1,
    )?;
    let overflow = match queue.push(
        signal(2, source, json!(true), IngressReliability::Reliable)?,
        1,
    ) {
        Err(overflow) => overflow,
        Ok(_) => return Err(std::io::Error::other("expected ingress overflow").into()),
    };
    assert_eq!(overflow.ordinal.get(), 2);
    assert!(matches!(
        overflow.cleanup,
        SchedulerIntent::CleanupAll { .. }
    ));
    assert_eq!(queue.len(), 1);
    Ok(())
}

#[test]
fn concurrent_results_remain_run_local_and_terminal_is_applied_once() -> Result<(), Box<dyn Error>>
{
    let plan = plan()?;
    let source = endpoint("button", "pressed")?;
    let first = transition(
        &plan,
        state()?,
        signal(1, source.clone(), json!(true), IngressReliability::Reliable)?,
        0,
        &GraphLimits::default(),
    )?;
    let first_invocation = invocation_id(&first.intents)?;
    let second = transition(
        &plan,
        first.state,
        signal(2, source, json!(true), IngressReliability::Reliable)?,
        1,
        &GraphLimits::default(),
    )?;
    let second_invocation = invocation_id(&second.intents)?;
    assert_ne!(first_invocation, second_invocation);
    assert_eq!(second.state.pending.len(), 2);

    let completed_second = transition(
        &plan,
        second.state,
        IngressItem {
            context: context(3)?,
            kind: IngressKind::Invocation {
                reply: InvocationReply::Terminal {
                    invocation_id: second_invocation,
                    outcome: CommandOutcome::Succeeded {
                        result: json!("second"),
                    },
                },
            },
        },
        2,
        &GraphLimits::default(),
    )?;
    assert_eq!(completed_second.state.pending.len(), 1);
    assert_eq!(
        feedback_values(&completed_second.intents),
        vec![json!("second")]
    );

    let completed_first = transition(
        &plan,
        completed_second.state,
        IngressItem {
            context: context(4)?,
            kind: IngressKind::Invocation {
                reply: InvocationReply::Terminal {
                    invocation_id: first_invocation.clone(),
                    outcome: CommandOutcome::Succeeded {
                        result: json!("first"),
                    },
                },
            },
        },
        3,
        &GraphLimits::default(),
    )?;
    assert_eq!(
        feedback_values(&completed_first.intents),
        vec![json!("first")]
    );

    let duplicate = transition(
        &plan,
        completed_first.state,
        IngressItem {
            context: context(5)?,
            kind: IngressKind::Invocation {
                reply: InvocationReply::Terminal {
                    invocation_id: first_invocation,
                    outcome: CommandOutcome::Succeeded {
                        result: json!("first"),
                    },
                },
            },
        },
        4,
        &GraphLimits::default(),
    )?;
    assert_eq!(duplicate.faults, vec![SchedulerFault::DuplicateTerminal]);
    assert!(feedback_values(&duplicate.intents).is_empty());
    Ok(())
}

#[test]
fn stale_epoch_is_rejected_without_advancing_order() -> Result<(), Box<dyn Error>> {
    let plan = plan()?;
    let mut stale = signal(
        1,
        endpoint("button", "pressed")?,
        json!(true),
        IngressReliability::Reliable,
    )?;
    stale.context.activation_epoch = ActivationEpoch::try_new(2)?;
    let result = transition(&plan, state()?, stale, 0, &GraphLimits::default())?;
    assert_eq!(result.faults, vec![SchedulerFault::StaleIngress]);
    assert_eq!(result.state.last_ordinal, None);
    assert!(result.state.pending.is_empty());
    Ok(())
}

#[test]
fn identical_ordered_fixture_produces_identical_state_and_intents() -> Result<(), Box<dyn Error>> {
    let plan = plan()?;
    let item = signal(
        1,
        endpoint("button", "pressed")?,
        json!(true),
        IngressReliability::Reliable,
    )?;
    let first = transition(&plan, state()?, item.clone(), 50, &GraphLimits::default())?;
    let second = transition(&plan, state()?, item, 50, &GraphLimits::default())?;
    assert_eq!(first, second);
    Ok(())
}

#[test]
fn state_updates_route_feedback_and_false_condition_stops_the_run() -> Result<(), Box<dyn Error>> {
    let plan = plan()?;
    let false_signal = transition(
        &plan,
        state()?,
        signal(
            1,
            endpoint("button", "pressed")?,
            json!(false),
            IngressReliability::Reliable,
        )?,
        0,
        &GraphLimits::default(),
    )?;
    assert!(
        !false_signal
            .intents
            .iter()
            .any(|intent| matches!(intent, SchedulerIntent::Invoke { .. }))
    );
    assert!(false_signal.state.active_runs.is_empty());

    let updated = transition(
        &plan,
        false_signal.state,
        IngressItem {
            context: context(2)?,
            kind: IngressKind::StateUpdate {
                node_id: GraphNodeId::try_new("state")?,
                value: json!("manual"),
            },
        },
        1,
        &GraphLimits::default(),
    )?;
    assert_eq!(feedback_values(&updated.intents), vec![json!("manual")]);
    assert_eq!(
        updated.state.values.get(&GraphNodeId::try_new("state")?),
        Some(&json!("manual"))
    );
    Ok(())
}

#[test]
fn faults_invalidate_activation_and_cleanup_only_once() -> Result<(), Box<dyn Error>> {
    for fault in [
        IngressKind::GrantFault,
        IngressKind::TopologyFault {
            endpoint: endpoint("button", "pressed")?,
        },
    ] {
        let mut scheduler = WorkspaceScheduler::new(
            plan()?,
            HostBootId::try_new("boot")?,
            ActivationEpoch::try_new(1)?,
            GraphLimits::default(),
        )?;
        scheduler.admit(signal(
            1,
            endpoint("button", "pressed")?,
            json!(true),
            IngressReliability::Reliable,
        )?)?;
        assert!(
            scheduler
                .step(0)?
                .ok_or("start")?
                .intents
                .iter()
                .any(|v| matches!(v, SchedulerIntent::Invoke { .. }))
        );
        scheduler.admit(IngressItem {
            context: context(2)?,
            kind: fault.clone(),
        })?;
        let stopped = scheduler.step(1)?.ok_or("fault")?;
        assert!(
            stopped
                .intents
                .iter()
                .any(|v| matches!(v, SchedulerIntent::CancelInvocation { .. }))
        );
        assert_eq!(
            stopped
                .intents
                .iter()
                .filter(|v| matches!(v, SchedulerIntent::CleanupAll { .. }))
                .count(),
            1
        );
        assert!(scheduler.state().pending.is_empty());
        assert!(scheduler.state().invalidated);
        // A pre-commit apply abort cannot revive invalid runtime bindings/grants.
        scheduler.reopen_after_barrier();
        assert!(scheduler.state().closed);
        for (ordinal, kind) in [
            (3, fault),
            (4, IngressKind::Barrier),
            (5, IngressKind::Barrier),
        ] {
            scheduler.admit(IngressItem {
                context: context(ordinal)?,
                kind,
            })?;
            assert!(
                scheduler
                    .step(ordinal)?
                    .ok_or("duplicate close")?
                    .intents
                    .is_empty()
            );
        }
        scheduler.admit(signal(
            6,
            endpoint("button", "pressed")?,
            json!(true),
            IngressReliability::Reliable,
        )?)?;
        let after = scheduler.step(6)?.ok_or("late signal")?;
        assert!(after.intents.is_empty());
        assert_eq!(after.faults, vec![SchedulerFault::BarrierClosed]);
    }
    Ok(())
}

#[test]
fn barrier_can_reopen_unless_a_fault_arrives_while_closed() -> Result<(), Box<dyn Error>> {
    for fault in [
        None,
        Some(IngressKind::GrantFault),
        Some(IngressKind::TopologyFault {
            endpoint: endpoint("button", "pressed")?,
        }),
    ] {
        let mut scheduler = WorkspaceScheduler::new(
            plan()?,
            HostBootId::try_new("boot")?,
            ActivationEpoch::try_new(1)?,
            GraphLimits::default(),
        )?;
        scheduler.admit(IngressItem {
            context: context(1)?,
            kind: IngressKind::Barrier,
        })?;
        assert!(
            scheduler
                .step(0)?
                .ok_or("barrier")?
                .intents
                .iter()
                .any(|v| matches!(v, SchedulerIntent::CleanupAll { .. }))
        );
        let invalidated = fault.is_some();
        if let Some(kind) = fault {
            scheduler.admit(IngressItem {
                context: context(2)?,
                kind,
            })?;
            assert!(scheduler.step(1)?.ok_or("fault")?.intents.is_empty());
        }
        scheduler.reopen_after_barrier();
        assert_eq!(scheduler.state().closed, invalidated);
    }
    Ok(())
}

#[test]
fn ingress_order_must_be_strictly_monotonic() -> Result<(), Box<dyn Error>> {
    let plan = plan()?;
    let source = endpoint("button", "pressed")?;
    let first = transition(
        &plan,
        state()?,
        signal(
            2,
            source.clone(),
            json!(false),
            IngressReliability::Reliable,
        )?,
        0,
        &GraphLimits::default(),
    )?;
    let error = transition(
        &plan,
        first.state,
        signal(2, source, json!(false), IngressReliability::Reliable)?,
        1,
        &GraphLimits::default(),
    )
    .err()
    .ok_or_else(|| std::io::Error::other("expected out-of-order rejection"))?;
    assert!(matches!(
        error,
        deckweave_automation::SchedulerTransitionError::IngressOutOfOrder {
            previous: 2,
            actual: 2
        }
    ));
    Ok(())
}

#[test]
fn accepted_job_times_out_and_is_cancelled_with_its_job_id() -> Result<(), Box<dyn Error>> {
    let plan = plan()?;
    let source = endpoint("button", "pressed")?;
    let started = transition(
        &plan,
        state()?,
        signal(1, source, json!(true), IngressReliability::Reliable)?,
        10,
        &GraphLimits::default(),
    )?;
    let invocation_id = invocation_id(&started.intents)?;
    let job_id = JobId::try_new("job")?;
    let accepted = transition(
        &plan,
        started.state,
        IngressItem {
            context: context(2)?,
            kind: IngressKind::Invocation {
                reply: InvocationReply::AcceptedJob {
                    invocation_id: invocation_id.clone(),
                    job_id: job_id.clone(),
                },
            },
        },
        20,
        &GraphLimits::default(),
    )?;
    assert!(accepted.intents.is_empty());
    let timed_out = transition(
        &plan,
        accepted.state,
        IngressItem {
            context: context(3)?,
            kind: IngressKind::TimerDeadline {
                invocation_id: invocation_id.clone(),
            },
        },
        1_010,
        &GraphLimits::default(),
    )?;
    assert!(timed_out.faults.contains(&SchedulerFault::DeadlineExceeded));
    assert!(timed_out.intents.iter().any(|intent| matches!(
        intent,
        SchedulerIntent::CancelInvocation {
            invocation_id: actual_invocation,
            job_id: Some(actual_job),
            ..
        } if *actual_invocation == invocation_id && *actual_job == job_id
    )));
    assert!(timed_out.state.pending.is_empty());
    Ok(())
}

#[test]
fn a_terminal_reply_at_or_after_deadline_cannot_apply_late_effects() -> Result<(), Box<dyn Error>> {
    let plan = plan()?;
    let started = transition(
        &plan,
        state()?,
        signal(
            1,
            endpoint("button", "pressed")?,
            json!(true),
            IngressReliability::Reliable,
        )?,
        10,
        &GraphLimits::default(),
    )?;
    let invocation_id = invocation_id(&started.intents)?;
    let late = transition(
        &plan,
        started.state,
        IngressItem {
            context: context(2)?,
            kind: IngressKind::Invocation {
                reply: InvocationReply::Terminal {
                    invocation_id,
                    outcome: CommandOutcome::Succeeded {
                        result: json!("late"),
                    },
                },
            },
        },
        1_010,
        &GraphLimits::default(),
    )?;
    assert!(late.faults.contains(&SchedulerFault::DeadlineExceeded));
    assert!(feedback_values(&late.intents).is_empty());
    assert!(late.state.pending.is_empty());
    Ok(())
}

#[test]
fn graph_budgets_reject_new_work_without_dropping_existing_runs() -> Result<(), Box<dyn Error>> {
    let plan = plan()?;
    let source = endpoint("button", "pressed")?;
    let mut limits = GraphLimits {
        maximum_timers: 1,
        ..GraphLimits::default()
    };
    let first = transition(
        &plan,
        state()?,
        signal(1, source.clone(), json!(true), IngressReliability::Reliable)?,
        0,
        &limits,
    )?;
    let first_invocation = invocation_id(&first.intents)?;
    let second = transition(
        &plan,
        first.state,
        signal(2, source, json!(true), IngressReliability::Reliable)?,
        1,
        &limits,
    )?;
    assert!(second.faults.contains(&SchedulerFault::BudgetExceeded(
        deckweave_automation::GraphLimitKind::Timers
    )));
    assert!(second.state.pending.contains_key(&first_invocation));
    assert_eq!(second.state.pending.len(), 1);

    limits.maximum_payload_bytes = 1;
    let oversized = transition(
        &plan,
        second.state,
        signal(
            3,
            endpoint("button", "pressed")?,
            json!(true),
            IngressReliability::Reliable,
        )?,
        2,
        &limits,
    )?;
    assert!(oversized.faults.contains(&SchedulerFault::BudgetExceeded(
        deckweave_automation::GraphLimitKind::PayloadBytes
    )));
    assert!(oversized.state.pending.contains_key(&first_invocation));
    Ok(())
}

#[test]
fn malformed_and_oversized_results_never_reach_feedback() -> Result<(), Box<dyn Error>> {
    for (result, expected) in [
        (json!(42), SchedulerFault::MalformedResult),
        (
            json!("this result is deliberately longer than thirty-two bytes"),
            SchedulerFault::OversizedResult,
        ),
    ] {
        let plan = plan()?;
        let started = transition(
            &plan,
            state()?,
            signal(
                1,
                endpoint("button", "pressed")?,
                json!(true),
                IngressReliability::Reliable,
            )?,
            0,
            &GraphLimits::default(),
        )?;
        let invocation_id = invocation_id(&started.intents)?;
        let completed = transition(
            &plan,
            started.state,
            IngressItem {
                context: context(2)?,
                kind: IngressKind::Invocation {
                    reply: InvocationReply::Terminal {
                        invocation_id,
                        outcome: CommandOutcome::Succeeded { result },
                    },
                },
            },
            1,
            &GraphLimits::default(),
        )?;
        assert!(completed.faults.contains(&expected));
        assert!(feedback_values(&completed.intents).is_empty());
        assert!(completed.state.pending.is_empty());
    }
    Ok(())
}

fn invocation_id(
    intents: &[SchedulerIntent],
) -> Result<deckweave_domain::ids::InvocationId, Box<dyn Error>> {
    intents
        .iter()
        .find_map(|intent| match intent {
            SchedulerIntent::Invoke { invocation_id, .. } => Some(invocation_id.clone()),
            _ => None,
        })
        .ok_or_else(|| std::io::Error::other("missing invocation intent").into())
}

fn feedback_values(intents: &[SchedulerIntent]) -> Vec<Value> {
    intents
        .iter()
        .filter_map(|intent| match intent {
            SchedulerIntent::Feedback { value, .. } => Some(value.clone()),
            _ => None,
        })
        .collect()
}
