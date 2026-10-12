// SPDX-License-Identifier: GPL-3.0-or-later
//! Versioned workspace graph owned by Automation Execution.

mod arguments;
mod model;
pub use arguments::*;
mod validation;

pub use model::{
    ConditionOperator, EndpointAvailabilityPolicy, GraphDefinition, GraphDocument, GraphEdge,
    GraphEdgeId, GraphEdgeKind, GraphFormatVersion, GraphIdentityError, GraphLayout, GraphNode,
    GraphNodeKind, GraphNodePlacement, GraphPort, GraphPortId, GraphPortKind, GraphPortRef,
    GraphPortRole,
};
pub use validation::{GraphLimitKind, GraphLimits, GraphValidationError};
