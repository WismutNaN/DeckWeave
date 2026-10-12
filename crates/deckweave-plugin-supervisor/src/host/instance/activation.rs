// SPDX-License-Identifier: GPL-3.0-or-later
//! Дополнительные activation leases одной изолированной runtime group.
use super::*;
use deckweave_protocol::{
    FEATURE_CONFIGURED_INSTANCES,
    v1::{OpaquePayload, SchemaReference},
};
use deckweave_workspace::PluginInstanceConfiguration;

pub(super) fn validate_features(
    controller: &RuntimeController,
    configuration: Option<&PluginInstanceConfiguration>,
) -> Result<(), HostRuntimeError> {
    let Some(configuration) = configuration else {
        return Ok(());
    };
    configuration
        .validate()
        .map_err(|e| HostRuntimeError::Protocol(e.into()))?;
    let features = &controller
        .session()
        .ok_or_else(|| HostRuntimeError::Protocol("missing session".into()))?
        .negotiated_features;
    if features.get(FEATURE_CONFIGURED_INSTANCES).copied() != Some(1)
        || features
            .get(deckweave_protocol::FEATURE_ACTIVATION_COMMIT)
            .copied()
            != Some(1)
        || configuration
            .required_features
            .iter()
            .any(|(id, version)| features.get(id.as_str()) != Some(version))
    {
        return Err(HostRuntimeError::Protocol(
            "configured instance requires unnegotiated feature".into(),
        ));
    }
    Ok(())
}

pub(super) fn settings_payload(
    configuration: Option<&PluginInstanceConfiguration>,
) -> Result<Option<OpaquePayload>, HostRuntimeError> {
    configuration
        .map(|c| {
            Ok(OpaquePayload {
                schema: Some(SchemaReference {
                    id: c.settings_schema.id.to_string(),
                    major: c.settings_schema.version.major,
                    minor: c.settings_schema.version.minor,
                }),
                value: serde_json::to_vec(&c.settings)
                    .map_err(|e| HostRuntimeError::Protocol(e.to_string()))?,
            })
        })
        .transpose()
}

pub(super) fn decode_nodes(
    payload: &OpaquePayload,
) -> Result<Vec<NodeDescriptor>, HostRuntimeError> {
    if !payload.schema.as_ref().is_some_and(|s| {
        s.id == "org.deckweave.descriptor.node_list" && s.major == 1 && s.minor == 0
    }) {
        return Err(HostRuntimeError::Protocol(
            "descriptor schema mismatch".into(),
        ));
    }
    let nodes: Vec<NodeDescriptor> = serde_json::from_slice(&payload.value)
        .map_err(|e| HostRuntimeError::Protocol(e.to_string()))?;
    if nodes.len() > 4096 {
        return Err(HostRuntimeError::Protocol(
            "descriptor node limit exceeded".into(),
        ));
    }
    for node in &nodes {
        node.validate()
            .map_err(|e| HostRuntimeError::Protocol(e.to_string()))?;
    }
    Ok(nodes)
}

impl RuntimeInstance {
    pub(in crate::host) fn belongs_to(
        &self,
        requirement: &deckweave_automation::RuntimeRequirement,
    ) -> bool {
        let key = self.controller.health().key;
        key.package_id == requirement.package_id
            && key.runtime_group_id == requirement.runtime_group_id
    }

    pub(in crate::host) fn activate_additional(
        &mut self,
        requirement: &deckweave_automation::RuntimeRequirement,
        config: &HostRuntimeConfig,
    ) -> Result<(), HostRuntimeError> {
        if self
            .controller
            .session()
            .and_then(|s| s.negotiated_features.get(FEATURE_CONFIGURED_INSTANCES))
            .copied()
            != Some(1)
            || self
                .controller
                .session()
                .and_then(|s| {
                    s.negotiated_features
                        .get(deckweave_protocol::FEATURE_ACTIVATION_COMMIT)
                })
                .copied()
                != Some(1)
        {
            return Err(HostRuntimeError::Protocol(
                "grouped activation is not negotiated".into(),
            ));
        }
        let configuration = requirement.configuration.as_ref();
        validate_features(&self.controller, configuration)?;
        let (lease, _) = self
            .controller
            .acquire_lease(RuntimeLeaseReason::Binding, None)?;
        let request = ActivationRequest {
            runtime_lease_id: lease.lease_id.to_string(),
            contribution_id: requirement.contribution_id.to_string(),
            settings: settings_payload(configuration)?,
            locale: Some(wire::locale_context(&config.locale)),
            configured_instance_id: configuration.map(|c| c.instance_id.to_string()),
            settings_revision: configuration.map_or(0, |c| c.settings_revision),
            credential_refs: configuration
                .map(|c| {
                    c.credentials
                        .iter()
                        .map(|(k, v)| (k.clone(), v.to_string()))
                        .collect()
                })
                .unwrap_or_default(),
        };
        self.process.send(&envelope(
            self.generation,
            &mut self.next_message_id,
            protocol_envelope::Body::Activate(request),
        )?)?;
        let deadline = Instant::now() + config.handshake_timeout;
        let activated = loop {
            let now = Instant::now();
            if now >= deadline {
                return Err(HostRuntimeError::Protocol("activation timeout".into()));
            }
            let message = self.process.receive(deadline - now)?;
            self.controller.validate_envelope(&message)?;
            match message.body {
                Some(protocol_envelope::Body::Activated(reply)) => break reply,
                Some(protocol_envelope::Body::Heartbeat(_)) => continue,
                _ => {
                    return Err(HostRuntimeError::Protocol(
                        "provider emitted data during preparation".into(),
                    ));
                }
            }
        };
        if activated.runtime_lease_id != lease.lease_id.as_str()
            || activated.descriptor_revision < self.descriptor_revision.get()
        {
            return Err(HostRuntimeError::Protocol(
                "activation identity/revision mismatch".into(),
            ));
        }
        let nodes =
            decode_nodes(&activated.descriptor.ok_or_else(|| {
                HostRuntimeError::Protocol("activation descriptor missing".into())
            })?)?;
        self.activation_nodes.push((
            requirement.contribution_id.clone(),
            configuration.map(|c| c.instance_id.clone()),
            nodes.clone(),
        ));
        for node in nodes {
            if let Some(existing) = self.nodes.iter_mut().find(|n| n.node_id == node.node_id) {
                if existing.kind != node.kind {
                    return Err(HostRuntimeError::Protocol("node kind conflict".into()));
                }
                existing.devices.extend(node.devices);
                existing.services.extend(node.services);
                existing
                    .validate()
                    .map_err(|e| HostRuntimeError::Protocol(e.to_string()))?;
            } else {
                self.nodes.push(node);
            }
        }
        if self.nodes.len() > 4096 {
            return Err(HostRuntimeError::Protocol(
                "group descriptor node limit exceeded".into(),
            ));
        }
        self.descriptor_revision = DescriptorRevision::try_new(activated.descriptor_revision)
            .map_err(|e| HostRuntimeError::Protocol(e.to_string()))?;
        for node in &mut self.nodes {
            node.descriptor_revision = self.descriptor_revision;
            for element in node
                .devices
                .iter_mut()
                .flat_map(|d| &mut d.elements)
                .chain(node.services.iter_mut().flat_map(|s| &mut s.elements))
            {
                for port in element.capabilities.iter_mut().flat_map(|c| &mut c.ports) {
                    port.descriptor_revision = self.descriptor_revision;
                }
            }
        }
        self.activation_leases.push(lease.lease_id);
        self.contributions.push(requirement.contribution_id.clone());
        Ok(())
    }
}
