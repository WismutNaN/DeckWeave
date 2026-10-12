// SPDX-License-Identifier: GPL-3.0-or-later
use super::GraphCompileError;
impl GraphCompileError {
    pub(super) fn into_plan_error(self) -> crate::PlanCompileError {
        use GraphCompileError::*;
        let (code, node_id, field) = match self {
            InvalidArgument { node_id, field } => ("invalid_argument", Some(node_id), Some(field)),
            UnresolvedEndpoint { node_id } => ("unresolved_endpoint", Some(node_id), None),
            UnsupportedEndpoint { node_id } => ("unsupported_endpoint", Some(node_id), None),
            RequiredEndpointUnavailable { node_id } => {
                ("endpoint_unavailable", Some(node_id), None)
            }
            MissingPermission { node_id, .. } => ("missing_permission", Some(node_id), None),
            MissingResource { node_id, .. } => ("missing_resource", Some(node_id), None),
            UnresolvedSchema { node_id } => ("unresolved_schema", Some(node_id), None),
            SchemaConstraintMismatch { node_id } => ("schema_mismatch", Some(node_id), None),
            OperationMismatch { node_id } => ("operation_mismatch", Some(node_id), None),
            InvalidValue { node_id } => ("invalid_value", Some(node_id), None),
            InvalidTopology { node_id } | DirectionMismatch { node_id, .. } => {
                ("invalid_topology", Some(node_id), None)
            }
            Cycle { .. } => ("graph_cycle", None, None),
            PackageLockMismatch { .. } => ("package_lock_mismatch", None, None),
            _ => ("invalid_graph", None, None),
        };
        crate::PlanCompileError::Diagnostics(vec![deckweave_workspace::DraftDiagnostic {
            code: code.into(),
            node_id,
            field,
        }])
    }
}
