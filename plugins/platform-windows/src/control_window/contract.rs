// SPDX-License-Identifier: GPL-3.0-or-later
use super::{ELEMENT, NODE, PROVIDER, SERVICE, WindowInvocation, WindowResult};
use crate::{
    PlatformFailure,
    window::{WindowCatalogRequest, WindowReference, WindowSelector},
};
use deckweave_protocol::v1::{
    EndpointPath, InvocationRequest, OpaquePayload, SchemaReference, endpoint_path,
};
use serde::Serialize;
use std::time::Duration;

pub(crate) fn parse_invocation(
    request: &InvocationRequest,
) -> Result<Option<WindowInvocation>, PlatformFailure> {
    let Some(target) = request.target.as_ref() else {
        return Err(PlatformFailure::InvalidInput);
    };
    if !is_window_target(target) {
        return Ok(None);
    }
    let timeout = timeout(request)?;
    let invocation = match (target.capability_id.as_str(), target.port_id.as_str()) {
        ("window-activation", "activate") => {
            require_operation(request, "activate-window")?;
            WindowInvocation::Activate(
                json_input::<WindowReference>(request, "org.deckweave.windows.window_ref", 512)?,
                timeout,
            )
        }
        ("window-activation", "focus") => {
            require_operation(request, "focus-application")?;
            WindowInvocation::Focus(
                json_input::<WindowSelector>(
                    request,
                    "org.deckweave.windows.window_selector",
                    66_560,
                )?,
                timeout,
            )
        }
        ("window-catalog", "list") => {
            require_operation(request, "list-windows")?;
            let page = json_input::<WindowCatalogRequest>(
                request,
                "org.deckweave.windows.window_catalog_request",
                128,
            )?;
            if page.limit == 0 || page.limit > 10 || page.offset > 256 {
                return Err(PlatformFailure::InvalidInput);
            }
            WindowInvocation::Catalog(page, timeout)
        }
        _ => return Ok(None),
    };
    Ok(Some(invocation))
}

pub(crate) fn result_payload(result: &WindowResult) -> Result<OpaquePayload, PlatformFailure> {
    match result {
        WindowResult::State(state) => payload("org.deckweave.windows.active_window_state", state),
        WindowResult::Catalog(catalog) => payload("org.deckweave.windows.window_catalog", catalog),
    }
}

pub(super) fn payload<T: Serialize>(
    schema: &str,
    value: &T,
) -> Result<OpaquePayload, PlatformFailure> {
    Ok(OpaquePayload {
        schema: Some(SchemaReference {
            id: schema.into(),
            major: 1,
            minor: 0,
        }),
        value: serde_json::to_vec(value).map_err(|_| PlatformFailure::OsApiUnavailable)?,
    })
}

fn is_window_target(target: &EndpointPath) -> bool {
    target.node_id == NODE
        && target.provider_id == PROVIDER
        && target.element_id == ELEMENT
        && matches!(
            target.target.as_ref(),
            Some(endpoint_path::Target::ServiceId(service)) if service == SERVICE
        )
}

fn require_operation(request: &InvocationRequest, expected: &str) -> Result<(), PlatformFailure> {
    if request.operation_id != expected {
        return Err(PlatformFailure::InvalidInput);
    }
    Ok(())
}

fn timeout(request: &InvocationRequest) -> Result<Duration, PlatformFailure> {
    let budget = request
        .remaining_budget_ms
        .ok_or(PlatformFailure::DeadlineExceeded)?;
    if budget == 0 {
        return Err(PlatformFailure::DeadlineExceeded);
    }
    Ok(Duration::from_millis(budget.min(5_000)))
}

fn json_input<T: serde::de::DeserializeOwned>(
    request: &InvocationRequest,
    expected_schema: &str,
    maximum: usize,
) -> Result<T, PlatformFailure> {
    let input = request
        .input
        .as_ref()
        .ok_or(PlatformFailure::InvalidInput)?;
    if input.value.len() > maximum {
        return Err(PlatformFailure::InvalidInput);
    }
    let schema = input.schema.as_ref().ok_or(PlatformFailure::InvalidInput)?;
    if schema.id != expected_schema || schema.major != 1 || schema.minor != 0 {
        return Err(PlatformFailure::InvalidInput);
    }
    serde_json::from_slice(&input.value).map_err(|_| PlatformFailure::InvalidInput)
}
