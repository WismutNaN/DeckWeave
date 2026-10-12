// SPDX-License-Identifier: GPL-3.0-or-later

use deckweave_automation::{
    ConditionOperator, EndpointAvailabilityPolicy, GraphDefinition, GraphDocument, GraphEdge,
    GraphEdgeId, GraphEdgeKind, GraphFormatVersion, GraphLayout, GraphLimitKind, GraphLimits,
    GraphNode, GraphNodeKind, GraphNodePlacement, GraphPort, GraphPortId, GraphPortKind,
    GraphPortRef, GraphPortRole, GraphValidationError,
};
use deckweave_domain::{
    endpoint::{EndpointTargetRef, PortPath, StableEndpointRef},
    ids::{
        CapabilityId, DeviceIdentity, ElementId, GraphNodeId, NamespacedKind, NodeId, OperationId,
        PackageId, PortId, ProviderId,
    },
    schema::{SchemaRef, SchemaVersion},
};
use std::{collections::BTreeMap, error::Error};

fn schema(id: &str) -> Result<SchemaRef, Box<dyn Error>> {
    Ok(SchemaRef {
        id: NamespacedKind::try_new(id)?,
        version: SchemaVersion { major: 1, minor: 0 },
    })
}

fn endpoint(element: &str, port: &str) -> Result<StableEndpointRef, Box<dyn Error>> {
    Ok(StableEndpointRef {
        package_id: PackageId::try_new("org.deckweave.virtual")?,
        node_id: NodeId::try_new("virtual-node")?,
        target: EndpointTargetRef::Device {
            provider_id: ProviderId::try_new("virtual-provider")?,
            device_identity: DeviceIdentity::try_new("virtual-device")?,
        },
        port: PortPath {
            element_id: ElementId::try_new(element)?,
            capability_id: CapabilityId::try_new("automation")?,
            port_id: PortId::try_new(port)?,
        },
    })
}

