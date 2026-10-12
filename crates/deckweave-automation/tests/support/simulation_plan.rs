// SPDX-License-Identifier: GPL-3.0-or-later

use deckweave_automation::{
    CatalogEndpoint, CatalogSchema, CompileFence, DescriptorCatalog, EndpointAvailabilityPolicy,
    GraphDefinition, GraphDocument, GraphEdge, GraphEdgeId, GraphEdgeKind, GraphFormatVersion,
    GraphLayout, GraphLimits, GraphNode, GraphNodeKind, GraphPlanCompiler, GraphPort, GraphPortId,
    GraphPortKind, GraphPortRef, GraphPortRole, RuntimeRequirement, SchemaValueKind,
};
use deckweave_domain::{
    automation::{
        DataPolicy, ExecutionKind, ExecutionPolicy, OperationDescriptor, OperationLimits,
    },
    capability::{
        AvailabilityDescriptor, PortAvailability, PortDescriptor, PortDirection, PortKind,
        PortLifetime, PortSemantics,
    },
    endpoint::{EndpointTargetRef, PortPath, StableEndpointRef},
    ids::{
        CapabilityId, ContributionId, DescriptorRevision, DeviceIdentity, ElementId, GrantRevision,
        GraphNodeId, NamespacedKind, NodeId, OperationId, PackageId, PortId, ProviderId,
        RegistryRevision, RuntimeGroupId,
    },
    schema::{SchemaRef, SchemaVersion, SchemaVersionRange},
};
use deckweave_workspace::PackageLockEntry;
use std::{collections::BTreeMap, error::Error};

pub fn source_endpoint() -> Result<StableEndpointRef, Box<dyn Error>> {
    endpoint("button", "pressed")
}

pub fn compiled_plan(
    limits: GraphLimits,
) -> Result<deckweave_automation::CompiledExecutionPlan, Box<dyn Error>> {
    let compiler = GraphPlanCompiler::new(catalog()?, limits)?;
    Ok(compiler.compile_document(
        &serde_json::to_value(graph()?)?,
        &[PackageLockEntry {
            package_id: PackageId::try_new("org.deckweave.virtual")?,
            version: "1.0.0".into(),
            artifact_sha256: "a".repeat(64),
            source: None,
            extensions: BTreeMap::new(),
        }],
        "simulation-lock",
        &CompileFence {
            registry_revision: RegistryRevision::try_new(1)?,
            grant_revision: GrantRevision::try_new(1)?,
        },
    )?)
}

fn graph() -> Result<GraphDocument, Box<dyn Error>> {
    let event = schema("org.deckweave.schema.event")?;
    let text = schema("org.deckweave.schema.text")?;
    Ok(GraphDocument {
        schema_version: GraphFormatVersion { major: 1, minor: 0 },
        graph: GraphDefinition {
            nodes: vec![
                node(
                    "source",
                    GraphNodeKind::Source {
                        endpoint: source_endpoint()?,
                        availability: EndpointAvailabilityPolicy::Deferred,
                    },
                    vec![],
                    vec![port(
                        "event",
                        GraphPortKind::Flow,
                        GraphPortRole::Trigger,
                        event.clone(),
                    )?],
                )?,
                node(
                    "action",
                    GraphNodeKind::Action {
                        endpoint: endpoint("service", "invoke")?,
                        operation_id: OperationId::try_new("echo")?,
                        availability: EndpointAvailabilityPolicy::Deferred,
                    },
                    vec![port(
                        "trigger",
                        GraphPortKind::Flow,
                        GraphPortRole::Trigger,
                        event,
                    )?],
                    vec![port(
                        "result",
                        GraphPortKind::Value,
                        GraphPortRole::Result,
                        text.clone(),
                    )?],
                )?,
                node(
                    "state",
                    GraphNodeKind::State {
                        state_key: NamespacedKind::try_new("org.deckweave.last_result")?,
                        initial_value: Some(serde_json::json!("initial")),
                    },
                    vec![port(
                        "input",
                        GraphPortKind::Value,
                        GraphPortRole::Input,
                        text.clone(),
                    )?],
                    vec![port(
                        "value",
                        GraphPortKind::Value,
                        GraphPortRole::Value,
                        text.clone(),
                    )?],
                )?,
                node(
                    "feedback",
                    GraphNodeKind::Feedback {
                        endpoint: endpoint("display", "text")?,
                        availability: EndpointAvailabilityPolicy::Deferred,
                    },
                    vec![port(
                        "input",
                        GraphPortKind::Value,
                        GraphPortRole::Input,
                        text,
                    )?],
                    vec![],
                )?,
            ],
            edges: vec![
                edge(
                    "trigger",
                    GraphEdgeKind::Flow,
                    ("source", "event"),
                    ("action", "trigger"),
                )?,
                edge(
                    "result",
                    GraphEdgeKind::Data,
                    ("action", "result"),
                    ("state", "input"),
                )?,
                edge(
                    "feedback",
                    GraphEdgeKind::Data,
                    ("state", "value"),
                    ("feedback", "input"),
                )?,
            ],
            extensions: BTreeMap::new(),
        },
        layout: GraphLayout::default(),
        extensions: BTreeMap::new(),
    })
}

