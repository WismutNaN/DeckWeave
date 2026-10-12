// SPDX-License-Identifier: GPL-3.0-or-later
//! Automation planning contracts. Runtime scheduling is added in Phase 5.

mod fixture;
mod plan;

pub use fixture::{RestrictedFixtureCompiler, VIRTUAL_FIXTURE_PACKAGE_ID};
pub use plan::{
    CompileFence, CompiledExecutionPlan, ExecutionPlanKind, PlanCompileError, PlanCompiler,
};
