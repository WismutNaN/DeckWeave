// SPDX-License-Identifier: GPL-3.0-or-later
//! Live virtual click and an explicit display-only projection. Trace stays redacted.
use super::*;
use deckweave_application::virtual_control::{VirtualDisplayValue, VirtualSurfaceSnapshot};
use deckweave_domain::{capability::PortKind, endpoint::StableEndpointRef};
use std::collections::BTreeSet;

impl LiveExecutionOwner {
    pub fn virtual_click(
        &self,
        epoch: ActivationEpoch,
        source: &RuntimeEndpointRef,
    ) -> Result<(), LiveExecutionError> {
        let _order = self
            .transition_order
            .lock()
            .map_err(|_| LiveExecutionError::Poisoned)?;
        {
            let state = self
                .state
                .lock()
                .map_err(|_| LiveExecutionError::Poisoned)?;
            let active = state
                .active
                .as_ref()
                .filter(|a| a.epoch == epoch && !a.scheduler.state().closed)
                .ok_or(LiveExecutionError::MissingGraph)?;
            if !active.graph.endpoint_contracts.iter().any(|c| {
                c.endpoint == source.stable
                    && c.port.kind.as_str() == PortKind::EVENT_OUT
                    && c.port
                        .tags
                        .iter()
                        .any(|t| t == "org.deckweave.virtual.press")
                    && active.bindings.get(&c.node_id) == Some(source)
            }) {
                return Err(LiveExecutionError::MissingGraph);
            }
        }
        // The transition lock covers both edges, including dispatch and barrier ordering.
        for pressed in [true, false] {
            self.ingest_ordered(
                Some(epoch),
                IngressKind::Signal {
                    source: source.stable.clone(),
                    payload: serde_json::json!(pressed),
                    source_ticks: self.clock.now_ticks(),
                    reliability: deckweave_automation::IngressReliability::Reliable,
                },
                self.clock.now_ticks(),
            )?;
        }
        Ok(())
    }
    pub fn virtual_surface(
        &self,
        allowed: &BTreeSet<StableEndpointRef>,
    ) -> Result<VirtualSurfaceSnapshot, LiveExecutionError> {
        let state = self
            .state
            .lock()
            .map_err(|_| LiveExecutionError::Poisoned)?;
        let Some(active) = &state.active else {
            return Ok(VirtualSurfaceSnapshot::default());
        };
        let displays = active
            .graph
            .endpoint_contracts
            .iter()
            .filter(|c| allowed.contains(&c.endpoint))
            .filter_map(|c| {
                let edge = active.graph.edges.iter().find(|e| {
                    e.source.node_id == c.node_id
                        && active
                            .graph
                            .state_slots
                            .iter()
                            .any(|s| s.node_id == e.target.node_id)
                })?;
                let text = active
                    .scheduler
                    .state()
                    .values
                    .get(&edge.target.node_id)?
                    .as_str()?;
                (text.len() <= 2048).then(|| VirtualDisplayValue {
                    endpoint: c.endpoint.clone(),
                    text: text.into(),
                })
            })
            .take(512)
            .collect();
        Ok(VirtualSurfaceSnapshot {
            activation_epoch: Some(active.epoch),
            displays,
        })
    }
}
