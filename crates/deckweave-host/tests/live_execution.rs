// SPDX-License-Identifier: GPL-3.0-or-later

use deckweave_automation::{
    BoundEndpoint, BoundExecutionGraph, CompiledEndpointContract, CompiledSchemaContract,
    EndpointAvailabilityPolicy, ExecutableGraph, GraphEdge, GraphEdgeId, GraphEdgeKind,
    GraphFormatVersion, GraphLimits, GraphNode, GraphNodeKind, GraphPort, GraphPortId,
    GraphPortKind, GraphPortRef, GraphPortRole, IngressReliability, SchemaValueKind,
    SourceSubscription,
};
use deckweave_domain::{
    capability::{
        AvailabilityDescriptor, PortAvailability, PortDescriptor, PortDirection, PortKind,
        PortLifetime, PortSemantics,
    },
    endpoint::{EndpointTargetRef, PortPath, RuntimeEndpointRef, StableEndpointRef},
    ids::{
        CapabilityId, ContributionId, DescriptorRevision, DeviceIdentity, ElementId, GraphNodeId,
        HostBootId, NodeId, PackageId, PluginSessionId, PortId, ProviderId, RuntimeGroupId,
        SessionGeneration,
    },
    schema::{SchemaRef, SchemaVersion, SchemaVersionRange},
};
use deckweave_host::{
    LiveExecutionOwner, RuntimeCommand, RuntimeDispatchFailure, RuntimeEvent, RuntimeEventKind,
    RuntimeEventSink, RuntimeIntentDispatcher,
};
use serde_json::json;
use std::{
    error::Error,
    sync::{Arc, Mutex},
};

#[derive(Default)]
struct RecordingDispatcher(Mutex<Vec<RuntimeCommand>>);

impl RuntimeIntentDispatcher for RecordingDispatcher {
    fn dispatch(&self, command: RuntimeCommand) -> Result<(), RuntimeDispatchFailure> {
        self.0
            .lock()
            .map_err(|_| RuntimeDispatchFailure {
                error: test_error(),
                possibly_sent: false,
            })?
            .push(command);
        Ok(())
    }
}

#[test]
fn barrier_closes_effects_and_old_epoch_cannot_reenter() -> Result<(), Box<dyn Error>> {
    let dispatcher = Arc::new(RecordingDispatcher::default());
    let owner = LiveExecutionOwner::new(HostBootId::try_new("boot-live")?, dispatcher.clone());
    let (graph, binding, source) = fixture()?;
    let first_epoch = owner.activate(Some(&graph), Some(&binding), GraphLimits::default())?;
    owner.publish(signal(first_epoch, source.clone(), true));
    assert_eq!(feedback_count(&dispatcher), 1);

    owner.close_barrier(10)?;
    assert!(
        dispatcher
            .0
            .lock()
            .map_err(|_| "commands poisoned")?
            .iter()
            .any(|command| matches!(command, RuntimeCommand::CleanupAll { .. }))
    );
    owner.publish(signal(first_epoch, source.clone(), true));
    assert_eq!(feedback_count(&dispatcher), 1);

    owner.resume_after_aborted_barrier()?;
    owner.publish(signal(first_epoch, source.clone(), true));
    assert_eq!(feedback_count(&dispatcher), 2);

    let second_epoch = owner.activate(Some(&graph), Some(&binding), GraphLimits::default())?;
    assert!(second_epoch > first_epoch);
    owner.publish(signal(first_epoch, source.clone(), true));
    assert_eq!(feedback_count(&dispatcher), 2);
    owner.publish(signal(second_epoch, source, true));
    assert_eq!(feedback_count(&dispatcher), 3);
    Ok(())
}

fn signal(
    activation_epoch: deckweave_domain::ids::ActivationEpoch,
    source: RuntimeEndpointRef,
    value: bool,
) -> RuntimeEvent {
    RuntimeEvent {
        activation_epoch,
        kind: RuntimeEventKind::Signal {
            source,
            payload: json!(value),
            source_ticks: 1,
            reliability: IngressReliability::Reliable,
        },
    }
}

fn feedback_count(dispatcher: &RecordingDispatcher) -> usize {
    dispatcher
        .0
        .lock()
        .map(|commands| {
            commands
                .iter()
                .filter(|command| matches!(command, RuntimeCommand::Feedback { .. }))
                .count()
        })
        .unwrap_or_default()
}

