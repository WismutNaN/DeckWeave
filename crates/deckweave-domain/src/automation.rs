// SPDX-License-Identifier: GPL-3.0-or-later
use crate::{
    endpoint::RuntimeEndpointRef,
    ids::{
        ActivationEpoch, BindingId, ExecutionId, GraphNodeId, InvocationId, JobId, NamespacedKind,
        OperationId, RuntimeArtifactId,
    },
    schema::{ExtensionMap, SchemaRef},
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;
use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExecutionKind {
    Short,
    Job,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExecutionPolicy {
    pub kind: ExecutionKind,
    pub progress: bool,
    pub cancellation: bool,
    pub reconciliation: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ResourceLeaseMode {
    Shared,
    Exclusive,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResourceRequirement {
    pub scope: NamespacedKind,
    pub mode: ResourceLeaseMode,
    pub hold_until_cleanup: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IdempotencyPolicy {
    pub key_scope: NamespacedKind,
    pub retention_seconds: u64,
    pub outcome_query: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OperationLimits {
    pub deadline_ms: u64,
    pub maximum_input_bytes: u64,
    pub maximum_output_bytes: u64,
    pub maximum_concurrent_invocations: u32,
    pub maximum_queue_depth: u32,
    pub maximum_per_minute: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DataPolicy {
    #[serde(default)]
    pub sensitive_fields: Vec<String>,
    #[serde(default)]
    pub external_recipients: Vec<String>,
    pub trace_allowed: bool,
    pub retention_seconds: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OperationDescriptor {
    pub operation_id: OperationId,
    pub version: u32,
    pub input_schema: SchemaRef,
    pub result_schema: SchemaRef,
    pub error_schema: SchemaRef,
    pub execution: ExecutionPolicy,
    #[serde(default)]
    pub effects: Vec<NamespacedKind>,
    pub idempotency: Option<IdempotencyPolicy>,
    pub limits: OperationLimits,
    #[serde(default)]
    pub resources: Vec<ResourceRequirement>,
    pub data_policy: DataPolicy,
    #[serde(default, flatten)]
    pub extensions: ExtensionMap,
}

/// Compatibility name for consumers that treat a resolved endpoint as a
/// command target. The same fenced value object is used by signal ingress.
pub type ResolvedTarget = RuntimeEndpointRef;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResolvedEndpointRoute {
    pub binding_id: BindingId,
    pub source: RuntimeEndpointRef,
    pub target: RuntimeEndpointRef,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CommandEnvelope {
    pub invocation_id: InvocationId,
    pub execution_id: ExecutionId,
    pub graph_node_id: GraphNodeId,
    pub activation_epoch: ActivationEpoch,
    pub target: ResolvedTarget,
    pub input: Value,
    pub deadline_host_ticks: u64,
    pub idempotency_key: Option<String>,
    #[serde(default, flatten)]
    pub extensions: ExtensionMap,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCategory {
    InvalidInput,
    Unsupported,
    Unavailable,
    PermissionDenied,
    ResourceBusy,
    DeadlineExceeded,
    Cancelled,
    TransientExternal,
    PermanentExternal,
    UnknownOutcome,
    Internal,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeckWeaveError {
    pub category: ErrorCategory,
    pub code: String,
    pub message_key: String,
    #[serde(default)]
    pub details: BTreeMap<String, Value>,
    pub diagnostic_attachment_id: Option<RuntimeArtifactId>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum CommandOutcome {
    Succeeded {
        result: Value,
    },
    Failed {
        error: DeckWeaveError,
    },
    Cancelled {
        error: DeckWeaveError,
        partial_effects: Vec<Value>,
    },
    UnknownOutcome {
        error: DeckWeaveError,
        known_effects: Vec<Value>,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum InvocationReply {
    Terminal {
        invocation_id: InvocationId,
        outcome: CommandOutcome,
    },
    AcceptedJob {
        invocation_id: InvocationId,
        job_id: JobId,
    },
    Rejected {
        invocation_id: InvocationId,
        error: DeckWeaveError,
    },
}

/// Local dispatch result. `Enqueued` only confirms admission to the bounded
/// host queue; it is never evidence that the provider accepted or completed
/// the operation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum InvocationDeliveryOutcome {
    RejectedBeforeSend {
        invocation_id: InvocationId,
        error: DeckWeaveError,
    },
    Enqueued {
        invocation_id: InvocationId,
    },
    PossiblySent {
        invocation_id: InvocationId,
        error: DeckWeaveError,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeArtifactRef {
    pub artifact_id: RuntimeArtifactId,
    pub schema: SchemaRef,
    pub media_type: String,
    pub size_bytes: u64,
    pub digest_sha256: String,
    pub owner_run_id: ExecutionId,
    pub sensitivity: NamespacedKind,
    pub expires_at_rfc3339: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum OperationValidationError {
    #[error("operation version и limits должны быть больше нуля")]
    ZeroLimit,
    #[error("job feature объявлена для short operation")]
    ShortOperationJobFeature,
    #[error("unknown outcome должен иметь category unknown_outcome")]
    WrongUnknownCategory,
    #[error("possibly-sent delivery должен иметь category unknown_outcome")]
    WrongDeliveryCategory,
    #[error("artifact digest должен быть 64-символьным SHA-256 hex")]
    InvalidDigest,
}

impl OperationDescriptor {
    pub fn validate(&self) -> Result<(), OperationValidationError> {
        let limits = &self.limits;
        if self.version == 0
            || limits.deadline_ms == 0
            || limits.maximum_input_bytes == 0
            || limits.maximum_output_bytes == 0
            || limits.maximum_concurrent_invocations == 0
            || limits.maximum_queue_depth == 0
            || limits.maximum_per_minute == 0
        {
            return Err(OperationValidationError::ZeroLimit);
        }
        if self.execution.kind == ExecutionKind::Short
            && (self.execution.progress
                || self.execution.cancellation
                || self.execution.reconciliation)
        {
            return Err(OperationValidationError::ShortOperationJobFeature);
        }
        Ok(())
    }
}

impl InvocationDeliveryOutcome {
    pub fn validate(&self) -> Result<(), OperationValidationError> {
        if let Self::PossiblySent { error, .. } = self
            && error.category != ErrorCategory::UnknownOutcome
        {
            return Err(OperationValidationError::WrongDeliveryCategory);
        }
        Ok(())
    }
}

impl CommandOutcome {
    pub fn validate(&self) -> Result<(), OperationValidationError> {
        if let Self::UnknownOutcome { error, .. } = self
            && error.category != ErrorCategory::UnknownOutcome
        {
            return Err(OperationValidationError::WrongUnknownCategory);
        }
        Ok(())
    }
}

impl RuntimeArtifactRef {
    pub fn validate(&self) -> Result<(), OperationValidationError> {
        if self.digest_sha256.len() != 64
            || !self
                .digest_sha256
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit())
        {
            return Err(OperationValidationError::InvalidDigest);
        }
        Ok(())
    }
}