fn catalog() -> Result<DescriptorCatalog, Box<dyn Error>> {
    let event = schema("org.deckweave.schema.event")?;
    let unit = schema("org.deckweave.schema.unit")?;
    let text = schema("org.deckweave.schema.text")?;
    let error = schema("org.deckweave.schema.error")?;
    let source = source_endpoint()?;
    let action = endpoint("service", "invoke")?;
    let feedback = endpoint("display", "text")?;
    Ok(DescriptorCatalog::try_new(
        vec![
            catalog_schema(event.clone(), SchemaValueKind::Json),
            catalog_schema(unit.clone(), SchemaValueKind::Unit),
            catalog_schema(text.clone(), SchemaValueKind::String),
            catalog_schema(error.clone(), SchemaValueKind::Json),
        ],
        vec![
            catalog_endpoint(
                source,
                PortDirection::Output,
                PortSemantics::Event,
                PortKind::EVENT_OUT,
                event,
                None,
            )?,
            catalog_endpoint(
                action,
                PortDirection::Input,
                PortSemantics::Command,
                PortKind::COMMAND_IN,
                unit,
                Some(operation(text, error)?),
            )?,
            catalog_endpoint(
                feedback,
                PortDirection::Input,
                PortSemantics::Value,
                PortKind::VALUE_IN,
                schema("org.deckweave.schema.text")?,
                None,
            )?,
        ],
    )?)
}

fn catalog_schema(schema: SchemaRef, value_kind: SchemaValueKind) -> CatalogSchema {
    CatalogSchema {
        schema,
        value_kind,
        unit: None,
        range: None,
    }
}

fn catalog_endpoint(
    endpoint: StableEndpointRef,
    direction: PortDirection,
    semantics: PortSemantics,
    kind: &str,
    schema: SchemaRef,
    operation: Option<OperationDescriptor>,
) -> Result<CatalogEndpoint, Box<dyn Error>> {
    let port_id = endpoint.port.port_id.clone();
    Ok(CatalogEndpoint {
        endpoint,
        runtime_requirement: RuntimeRequirement {
            package_id: PackageId::try_new("org.deckweave.virtual")?,
            package_version: "1.0.0".into(),
            artifact_sha256: "a".repeat(64),
            runtime_group_id: RuntimeGroupId::try_new("control")?,
            contribution_id: ContributionId::try_new("virtual")?,
        },
        port: PortDescriptor {
            port_id,
            kind: PortKind::try_new(kind)?,
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
        },
        operation,
    })
}

fn operation(
    result_schema: SchemaRef,
    error_schema: SchemaRef,
) -> Result<OperationDescriptor, Box<dyn Error>> {
    Ok(OperationDescriptor {
        operation_id: OperationId::try_new("echo")?,
        version: 1,
        input_schema: schema("org.deckweave.schema.unit")?,
        result_schema,
        error_schema,
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
            maximum_input_bytes: 32,
            maximum_output_bytes: 4_096,
            maximum_concurrent_invocations: 8,
            maximum_queue_depth: 32,
            maximum_per_minute: 10_000,
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

fn endpoint(element: &str, port: &str) -> Result<StableEndpointRef, Box<dyn Error>> {
    Ok(StableEndpointRef {
        package_id: PackageId::try_new("org.deckweave.virtual")?,
        node_id: NodeId::try_new("node")?,
        target: EndpointTargetRef::Device {
            provider_id: ProviderId::try_new("provider")?,
            device_identity: DeviceIdentity::try_new("device-b")?,
        },
        port: PortPath {
            element_id: ElementId::try_new(element)?,
            capability_id: CapabilityId::try_new("automation")?,
            port_id: PortId::try_new(port)?,
        },
    })
}

fn schema(id: &str) -> Result<SchemaRef, Box<dyn Error>> {
    Ok(SchemaRef {
        id: NamespacedKind::try_new(id)?,
        version: SchemaVersion { major: 1, minor: 0 },
    })
}

fn node(
    id: &str,
    kind: GraphNodeKind,
    inputs: Vec<GraphPort>,
    outputs: Vec<GraphPort>,
) -> Result<GraphNode, Box<dyn Error>> {
    Ok(GraphNode {
        id: GraphNodeId::try_new(id)?,
        kind,
        inputs,
        outputs,
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
        source: port_ref(source)?,
        target: port_ref(target)?,
        extensions: BTreeMap::new(),
    })
}

fn port_ref(value: (&str, &str)) -> Result<GraphPortRef, Box<dyn Error>> {
    Ok(GraphPortRef {
        node_id: GraphNodeId::try_new(value.0)?,
        port_id: GraphPortId::try_new(value.1)?,
    })
}
