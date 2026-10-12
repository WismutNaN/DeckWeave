// SPDX-License-Identifier: GPL-3.0-or-later

use crate::RuntimeRequirement;
use deckweave_domain::{
    automation::OperationDescriptor,
    capability::{NumericRange, PortDescriptor},
    endpoint::StableEndpointRef,
    schema::SchemaRef,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SchemaValueKind {
    Unit,
    Boolean,
    Number,
    String,
    Json,
    Structured(deckweave_domain::schema::PayloadSchema),
}

impl SchemaValueKind {
    pub fn accepts(&self, value: &serde_json::Value) -> bool {
        use deckweave_domain::schema::bounded_json;
        match self {
            Self::Unit => value.is_null(),
            Self::Boolean => value.is_boolean(),
            Self::Number => value.as_f64().is_some_and(f64::is_finite),
            Self::String => value.is_string() && bounded_json(value),
            Self::Json => bounded_json(value),
            Self::Structured(schema) => schema.accepts(value),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CatalogSchema {
    pub schema: SchemaRef,
    pub value_kind: SchemaValueKind,
    pub unit: Option<String>,
    pub range: Option<NumericRange>,
}

impl CatalogSchema {
    pub fn accepts(&self, value: &serde_json::Value) -> bool {
        self.value_kind.accepts(value)
            && self.range.as_ref().is_none_or(|range| {
                value
                    .as_f64()
                    .is_some_and(|number| number >= range.minimum && number <= range.maximum)
            })
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CatalogEndpoint {
    pub endpoint: StableEndpointRef,
    pub runtime_requirement: RuntimeRequirement,
    pub port: PortDescriptor,
    pub operation: Option<OperationDescriptor>,
}

#[derive(Debug, Clone)]
pub struct DescriptorCatalog {
    schemas: Vec<CatalogSchema>,
    endpoints: Vec<CatalogEndpoint>,
    semantic_sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum CatalogError {
    #[error("duplicate schema {0:?}")]
    DuplicateSchema(SchemaRef),
    #[error("duplicate endpoint {0}")]
    DuplicateEndpoint(String),
    #[error("catalog endpoint path does not match its port descriptor")]
    PortPathMismatch,
    #[error("catalog endpoint package does not match its runtime requirement")]
    PackageMismatch,
    #[error("catalog artifact hash is not SHA-256 hex")]
    InvalidArtifactHash,
    #[error("catalog descriptor is invalid: {0}")]
    InvalidDescriptor(String),
    #[error("catalog operation is invalid: {0}")]
    InvalidOperation(String),
    #[error("catalog references unknown schema {0:?}")]
    UnknownSchema(SchemaRef),
    #[error("catalog serialization failed: {0}")]
    Serialization(String),
}

impl DescriptorCatalog {
    pub fn schemas(&self) -> &[CatalogSchema] {
        &self.schemas
    }
    pub fn endpoints(&self) -> &[CatalogEndpoint] {
        &self.endpoints
    }
    pub fn try_new(
        mut schemas: Vec<CatalogSchema>,
        mut endpoints: Vec<CatalogEndpoint>,
    ) -> Result<Self, CatalogError> {
        schemas.sort_by(|left, right| {
            left.schema
                .id
                .cmp(&right.schema.id)
                .then(left.schema.version.cmp(&right.schema.version))
        });
        endpoints.sort_by(|left, right| left.endpoint.cmp(&right.endpoint));

        for pair in schemas.windows(2) {
            if pair[0].schema == pair[1].schema {
                return Err(CatalogError::DuplicateSchema(pair[0].schema.clone()));
            }
        }
        for schema in &schemas {
            if let SchemaValueKind::Structured(shape) = &schema.value_kind {
                shape
                    .validate_definition()
                    .map_err(|e| CatalogError::InvalidDescriptor(e.into()))?;
            }
            if let Some(range) = &schema.range
                && (!range.minimum.is_finite()
                    || !range.maximum.is_finite()
                    || range.minimum > range.maximum)
            {
                return Err(CatalogError::InvalidDescriptor(
                    "invalid schema numeric range".into(),
                ));
            }
        }
        for pair in endpoints.windows(2) {
            if pair[0].endpoint == pair[1].endpoint {
                return Err(CatalogError::DuplicateEndpoint(format!(
                    "{:?}",
                    pair[0].endpoint
                )));
            }
        }

        let known_schemas: BTreeSet<_> = schemas
            .iter()
            .map(|entry| (entry.schema.id.clone(), entry.schema.version))
            .collect();
        for endpoint in &endpoints {
            if endpoint.endpoint.port.port_id != endpoint.port.port_id {
                return Err(CatalogError::PortPathMismatch);
            }
            if endpoint.endpoint.package_id != endpoint.runtime_requirement.package_id {
                return Err(CatalogError::PackageMismatch);
            }
            if endpoint.runtime_requirement.artifact_sha256.len() != 64
                || !endpoint
                    .runtime_requirement
                    .artifact_sha256
                    .bytes()
                    .all(|byte| byte.is_ascii_hexdigit())
            {
                return Err(CatalogError::InvalidArtifactHash);
            }
            endpoint
                .port
                .validate()
                .map_err(|error| CatalogError::InvalidDescriptor(error.to_string()))?;
            require_schema(&known_schemas, &endpoint.port.schema)?;
            if let Some(operation) = &endpoint.operation {
                operation
                    .validate()
                    .map_err(|error| CatalogError::InvalidOperation(error.to_string()))?;
                require_schema(&known_schemas, &operation.input_schema)?;
                require_schema(&known_schemas, &operation.result_schema)?;
                require_schema(&known_schemas, &operation.error_schema)?;
                if endpoint.port.schema != operation.input_schema {
                    return Err(CatalogError::InvalidOperation(
                        "command port schema differs from operation input schema".into(),
                    ));
                }
            }
        }

        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct SemanticCatalog<'a> {
            schemas: &'a [CatalogSchema],
            endpoints: &'a [CatalogEndpoint],
        }
        let bytes = serde_json::to_vec(&SemanticCatalog {
            schemas: &schemas,
            endpoints: &endpoints,
        })
        .map_err(|error| CatalogError::Serialization(error.to_string()))?;
        Ok(Self {
            schemas,
            endpoints,
            semantic_sha256: format!("{:x}", Sha256::digest(bytes)),
        })
    }

    #[must_use]
    pub fn schema(&self, reference: &SchemaRef) -> Option<&CatalogSchema> {
        self.schemas.iter().find(|entry| entry.schema == *reference)
    }

    #[must_use]
    pub fn endpoint(&self, reference: &StableEndpointRef) -> Option<&CatalogEndpoint> {
        self.endpoints
            .binary_search_by(|entry| entry.endpoint.cmp(reference))
            .ok()
            .map(|index| &self.endpoints[index])
    }

    #[must_use]
    pub fn semantic_sha256(&self) -> &str {
        &self.semantic_sha256
    }
}

fn require_schema(
    known: &BTreeSet<(
        deckweave_domain::ids::NamespacedKind,
        deckweave_domain::schema::SchemaVersion,
    )>,
    reference: &SchemaRef,
) -> Result<(), CatalogError> {
    if !known.contains(&(reference.id.clone(), reference.version)) {
        return Err(CatalogError::UnknownSchema(reference.clone()));
    }
    Ok(())
}
