// SPDX-License-Identifier: GPL-3.0-or-later
//! Hardware-independent source → action → state → feedback production fixture.

use super::stage_named_package;
use deckweave_automation::{
    CatalogEndpoint, CatalogSchema, ConditionOperator, DescriptorCatalog,
    EndpointAvailabilityPolicy, GraphDefinition, GraphDocument, GraphEdge, GraphEdgeId,
    GraphEdgeKind, GraphFormatVersion, GraphLayout, GraphLimits, GraphNode, GraphNodeKind,
    GraphPlanCompiler, GraphPort, GraphPortId, GraphPortKind, GraphPortRef, GraphPortRole,
    RuntimeRequirement, SchemaValueKind,
};
use deckweave_domain::{
    automation::{
        DataPolicy, ExecutionKind, ExecutionPolicy, OperationDescriptor, OperationLimits,
    },
    capability::{NodeDescriptor, PortDescriptor},
    endpoint::{EndpointTargetRef, PortPath, StableEndpointRef},
    ids::{
        CapabilityId, ContributionId, DeviceIdentity, DraftId, DraftVersion, ElementId,
        GraphNodeId, NamespacedKind, NodeId, OperationId, PackageId, PortId, ProviderId,
        RuntimeGroupId, ServiceId, WorkspaceId,
    },
    schema::{SchemaRef, SchemaVersion},
};
use deckweave_workspace::{PackageLockEntry, WorkspaceDraft};
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    error::Error,
    path::{Path, PathBuf},
};

pub(super) const SOURCE_PACKAGE: &str = "org.deckweave.fixture.m1-source";
pub(super) const ACTION_PACKAGE: &str = "org.deckweave.fixture.m1-action";
pub(super) const FEEDBACK_PACKAGE: &str = "org.deckweave.fixture.m1-feedback";

pub(super) fn stage_and_build(
    root: &Path,
) -> Result<(WorkspaceDraft, GraphPlanCompiler, PathBuf), Box<dyn Error>> {
    let packages = [
        ("m1-source", SOURCE_PACKAGE, "source-group", "m1-source"),
        ("m1-action", ACTION_PACKAGE, "action-group", "m1-action"),
        (
            "m1-feedback",
            FEEDBACK_PACKAGE,
            "feedback-group",
            "m1-feedback",
        ),
    ];
    let mut hashes = BTreeMap::new();
    for (directory, package, group, contribution) in packages {
        hashes.insert(
            package,
            stage_named_package(root, directory, package, group, contribution)?,
        );
    }
    let source_descriptor: NodeDescriptor = serde_json::from_value(source_descriptor())?;
    let action_descriptor: NodeDescriptor = serde_json::from_value(service_descriptor(
        "m1-action-node",
        "action-service",
        "invoke",
        "action",
        "invoke",
        "CommandIn",
        "command",
        "org.deckweave.schema.unit",
    ))?;
    let feedback_descriptor: NodeDescriptor = serde_json::from_value(service_descriptor(
        "m1-feedback-node",
        "feedback-service",
        "display",
        "text",
        "value",
        "ValueIn",
        "value",
        "org.deckweave.schema.text",
    ))?;
    let boolean = schema("org.deckweave.schema.boolean")?;
    let number = schema("org.deckweave.schema.number")?;
    let text = schema("org.deckweave.schema.text")?;
    let unit = schema("org.deckweave.schema.unit")?;
    let event = schema("org.deckweave.schema.event")?;
    let error = schema("org.deckweave.schema.error")?;
    let endpoints = Endpoints::new()?;
    let catalog = DescriptorCatalog::try_new(
        vec![
            (boolean.clone(), SchemaValueKind::Boolean),
            (number.clone(), SchemaValueKind::Number),
            (text.clone(), SchemaValueKind::String),
            (unit.clone(), SchemaValueKind::Unit),
            (event.clone(), SchemaValueKind::Json),
            (error.clone(), SchemaValueKind::Json),
        ]
        .into_iter()
        .map(|(schema, value_kind)| CatalogSchema {
            schema,
            value_kind,
            unit: None,
            range: None,
        })
        .collect(),
        vec![
            catalog_endpoint(
                &endpoints.button,
                requirement(
                    SOURCE_PACKAGE,
                    "source-group",
                    "m1-source",
                    hashes[SOURCE_PACKAGE].clone(),
                )?,
                device_port(&source_descriptor, 1, "button", "pressed")?,
                None,
            ),
            catalog_endpoint(
                &endpoints.axis,
                requirement(
                    SOURCE_PACKAGE,
                    "source-group",
                    "m1-source",
                    hashes[SOURCE_PACKAGE].clone(),
                )?,
                device_port(&source_descriptor, 1, "axis", "value")?,
                None,
            ),
            catalog_endpoint(
                &endpoints.state_source,
                requirement(
                    SOURCE_PACKAGE,
                    "source-group",
                    "m1-source",
                    hashes[SOURCE_PACKAGE].clone(),
                )?,
                device_port(&source_descriptor, 1, "state", "value")?,
                None,
            ),
            catalog_endpoint(
                &endpoints.action,
                requirement(
                    ACTION_PACKAGE,
                    "action-group",
                    "m1-action",
                    hashes[ACTION_PACKAGE].clone(),
                )?,
                service_port(&action_descriptor)?,
                Some(operation(&unit, &text, &error)?),
            ),
            catalog_endpoint(
                &endpoints.feedback,
                requirement(
                    FEEDBACK_PACKAGE,
                    "feedback-group",
                    "m1-feedback",
                    hashes[FEEDBACK_PACKAGE].clone(),
                )?,
                service_port(&feedback_descriptor)?,
                None,
            ),
        ],
    )?;
    let graph = graph(&endpoints, &boolean, &number, &text, &event, &error)?;
    let package_lock = [SOURCE_PACKAGE, ACTION_PACKAGE, FEEDBACK_PACKAGE]
        .into_iter()
        .map(|package| {
            Ok(PackageLockEntry {
                package_id: PackageId::try_new(package)?,
                version: "0.1.0".into(),
                artifact_sha256: hashes[package].clone(),
                source: None,
                extensions: BTreeMap::new(),
            })
        })
        .collect::<Result<Vec<_>, deckweave_domain::ids::IdError>>()?;
    Ok((
        WorkspaceDraft {
            schema_version: 1,
            draft_id: DraftId::try_new("m1-draft")?,
            workspace_id: WorkspaceId::try_new("m1-workspace")?,
            base_revision_id: None,
            version: DraftVersion::try_new(1)?,
            package_lock,
            document: serde_json::to_value(graph)?,
            assets: vec![],
            private_section: None,
            extensions: BTreeMap::new(),
            unknown_fields: BTreeMap::new(),
        },
        GraphPlanCompiler::new(catalog, GraphLimits::default())?,
        root.join("packages/m1-feedback/0.1.0/content/feedback.received.json"),
    ))
}

