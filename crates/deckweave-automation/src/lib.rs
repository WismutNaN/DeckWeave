// SPDX-License-Identifier: GPL-3.0-or-later
//! Automation planning contracts. Runtime scheduling is added in Phase 5.

mod fixture;
mod graph;
mod plan;

pub use fixture::{
    RestrictedFixtureCompiler, VIRTUAL_FIXTURE_CONTRIBUTION_ID, VIRTUAL_FIXTURE_PACKAGE_ID,
    VIRTUAL_FIXTURE_RUNTIME_GROUP_ID,
};
pub use graph::{
    ConditionOperator, GraphDefinition, GraphDocument, GraphEdge, GraphEdgeId, GraphEdgeKind,
    GraphFormatVersion, GraphIdentityError, GraphLayout, GraphLimitKind, GraphLimits, GraphNode,
    GraphNodeKind, GraphNodePlacement, GraphPort, GraphPortId, GraphPortKind, GraphPortRef,
    GraphValidationError,
};
pub use plan::{
    COMPILED_PLAN_EXTENSION_ID, CompileFence, CompiledExecutionPlan, DurableExecutionPlan,
    PlanCompileError, PlanCompiler, RecoveryCompileInput, RuntimeRequirement,
};