fn fixture() -> Result<(ExecutableGraph, BoundExecutionGraph, RuntimeEndpointRef), Box<dyn Error>> {
    let schema = schema()?;
    let source = stable("button", "pressed")?;
    let feedback = stable("indicator", "value")?;
    let source_node = GraphNodeId::try_new("source")?;
    let feedback_node = GraphNodeId::try_new("feedback")?;
    let graph = ExecutableGraph {
        format: GraphFormatVersion { major: 1, minor: 0 },
        nodes: vec![
            GraphNode {
                id: source_node.clone(),
                kind: GraphNodeKind::Source {
                    endpoint: source.clone(),
                    availability: EndpointAvailabilityPolicy::Required,
                },
                inputs: vec![],
                outputs: vec![graph_port("value", GraphPortRole::Value, schema.clone())?],
                extensions: Default::default(),
            },
            GraphNode {
                id: feedback_node.clone(),
                kind: GraphNodeKind::Feedback {
                    endpoint: feedback.clone(),
                    availability: EndpointAvailabilityPolicy::Required,
                },
                inputs: vec![graph_port("input", GraphPortRole::Input, schema.clone())?],
                outputs: vec![],
                extensions: Default::default(),
            },
        ],
        edges: vec![GraphEdge {
            id: GraphEdgeId::try_new("source-feedback")?,
            kind: GraphEdgeKind::Data,
            source: port_ref("source", "value")?,
            target: port_ref("feedback", "input")?,
            extensions: Default::default(),
        }],
        topological_order: vec![source_node.clone(), feedback_node.clone()],
        source_subscriptions: vec![SourceSubscription {
            node_id: source_node.clone(),
            endpoint: source.clone(),
        }],
        state_slots: vec![],
        endpoint_contracts: vec![
            contract(
                source_node.clone(),
                source.clone(),
                PortDirection::Output,
                PortSemantics::Event,
                schema.clone(),
            )?,
            contract(
                feedback_node.clone(),
                feedback.clone(),
                PortDirection::Input,
                PortSemantics::Value,
                schema.clone(),
            )?,
        ],
        schema_contracts: vec![CompiledSchemaContract {
            schema,
            value_kind: SchemaValueKind::Boolean,
        }],
    };
    let source = runtime(source)?;
    let feedback = runtime(feedback)?;
    Ok((
        graph,
        BoundExecutionGraph {
            endpoints: vec![
                BoundEndpoint {
                    node_id: source_node,
                    endpoint: source.clone(),
                },
                BoundEndpoint {
                    node_id: feedback_node,
                    endpoint: feedback,
                },
            ],
        },
        source,
    ))
}

fn schema() -> Result<SchemaRef, Box<dyn Error>> {
    Ok(SchemaRef {
        id: deckweave_domain::ids::NamespacedKind::try_new("org.deckweave.boolean")?,
        version: SchemaVersion { major: 1, minor: 0 },
    })
}

fn stable(element: &str, port: &str) -> Result<StableEndpointRef, Box<dyn Error>> {
    Ok(StableEndpointRef {
        package_id: PackageId::try_new("org.deckweave.live")?,
        node_id: NodeId::try_new("node")?,
        target: EndpointTargetRef::Device {
            provider_id: ProviderId::try_new("provider")?,
            device_identity: DeviceIdentity::try_new("device")?,
        },
        port: PortPath {
            element_id: ElementId::try_new(element)?,
            capability_id: CapabilityId::try_new("control")?,
            port_id: PortId::try_new(port)?,
        },
    })
}

fn runtime(stable: StableEndpointRef) -> Result<RuntimeEndpointRef, Box<dyn Error>> {
    Ok(RuntimeEndpointRef {
        stable,
        runtime_group_id: RuntimeGroupId::try_new("control")?,
        plugin_session_id: PluginSessionId::try_new("session")?,
        session_generation: SessionGeneration::try_new(1)?,
        descriptor_revision: DescriptorRevision::try_new(1)?,
    })
}

fn graph_port(
    id: &str,
    role: GraphPortRole,
    schema: SchemaRef,
) -> Result<GraphPort, Box<dyn Error>> {
    Ok(GraphPort {
        id: GraphPortId::try_new(id)?,
        kind: GraphPortKind::Value,
        role,
        schema,
        unit: None,
        range: None,
    })
}

fn port_ref(node: &str, port: &str) -> Result<GraphPortRef, Box<dyn Error>> {
    Ok(GraphPortRef {
        node_id: GraphNodeId::try_new(node)?,
        port_id: GraphPortId::try_new(port)?,
    })
}

fn contract(
    node_id: GraphNodeId,
    endpoint: StableEndpointRef,
    direction: PortDirection,
    semantics: PortSemantics,
    schema: SchemaRef,
) -> Result<CompiledEndpointContract, Box<dyn Error>> {
    Ok(CompiledEndpointContract {
        node_id,
        endpoint: endpoint.clone(),
        runtime_group_id: RuntimeGroupId::try_new("control")?,
        contribution_id: ContributionId::try_new("virtual")?,
        port: PortDescriptor {
            port_id: endpoint.port.port_id,
            kind: PortKind::try_new(if direction == PortDirection::Output {
                PortKind::EVENT_OUT
            } else {
                PortKind::VALUE_IN
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
            extensions: Default::default(),
        },
        operation: None,
    })
}

fn test_error() -> deckweave_domain::automation::DeckWeaveError {
    deckweave_domain::automation::DeckWeaveError {
        category: deckweave_domain::automation::ErrorCategory::Internal,
        code: "test".into(),
        message_key: "test".into(),
        details: Default::default(),
        diagnostic_attachment_id: None,
    }
}
