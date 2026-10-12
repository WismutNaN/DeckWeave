// SPDX-License-Identifier: GPL-3.0-or-later
//! Versioned workspace graph owned by Automation Execution.

mod model;
mod validation;

pub use model::{
    ConditionOperator, GraphDefinition, GraphDocument, GraphEdge, GraphEdgeId, GraphEdgeKind,
    GraphFormatVersion, GraphIdentityError, GraphLayout, GraphNode, GraphNodeKind,
    GraphNodePlacement, GraphPort, GraphPortId, GraphPortKind, GraphPortRef,
};
pub use validation::{GraphLimitKind, GraphLimits, GraphValidationError};
