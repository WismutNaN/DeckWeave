// SPDX-License-Identifier: GPL-3.0-or-later
//! Automation planning contracts. Runtime scheduling is added in Phase 5.

mod compile;
mod executable;
mod fixture;
mod graph;
mod plan;
mod scheduler;
mod simulation;

pub use compile::{
    CatalogEndpoint, CatalogError, CatalogSchema, CompileAccess, DescriptorCatalog,
    GraphCompileError, GraphPlanCompiler, RuntimeBindError, RuntimeBinder, SchemaValueKind,
};
pub use executable::{
    BoundEndpoint, BoundExecutionGraph, CompileDiagnostic, CompileDiagnosticCode,
    CompiledEndpointContract, CompiledSchemaContract, ExecutableGraph, RuntimeBinding,
    SourceSubscription, StateSlot,
};
pub use fixture::{
    RestrictedFixtureCompiler, VIRTUAL_FIXTURE_CONTRIBUTION_ID, VIRTUAL_FIXTURE_PACKAGE_ID,
    VIRTUAL_FIXTURE_RUNTIME_GROUP_ID,
};
pub use graph::{
    ConditionOperator, EndpointAvailabilityPolicy, GraphDefinition, GraphDocument, GraphEdge,
    GraphEdgeId, GraphEdgeKind, GraphFormatVersion, GraphIdentityError, GraphLayout,
    GraphLimitKind, GraphLimits, GraphNode, GraphNodeKind, GraphNodePlacement, GraphPort,
    GraphPortId, GraphPortKind, GraphPortRef, GraphPortRole, GraphValidationError,
};
pub use plan::{
    COMPILED_PLAN_EXTENSION_ID, CompileFence, CompiledExecutionPlan, DurableExecutionPlan,
    PlanCompileError, PlanCompiler, RecoveryCompileInput, RuntimeRequirement,
};
pub use scheduler::{
    IngressContext, IngressItem, IngressKind, IngressOverflow, IngressQueue, IngressReliability,
    PendingInvocation, QueueAdmission, SchedulerFault, SchedulerIntent, SchedulerState,
    SchedulerStep, SchedulerTransition, SchedulerTransitionError, WorkspaceScheduler, transition,
};
pub use simulation::{
    LatencyPercentiles, SIMULATION_FIXTURE_VERSION, SIMULATION_TRACE_VERSION, SimulationError,
    SimulationEvent, SimulationFixture, SimulationLatency, SimulationReport, SimulationRunner,
    SimulationStatus, SimulationTrace, SimulationTraceEntry, SimulationTraceKind,
};