fn port(
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

fn port_ref(node: &str, port: &str) -> Result<GraphPortRef, Box<dyn Error>> {
    Ok(GraphPortRef {
        node_id: GraphNodeId::try_new(node)?,
        port_id: GraphPortId::try_new(port)?,
    })
}

fn edge(
    id: &str,
    kind: GraphEdgeKind,
    source_node: &str,
    source_port: &str,
    target_node: &str,
    target_port: &str,
) -> Result<GraphEdge, Box<dyn Error>> {
    Ok(GraphEdge {
        id: GraphEdgeId::try_new(id)?,
        kind,
        source: port_ref(source_node, source_port)?,
        target: port_ref(target_node, target_port)?,
        extensions: BTreeMap::new(),
    })
}

fn document() -> Result<GraphDocument, Box<dyn Error>> {
    let event = schema("org.deckweave.schema.event")?;
    let text = schema("org.deckweave.schema.text")?;
    let nodes = vec![
        GraphNode {
            id: GraphNodeId::try_new("source")?,
            kind: GraphNodeKind::Source {
                endpoint: endpoint("button", "pressed")?,
                availability: EndpointAvailabilityPolicy::Deferred,
            },
            inputs: vec![],
            outputs: vec![port(
                "event",
                GraphPortKind::Flow,
                GraphPortRole::Trigger,
                event.clone(),
            )?],
            extensions: BTreeMap::new(),
        },
        GraphNode {
            id: GraphNodeId::try_new("condition")?,
            kind: GraphNodeKind::Condition {
                operator: ConditionOperator::IsTrue,
                operand: None,
            },
            inputs: vec![port(
                "in",
                GraphPortKind::Flow,
                GraphPortRole::Input,
                event.clone(),
            )?],
            outputs: vec![port(
                "true",
                GraphPortKind::Flow,
                GraphPortRole::True,
                event.clone(),
            )?],
            extensions: BTreeMap::new(),
        },
        GraphNode {
            id: GraphNodeId::try_new("action")?,
            kind: GraphNodeKind::Action {
                endpoint: endpoint("service", "invoke")?,
                operation_id: OperationId::try_new("echo")?,
                availability: EndpointAvailabilityPolicy::Deferred,
            },
            inputs: vec![port(
                "invoke",
                GraphPortKind::Flow,
                GraphPortRole::Trigger,
                event,
            )?],
            outputs: vec![port(
                "result",
                GraphPortKind::Value,
                GraphPortRole::Result,
                text.clone(),
            )?],
            extensions: BTreeMap::new(),
        },
        GraphNode {
            id: GraphNodeId::try_new("state")?,
            kind: GraphNodeKind::State {
                state_key: NamespacedKind::try_new("org.deckweave.state.last_result")?,
                initial_value: None,
            },
            inputs: vec![port(
                "write",
                GraphPortKind::Value,
                GraphPortRole::Input,
                text.clone(),
            )?],
            outputs: vec![port(
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
                endpoint: endpoint("display", "text")?,
                availability: EndpointAvailabilityPolicy::Deferred,
            },
            inputs: vec![port(
                "value",
                GraphPortKind::Value,
                GraphPortRole::Input,
                text,
            )?],
            outputs: vec![],
            extensions: BTreeMap::new(),
        },
    ];
    let edges = vec![
        edge(
            "source-condition",
            GraphEdgeKind::Flow,
            "source",
            "event",
            "condition",
            "in",
        )?,
        edge(
            "condition-action",
            GraphEdgeKind::Flow,
            "condition",
            "true",
            "action",
            "invoke",
        )?,
        edge(
            "action-state",
            GraphEdgeKind::Data,
            "action",
            "result",
            "state",
            "write",
        )?,
        edge(
            "state-feedback",
            GraphEdgeKind::Data,
            "state",
            "value",
            "feedback",
            "value",
        )?,
    ];
    let mut placements = BTreeMap::new();
    placements.insert(
        GraphNodeId::try_new("source")?,
        GraphNodePlacement {
            x: 16.0,
            y: 24.0,
            collapsed: false,
        },
    );
    Ok(GraphDocument {
        plugin_instances: Default::default(),
        schema_version: GraphFormatVersion { major: 1, minor: 0 },
        graph: GraphDefinition {
            node_instances: Default::default(),
            action_arguments: Default::default(),
            nodes,
            edges,
            extensions: BTreeMap::from([(
                "org.deckweave.test.semantic".into(),
                serde_json::json!({"enabled": true}),
            )]),
        },
        layout: GraphLayout {
            nodes: placements,
            extensions: BTreeMap::from([(
                "org.deckweave.test.viewport".into(),
                serde_json::json!({"zoom": 1.25}),
            )]),
        },
        extensions: BTreeMap::new(),
    })
}

#[test]
fn versioned_typed_graph_round_trips_and_validates() -> Result<(), Box<dyn Error>> {
    let document = document()?;
    document.validate(&GraphLimits::default())?;
    let encoded = serde_json::to_vec(&document)?;
    let decoded: GraphDocument = serde_json::from_slice(&encoded)?;
    assert_eq!(decoded, document);
    assert_eq!(decoded.graph.nodes.len(), 5);
    assert_eq!(decoded.layout.nodes.len(), 1);
    Ok(())
}

#[test]
fn unknown_executable_node_is_rejected_instead_of_preserved_as_an_extension()
-> Result<(), Box<dyn Error>> {
    let mut value = serde_json::to_value(document()?)?;
    value["graph"]["nodes"][0]["kind"]["type"] = serde_json::json!("future_magic");
    assert!(serde_json::from_value::<GraphDocument>(value).is_err());
    Ok(())
}

#[test]
fn graph_validation_reports_schema_identity_and_limit_failures() -> Result<(), Box<dyn Error>> {
    let mut mismatch = document()?;
    mismatch.graph.nodes[3].inputs[0].schema = schema("org.deckweave.schema.number")?;
    assert!(matches!(
        mismatch.validate(&GraphLimits::default()),
        Err(GraphValidationError::SchemaMismatch { .. })
    ));

    let mut duplicate = document()?;
    duplicate.graph.nodes[1].id = duplicate.graph.nodes[0].id.clone();
    assert!(matches!(
        duplicate.validate(&GraphLimits::default()),
        Err(GraphValidationError::DuplicateNode { .. })
    ));

    let limited = GraphLimits {
        maximum_nodes: 4,
        ..GraphLimits::default()
    };
    assert_eq!(
        document()?.validate(&limited),
        Err(GraphValidationError::LimitExceeded {
            kind: GraphLimitKind::Nodes,
            maximum: 4,
            actual: 5,
        })
    );
    Ok(())
}

#[test]
fn layout_may_not_reference_a_nonexistent_semantic_node() -> Result<(), Box<dyn Error>> {
    let mut document = document()?;
    document.layout.nodes.insert(
        GraphNodeId::try_new("missing")?,
        GraphNodePlacement {
            x: 0.0,
            y: 0.0,
            collapsed: false,
        },
    );
    assert!(matches!(
        document.validate(&GraphLimits::default()),
        Err(GraphValidationError::UnknownLayoutNode { .. })
    ));
    Ok(())
}
