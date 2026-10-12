// SPDX-License-Identifier: GPL-3.0-or-later

use deckweave_automation::{
    CatalogEndpoint, CatalogSchema, CompileAccess, CompileDiagnosticCode, CompileFence,
    DescriptorCatalog, EndpointAvailabilityPolicy, GraphCompileError, GraphDefinition,
    GraphDocument, GraphEdge, GraphEdgeId, GraphEdgeKind, GraphFormatVersion, GraphLayout,
    GraphLimits, GraphNode, GraphNodeKind, GraphPlanCompiler, GraphPort, GraphPortId,
    GraphPortKind, GraphPortRef, GraphPortRole, RuntimeBindError, RuntimeBinder, RuntimeBinding,
    RuntimeRequirement, SchemaValueKind,
};
use deckweave_domain::{
    automation::{
        DataPolicy, ExecutionKind, ExecutionPolicy, OperationDescriptor, OperationLimits,
        ResourceLeaseMode, ResourceRequirement,
    },
    capability::{
        AvailabilityDescriptor, PortAvailability, PortDescriptor, PortDirection, PortKind,
        PortLifetime, PortSemantics,
    },
    endpoint::{EndpointTargetRef, PortPath, RuntimeEndpointRef, StableEndpointRef},
    ids::{
        CapabilityId, ContributionId, DescriptorRevision, DeviceIdentity, ElementId, GrantRevision,
        GraphNodeId, NamespacedKind, NodeId, OperationId, PackageId, PluginSessionId, PortId,
        ProviderId, RegistryRevision, RuntimeGroupId, SessionGeneration,
    },
    schema::{SchemaRef, SchemaVersion, SchemaVersionRange},
};
use deckweave_workspace::PackageLockEntry;
use std::{collections::BTreeMap, error::Error};
#[path = "support/action_results.rs"]
mod action_results;
#[path = "support/argument_compiler.rs"]
mod argument_compiler;

