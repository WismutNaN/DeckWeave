// SPDX-License-Identifier: GPL-3.0-or-later

use deckweave_domain::{
    automation::{
        DataPolicy, DeckWeaveError, ErrorCategory, ExecutionKind, ExecutionPolicy,
        InvocationDeliveryOutcome, OperationDescriptor, OperationLimits, OperationValidationError,
    },
    ids::{InvocationId, NamespacedKind, OperationId},
    schema::{SchemaRef, SchemaVersion},
};
use std::{collections::BTreeMap, error::Error};

fn schema(id: &str) -> Result<SchemaRef, Box<dyn Error>> {
    Ok(SchemaRef {
        id: NamespacedKind::try_new(id)?,
        version: SchemaVersion { major: 1, minor: 0 },
    })
}

fn error(category: ErrorCategory) -> DeckWeaveError {
    DeckWeaveError {
        category,
        code: "transport".into(),
        message_key: "automation.transport".into(),
        details: BTreeMap::new(),
        diagnostic_attachment_id: None,
    }
}

#[test]
fn enqueue_is_not_terminal_and_uncertain_delivery_is_explicit() -> Result<(), Box<dyn Error>> {
    let invocation_id = InvocationId::try_new("invocation-1")?;
    let enqueued = InvocationDeliveryOutcome::Enqueued {
        invocation_id: invocation_id.clone(),
    };
    assert_eq!(enqueued.validate(), Ok(()));

    let uncertain = InvocationDeliveryOutcome::PossiblySent {
        invocation_id: invocation_id.clone(),
        error: error(ErrorCategory::UnknownOutcome),
    };
    assert_eq!(uncertain.validate(), Ok(()));

    let invalid = InvocationDeliveryOutcome::PossiblySent {
        invocation_id,
        error: error(ErrorCategory::Unavailable),
    };
    assert_eq!(
        invalid.validate(),
        Err(OperationValidationError::WrongDeliveryCategory)
    );
    Ok(())
}

#[test]
fn operation_queue_limit_must_be_bounded() -> Result<(), Box<dyn Error>> {
    let value_schema = schema("org.deckweave.schema.value")?;
    let descriptor = OperationDescriptor {
        operation_id: OperationId::try_new("operation")?,
        version: 1,
        input_schema: value_schema.clone(),
        result_schema: value_schema.clone(),
        error_schema: value_schema,
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
            maximum_output_bytes: 1_024,
            maximum_concurrent_invocations: 1,
            maximum_queue_depth: 0,
            maximum_per_minute: 60,
        },
        resources: vec![],
        data_policy: DataPolicy {
            sensitive_fields: vec![],
            external_recipients: vec![],
            trace_allowed: true,
            retention_seconds: 60,
        },
        extensions: BTreeMap::new(),
    };
    assert_eq!(
        descriptor.validate(),
        Err(OperationValidationError::ZeroLimit)
    );
    Ok(())
}
