// SPDX-License-Identifier: GPL-3.0-or-later
use super::*;
impl GraphPlanCompiler {
    pub(super) fn validate_access(
        &self,
        node: &GraphNode,
        endpoint: &CatalogEndpoint,
    ) -> Result<(), GraphCompileError> {
        for permission in &endpoint.port.required_permissions {
            if !self.access.granted_permissions.contains(permission)
                && !self
                    .access
                    .endpoint_permissions
                    .get(&endpoint.endpoint)
                    .is_some_and(|scopes| scopes.contains(permission))
            {
                return Err(GraphCompileError::MissingPermission {
                    node_id: node.id.clone(),
                    scope: permission.clone(),
                });
            }
        }
        if let Some(operation) = &endpoint.operation {
            for resource in &operation.resources {
                if !self.access.available_resources.contains(&resource.scope) {
                    return Err(GraphCompileError::MissingResource {
                        node_id: node.id.clone(),
                        scope: resource.scope.clone(),
                    });
                }
            }
        }
        Ok(())
    }
}