fn schema(id: &str) -> Result<SchemaRef, Box<dyn Error>> {
    Ok(SchemaRef {
        id: deckweave_domain::ids::NamespacedKind::try_new(id)?,
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

fn port_ref(node: &str, port: &str) -> Result<GraphPortRef, Box<dyn Error>> {
    Ok(GraphPortRef {
        node_id: GraphNodeId::try_new(node)?,
        port_id: GraphPortId::try_new(port)?,
    })
}

fn edge(
    id: &str,
    kind: GraphEdgeKind,
    from_node: &str,
    from_port: &str,
    to_node: &str,
    to_port: &str,
) -> Result<GraphEdge, Box<dyn Error>> {
    Ok(GraphEdge {
        id: GraphEdgeId::try_new(id)?,
        kind,
        source: port_ref(from_node, from_port)?,
        target: port_ref(to_node, to_port)?,
        extensions: BTreeMap::new(),
    })
}

fn graph() -> Result<GraphDocument, Box<dyn Error>> {
    let event = schema("org.deckweave.schema.event")?;
    let text = schema("org.deckweave.schema.text")?;
    Ok(GraphDocument {
        plugin_instances: Default::default(),
        schema_version: GraphFormatVersion { major: 1, minor: 0 },
        graph: GraphDefinition {
            node_instances: Default::default(),
            action_arguments: Default::default(),
            nodes: vec![
                GraphNode {
                    id: GraphNodeId::try_new("source")?,
                    kind: GraphNodeKind::Source {
                        endpoint: endpoint("button", "pressed")?,
                        availability: EndpointAvailabilityPolicy::Deferred,
                    },
                    inputs: vec![],
                    outputs: vec![graph_port(
                        "event",
                        GraphPortKind::Flow,
                        GraphPortRole::Trigger,
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
                    inputs: vec![graph_port(
                        "trigger",
                        GraphPortKind::Flow,
                        GraphPortRole::Trigger,
                        event,
                    )?],
                    outputs: vec![graph_port(
                        "result",
                        GraphPortKind::Value,
                        GraphPortRole::Result,
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
                    inputs: vec![graph_port(
                        "value",
                        GraphPortKind::Value,
                        GraphPortRole::Input,
                        text,
                    )?],
                    outputs: vec![],
                    extensions: BTreeMap::new(),
                },
            ],
            edges: vec![
                edge(
                    "trigger",
                    GraphEdgeKind::Flow,
                    "source",
                    "event",
                    "action",
                    "trigger",
                )?,
                edge(
                    "result",
                    GraphEdgeKind::Data,
                    "action",
                    "result",
                    "feedback",
                    "value",
                )?,
            ],
            extensions: BTreeMap::new(),
        },
        layout: GraphLayout::default(),
        extensions: BTreeMap::new(),
    })
}

fn descriptor(
    reference: &StableEndpointRef,
    direction: PortDirection,
    semantics: PortSemantics,
    kind: &str,
    schema: SchemaRef,
    availability: PortAvailability,
) -> Result<PortDescriptor, Box<dyn Error>> {
    Ok(PortDescriptor {
        port_id: reference.port.port_id.clone(),
        kind: PortKind::try_new(kind)?,
        direction,
        semantics,
        compatible_versions: SchemaVersionRange {
            minimum: schema.version,
            maximum: schema.version,
        },
        schema,
        unit: None,
        range: None,
        coordinate_space: None,
        rate: None,
        required_permissions: vec![],
        availability: AvailabilityDescriptor {
            status: availability,
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
            maximum_input_bytes: 1_024,
            maximum_output_bytes: 4_096,
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

fn requirement() -> Result<RuntimeRequirement, Box<dyn Error>> {
    Ok(RuntimeRequirement {
        configuration: None,
        package_id: PackageId::try_new("org.deckweave.virtual")?,
        package_version: "1.0.0".into(),
        artifact_sha256: "a".repeat(64),
        runtime_group_id: RuntimeGroupId::try_new("control")?,
        contribution_id: ContributionId::try_new("virtual")?,
    })
}

fn catalog(
    availability: PortAvailability,
    restricted: bool,
) -> Result<DescriptorCatalog, Box<dyn Error>> {
    let event = schema("org.deckweave.schema.event")?;
    let text = schema("org.deckweave.schema.text")?;
    let unit = schema("org.deckweave.schema.unit")?;
    let error = schema("org.deckweave.schema.error")?;
    let source = endpoint("button", "pressed")?;
    let action = endpoint("service", "invoke")?;
    let feedback = endpoint("display", "text")?;
    let mut action_port = descriptor(
        &action,
        PortDirection::Input,
        PortSemantics::Command,
        PortKind::COMMAND_IN,
        unit.clone(),
        availability,
    )?;
    let mut action_operation = operation()?;
    if restricted {
        action_port
            .required_permissions
            .push(NamespacedKind::try_new("org.deckweave.permission.invoke")?);
        action_operation.resources.push(ResourceRequirement {
            scope: NamespacedKind::try_new("org.deckweave.resource.echo")?,
            mode: ResourceLeaseMode::Exclusive,
            hold_until_cleanup: true,
        });
    }
    Ok(DescriptorCatalog::try_new(
        vec![
            CatalogSchema {
                schema: event.clone(),
                value_kind: SchemaValueKind::Json,
                unit: None,
                range: None,
            },
            CatalogSchema {
                schema: text.clone(),
                value_kind: SchemaValueKind::String,
                unit: None,
                range: None,
            },
            CatalogSchema {
                schema: unit.clone(),
                value_kind: SchemaValueKind::Unit,
                unit: None,
                range: None,
            },
            CatalogSchema {
                schema: error.clone(),
                value_kind: SchemaValueKind::Json,
                unit: None,
                range: None,
            },
        ],
        vec![
            CatalogEndpoint {
                endpoint: source.clone(),
                runtime_requirement: requirement()?,
                port: descriptor(
                    &source,
                    PortDirection::Output,
                    PortSemantics::Event,
                    PortKind::EVENT_OUT,
                    event,
                    availability,
                )?,
                operation: None,
            },
            CatalogEndpoint {
                endpoint: action.clone(),
                runtime_requirement: requirement()?,
                port: action_port,
                operation: Some(action_operation),
            },
            CatalogEndpoint {
                endpoint: feedback.clone(),
                runtime_requirement: requirement()?,
                port: descriptor(
                    &feedback,
                    PortDirection::Input,
                    PortSemantics::Value,
                    PortKind::VALUE_IN,
                    text,
                    availability,
                )?,
                operation: None,
            },
        ],
    )?)
}

fn package_lock() -> Result<Vec<PackageLockEntry>, Box<dyn Error>> {
    Ok(vec![PackageLockEntry {
        package_id: PackageId::try_new("org.deckweave.virtual")?,
        version: "1.0.0".into(),
        artifact_sha256: "a".repeat(64),
        source: None,
        extensions: BTreeMap::new(),
    }])
}

fn fence() -> Result<CompileFence, Box<dyn Error>> {
    Ok(CompileFence {
        registry_revision: RegistryRevision::try_new(1)?,
        grant_revision: GrantRevision::try_new(1)?,
    })
}

#[test]
fn static_catalog_compiles_a_canonical_dag_without_live_runtime() -> Result<(), Box<dyn Error>> {
    let compiler = GraphPlanCompiler::new(
        catalog(PortAvailability::Available, false)?,
        GraphLimits::default(),
    )?;
    let graph = graph()?;
    let first = compiler.compile_document(
        &serde_json::to_value(&graph)?,
        &package_lock()?,
        "lock",
        &fence()?,
    )?;
    assert_eq!(first.runtime_requirements().len(), 1);
    assert!(first.diagnostics().is_empty());
    assert_eq!(first.clone().seal()?.verify()?, first);
    let executable = first
        .executable_graph()
        .ok_or_else(|| std::io::Error::other("missing graph plan"))?;
    assert_eq!(executable.topological_order.len(), 3);

    let mut reordered = graph;
    reordered.graph.nodes.reverse();
    reordered.graph.edges.reverse();
    reordered.layout.nodes.insert(
        GraphNodeId::try_new("source")?,
        deckweave_automation::GraphNodePlacement {
            x: 99.0,
            y: -12.0,
            collapsed: true,
        },
    );
    let second = compiler.compile_document(
        &serde_json::to_value(reordered)?,
        &package_lock()?,
        "lock",
        &fence()?,
    )?;
    assert_eq!(first.semantic_sha256(), second.semantic_sha256());
    Ok(())
}

#[test]
fn cycle_and_missing_endpoint_are_structured_compile_errors() -> Result<(), Box<dyn Error>> {
    let compiler = GraphPlanCompiler::new(
        catalog(PortAvailability::Available, false)?,
        GraphLimits::default(),
    )?;
    let mut cyclic = graph()?;
    cyclic.graph.nodes[1].inputs.push(graph_port(
        "payload",
        GraphPortKind::Value,
        GraphPortRole::Input,
        schema("org.deckweave.schema.text")?,
    )?);
    cyclic.graph.edges.push(edge(
        "self-cycle",
        GraphEdgeKind::Data,
        "action",
        "result",
        "action",
        "payload",
    )?);
    assert!(matches!(
        compiler.compile_document(
            &serde_json::to_value(cyclic)?,
            &package_lock()?,
            "lock",
            &fence()?
        ),
        Err(GraphCompileError::Cycle { .. })
    ));

    let mut missing = graph()?;
    if let GraphNodeKind::Source { endpoint, .. } = &mut missing.graph.nodes[0].kind {
        endpoint.port.port_id = PortId::try_new("missing")?;
    }
    assert!(matches!(
        compiler.compile_document(
            &serde_json::to_value(missing)?,
            &package_lock()?,
            "lock",
            &fence()?
        ),
        Err(GraphCompileError::UnresolvedEndpoint { .. })
    ));
    Ok(())
}

#[test]
fn deferred_availability_is_visible_and_runtime_binding_revalidates_contracts()
-> Result<(), Box<dyn Error>> {
    let compiler = GraphPlanCompiler::new(
        catalog(PortAvailability::Disconnected, false)?,
        GraphLimits::default(),
    )?;
    let plan = compiler.compile_document(
        &serde_json::to_value(graph()?)?,
        &package_lock()?,
        "lock",
        &fence()?,
    )?;
    assert!(
        plan.diagnostics()
            .iter()
            .all(|diagnostic| diagnostic.code == CompileDiagnosticCode::NeedsRuntimeAvailability)
    );

    let executable = plan
        .executable_graph()
        .ok_or_else(|| std::io::Error::other("missing graph plan"))?;
    let mut bindings = Vec::new();
    for contract in &executable.endpoint_contracts {
        let mut port = contract.port.clone();
        port.availability.status = PortAvailability::Available;
        bindings.push(RuntimeBinding {
            endpoint: RuntimeEndpointRef {
                stable: contract.endpoint.clone(),
                runtime_group_id: RuntimeGroupId::try_new("control")?,
                plugin_session_id: PluginSessionId::try_new("session")?,
                session_generation: SessionGeneration::try_new(1)?,
                descriptor_revision: DescriptorRevision::try_new(1)?,
            },
            contribution_id: ContributionId::try_new("virtual")?,
            port,
            operation: contract.operation.clone(),
        });
    }
    let bound = RuntimeBinder.bind_plan(&plan, &bindings)?;
    assert_eq!(bound.endpoints.len(), 3);

    bindings[0].contribution_id = ContributionId::try_new("wrong")?;
    assert!(matches!(
        RuntimeBinder.bind_plan(&plan, &bindings),
        Err(RuntimeBindError::IncompatibleDescriptor { .. })
    ));
    bindings[0].contribution_id = ContributionId::try_new("virtual")?;
    bindings[0].endpoint.runtime_group_id = RuntimeGroupId::try_new("wrong")?;
    assert!(matches!(
        RuntimeBinder.bind_plan(&plan, &bindings),
        Err(RuntimeBindError::IncompatibleDescriptor { .. })
    ));
    bindings[0].endpoint.runtime_group_id = RuntimeGroupId::try_new("control")?;
    bindings[0].endpoint.descriptor_revision = DescriptorRevision::try_new(2)?;
    assert!(matches!(
        RuntimeBinder.bind_plan(&plan, &bindings),
        Err(RuntimeBindError::IncompatibleDescriptor { .. })
    ));
    Ok(())
}

#[test]
fn availability_permissions_and_resources_follow_explicit_compile_policy()
-> Result<(), Box<dyn Error>> {
    let unavailable = GraphPlanCompiler::new(
        catalog(PortAvailability::Disconnected, false)?,
        GraphLimits::default(),
    )?;
    let mut required = graph()?;
    if let GraphNodeKind::Source { availability, .. } = &mut required.graph.nodes[0].kind {
        *availability = EndpointAvailabilityPolicy::Required;
    }
    assert!(matches!(
        unavailable.compile_document(
            &serde_json::to_value(required)?,
            &package_lock()?,
            "lock",
            &fence()?
        ),
        Err(GraphCompileError::RequiredEndpointUnavailable { .. })
    ));

    let denied = GraphPlanCompiler::new(
        catalog(PortAvailability::Available, true)?,
        GraphLimits::default(),
    )?;
    assert!(matches!(
        denied.compile_document(
            &serde_json::to_value(graph()?)?,
            &package_lock()?,
            "lock",
            &fence()?
        ),
        Err(GraphCompileError::MissingPermission { .. })
    ));

    let permission = NamespacedKind::try_new("org.deckweave.permission.invoke")?;
    let resource = NamespacedKind::try_new("org.deckweave.resource.echo")?;
    let permission_only = GraphPlanCompiler::with_access(
        catalog(PortAvailability::Available, true)?,
        GraphLimits::default(),
        CompileAccess {
            endpoint_permissions: Default::default(),
            granted_permissions: [permission.clone()].into_iter().collect(),
            available_resources: Default::default(),
        },
    )?;
    assert!(matches!(
        permission_only.compile_document(
            &serde_json::to_value(graph()?)?,
            &package_lock()?,
            "lock",
            &fence()?
        ),
        Err(GraphCompileError::MissingResource { .. })
    ));

    let allowed = GraphPlanCompiler::with_access(
        catalog(PortAvailability::Available, true)?,
        GraphLimits::default(),
        CompileAccess {
            endpoint_permissions: Default::default(),
            granted_permissions: [permission].into_iter().collect(),
            available_resources: [resource].into_iter().collect(),
        },
    )?;
    allowed.compile_document(
        &serde_json::to_value(graph()?)?,
        &package_lock()?,
        "lock",
        &fence()?,
    )?;
    Ok(())
}