struct Endpoints {
    button: StableEndpointRef,
    axis: StableEndpointRef,
    state_source: StableEndpointRef,
    action: StableEndpointRef,
    feedback: StableEndpointRef,
}

impl Endpoints {
    fn new() -> Result<Self, Box<dyn Error>> {
        Ok(Self {
            button: device_endpoint("button", "button", "pressed")?,
            axis: device_endpoint("axis", "axis", "value")?,
            state_source: device_endpoint("state", "state", "value")?,
            action: service_endpoint(
                ACTION_PACKAGE,
                "m1-action-node",
                "action-service",
                "invoke",
                "action",
                "invoke",
            )?,
            feedback: service_endpoint(
                FEEDBACK_PACKAGE,
                "m1-feedback-node",
                "feedback-service",
                "display",
                "text",
                "value",
            )?,
        })
    }
}

fn graph(
    endpoints: &Endpoints,
    boolean: &SchemaRef,
    number: &SchemaRef,
    text: &SchemaRef,
    event: &SchemaRef,
    error: &SchemaRef,
) -> Result<GraphDocument, Box<dyn Error>> {
    let nodes = vec![
        source_node("button-source", &endpoints.button, "value", boolean.clone())?,
        source_node("axis-source", &endpoints.axis, "value", number.clone())?,
        source_node(
            "state-source",
            &endpoints.state_source,
            "value",
            text.clone(),
        )?,
        GraphNode {
            id: GraphNodeId::try_new("condition")?,
            kind: GraphNodeKind::Condition {
                operator: ConditionOperator::IsTrue,
                operand: None,
            },
            inputs: vec![port(
                "value",
                GraphPortKind::Value,
                GraphPortRole::Input,
                boolean.clone(),
            )?],
            outputs: vec![
                port(
                    "true",
                    GraphPortKind::Flow,
                    GraphPortRole::True,
                    event.clone(),
                )?,
                port(
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
                endpoint: endpoints.action.clone(),
                operation_id: OperationId::try_new("m1-echo")?,
                availability: EndpointAvailabilityPolicy::Required,
            },
            inputs: vec![port(
                "trigger",
                GraphPortKind::Flow,
                GraphPortRole::Trigger,
                event.clone(),
            )?],
            outputs: vec![
                port(
                    "result",
                    GraphPortKind::Value,
                    GraphPortRole::Result,
                    text.clone(),
                )?,
                port(
                    "error",
                    GraphPortKind::Value,
                    GraphPortRole::Error,
                    error.clone(),
                )?,
            ],
            extensions: BTreeMap::new(),
        },
        GraphNode {
            id: GraphNodeId::try_new("result-state")?,
            kind: GraphNodeKind::State {
                state_key: NamespacedKind::try_new("org.deckweave.m1.last_result")?,
                initial_value: Some(Value::String("idle".into())),
            },
            inputs: vec![port(
                "input",
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
                endpoint: endpoints.feedback.clone(),
                availability: EndpointAvailabilityPolicy::Required,
            },
            inputs: vec![port(
                "input",
                GraphPortKind::Value,
                GraphPortRole::Input,
                text.clone(),
            )?],
            outputs: vec![],
            extensions: BTreeMap::new(),
        },
    ];
    Ok(GraphDocument {
        schema_version: GraphFormatVersion { major: 1, minor: 0 },
        graph: GraphDefinition {
            nodes,
            edges: vec![
                edge(
                    "button-condition",
                    GraphEdgeKind::Data,
                    ("button-source", "value"),
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
                    ("result-state", "input"),
                )?,
                edge(
                    "state-feedback",
                    GraphEdgeKind::Data,
                    ("result-state", "value"),
                    ("feedback", "input"),
                )?,
            ],
            extensions: BTreeMap::new(),
        },
        layout: GraphLayout::default(),
        extensions: BTreeMap::new(),
    })
}

fn source_node(
    id: &str,
    endpoint: &StableEndpointRef,
    output: &str,
    schema: SchemaRef,
) -> Result<GraphNode, Box<dyn Error>> {
    Ok(GraphNode {
        id: GraphNodeId::try_new(id)?,
        kind: GraphNodeKind::Source {
            endpoint: endpoint.clone(),
            availability: EndpointAvailabilityPolicy::Required,
        },
        inputs: vec![],
        outputs: vec![port(
            output,
            GraphPortKind::Value,
            GraphPortRole::Value,
            schema,
        )?],
        extensions: BTreeMap::new(),
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

fn edge(
    id: &str,
    kind: GraphEdgeKind,
    source: (&str, &str),
    target: (&str, &str),
) -> Result<GraphEdge, Box<dyn Error>> {
    Ok(GraphEdge {
        id: GraphEdgeId::try_new(id)?,
        kind,
        source: port_ref(source.0, source.1)?,
        target: port_ref(target.0, target.1)?,
        extensions: BTreeMap::new(),
    })
}

fn port_ref(node: &str, port: &str) -> Result<GraphPortRef, Box<dyn Error>> {
    Ok(GraphPortRef {
        node_id: GraphNodeId::try_new(node)?,
        port_id: GraphPortId::try_new(port)?,
    })
}

fn catalog_endpoint(
    endpoint: &StableEndpointRef,
    runtime_requirement: RuntimeRequirement,
    port: PortDescriptor,
    operation: Option<OperationDescriptor>,
) -> CatalogEndpoint {
    CatalogEndpoint {
        endpoint: endpoint.clone(),
        runtime_requirement,
        port,
        operation,
    }
}

fn requirement(
    package: &str,
    group: &str,
    contribution: &str,
    hash: String,
) -> Result<RuntimeRequirement, Box<dyn Error>> {
    Ok(RuntimeRequirement {
        package_id: PackageId::try_new(package)?,
        package_version: "0.1.0".into(),
        artifact_sha256: hash,
        runtime_group_id: RuntimeGroupId::try_new(group)?,
        contribution_id: ContributionId::try_new(contribution)?,
    })
}

fn operation(
    unit: &SchemaRef,
    text: &SchemaRef,
    error: &SchemaRef,
) -> Result<OperationDescriptor, Box<dyn Error>> {
    Ok(OperationDescriptor {
        operation_id: OperationId::try_new("m1-echo")?,
        version: 1,
        input_schema: unit.clone(),
        result_schema: text.clone(),
        error_schema: error.clone(),
        execution: ExecutionPolicy {
            kind: ExecutionKind::Job,
            progress: false,
            cancellation: true,
            reconciliation: false,
        },
        effects: vec![],
        idempotency: None,
        limits: OperationLimits {
            deadline_ms: 2_000,
            maximum_input_bytes: 16,
            maximum_output_bytes: 128,
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

fn schema(id: &str) -> Result<SchemaRef, Box<dyn Error>> {
    Ok(SchemaRef {
        id: NamespacedKind::try_new(id)?,
        version: SchemaVersion { major: 1, minor: 0 },
    })
}

fn device_endpoint(
    element: &str,
    capability: &str,
    port: &str,
) -> Result<StableEndpointRef, Box<dyn Error>> {
    Ok(StableEndpointRef {
        package_id: PackageId::try_new(SOURCE_PACKAGE)?,
        node_id: NodeId::try_new("m1-source-node")?,
        target: EndpointTargetRef::Device {
            provider_id: ProviderId::try_new("m1-provider")?,
            device_identity: DeviceIdentity::try_new("device-b")?,
        },
        port: PortPath {
            element_id: ElementId::try_new(element)?,
            capability_id: CapabilityId::try_new(capability)?,
            port_id: PortId::try_new(port)?,
        },
    })
}

fn service_endpoint(
    package: &str,
    node: &str,
    service: &str,
    element: &str,
    capability: &str,
    port: &str,
) -> Result<StableEndpointRef, Box<dyn Error>> {
    Ok(StableEndpointRef {
        package_id: PackageId::try_new(package)?,
        node_id: NodeId::try_new(node)?,
        target: EndpointTargetRef::Service {
            provider_id: ProviderId::try_new("m1-provider")?,
            service_id: ServiceId::try_new(service)?,
        },
        port: PortPath {
            element_id: ElementId::try_new(element)?,
            capability_id: CapabilityId::try_new(capability)?,
            port_id: PortId::try_new(port)?,
        },
    })
}

fn device_port(
    descriptor: &NodeDescriptor,
    device: usize,
    element: &str,
    port: &str,
) -> Result<PortDescriptor, Box<dyn Error>> {
    descriptor.devices[device]
        .elements
        .iter()
        .find(|candidate| candidate.element_id.as_str() == element)
        .and_then(|element| element.capabilities.first())
        .and_then(|capability| {
            capability
                .ports
                .iter()
                .find(|candidate| candidate.port_id.as_str() == port)
        })
        .cloned()
        .ok_or_else(|| "fixture device port missing".into())
}

fn service_port(descriptor: &NodeDescriptor) -> Result<PortDescriptor, Box<dyn Error>> {
    descriptor
        .services
        .first()
        .and_then(|service| service.elements.first())
        .and_then(|element| element.capabilities.first())
        .and_then(|capability| capability.ports.first())
        .cloned()
        .ok_or_else(|| "fixture service port missing".into())
}

fn source_descriptor() -> Value {
    let elements = json!([
        element(
            "button",
            "button",
            "pressed",
            "EventOut",
            "event",
            "org.deckweave.schema.boolean"
        ),
        element(
            "axis",
            "axis",
            "value",
            "ValueOut",
            "value",
            "org.deckweave.schema.number"
        ),
        element(
            "state",
            "state",
            "value",
            "ValueOut",
            "value",
            "org.deckweave.schema.text"
        )
    ]);
    json!({
        "nodeId": "m1-source-node", "kind": "virtual", "label": "M1 source", "descriptorRevision": 1,
        "devices": [
            { "identity": "device-a", "providerId": "m1-provider", "kind": "virtual", "label": "Twin A", "evidence": [], "elements": elements },
            { "identity": "device-b", "providerId": "m1-provider", "kind": "virtual", "label": "Twin B", "evidence": [], "elements": elements }
        ]
    })
}

#[allow(clippy::too_many_arguments)]
fn service_descriptor(
    node: &str,
    service: &str,
    element_id: &str,
    capability_id: &str,
    port_id: &str,
    kind: &str,
    semantics: &str,
    schema: &str,
) -> Value {
    json!({
        "nodeId": node, "kind": "virtual", "label": node, "descriptorRevision": 1,
        "services": [{ "serviceId": service, "providerId": "m1-provider", "label": service,
            "elements": [element(element_id, capability_id, port_id, kind, semantics, schema)] }]
    })
}

fn element(
    element_id: &str,
    capability_id: &str,
    port_id: &str,
    kind: &str,
    semantics: &str,
    schema: &str,
) -> Value {
    json!({
        "elementId": element_id, "label": element_id,
        "capabilities": [{ "capabilityId": capability_id, "kind": format!("org.deckweave.fixture.{capability_id}"),
            "ports": [{
                "portId": port_id, "kind": kind,
                "direction": if kind.ends_with("Out") { "output" } else { "input" },
                "semantics": semantics,
                "schema": { "id": schema, "version": { "major": 1, "minor": 0 } },
                "compatibleVersions": { "minimum": { "major": 1, "minor": 0 }, "maximum": { "major": 1, "minor": 0 } },
                "availability": { "status": "available" }, "lifetime": "always", "descriptorRevision": 1, "tags": ["m1"]
            }]
        }]
    })
}
