// SPDX-License-Identifier: GPL-3.0-or-later
use super::*;
impl ProductionRuntimeCoordinator {
    pub fn with_credential_broker(
        mut self,
        broker: Arc<dyn deckweave_application::CredentialBroker>,
    ) -> Self {
        self.credential_broker = broker;
        self
    }
    pub(super) fn launch_requirement(
        &self,
        requirement: &RuntimeRequirement,
    ) -> Result<RuntimeInstance, HostRuntimeError> {
        let (package, group, contribution) = self.resolve_requirement(requirement)?;
        let mut config = self.config.clone();
        config.locale = self
            .locale
            .lock()
            .map_err(|_| HostRuntimeError::State("locale lock poisoned".into()))?
            .clone();
        RuntimeInstance::launch(
            package,
            group,
            contribution,
            requirement.configuration.as_ref(),
            self.allocate_generation()?,
            &config,
            Arc::clone(&self.authorization),
            self.principal.clone(),
            self.process_policy.as_ref(),
            elapsed_millis(self.clock),
        )
    }

    pub(super) fn resolve_requirement(
        &self,
        requirement: &RuntimeRequirement,
    ) -> Result<(Arc<IndexedPackage>, RuntimeGroupId, ContributionId), HostRuntimeError> {
        if let Some(configuration) = &requirement.configuration {
            for reference in configuration.credentials.values() {
                self.credential_broker
                    .check_reference(&deckweave_domain::security::CredentialRequest {
                        configured_instance_id: Some(configuration.instance_id.clone()),
                        principal_id: self.principal.clone(),
                        package_id: requirement.package_id.clone(),
                        credential_ref: reference.clone(),
                        purpose: deckweave_domain::ids::NamespacedKind::try_new(
                            "org.deckweave.credential.plugin-use",
                        )
                        .map_err(|e| HostRuntimeError::State(e.to_string()))?,
                        deadline_host_ticks: elapsed_millis(self.clock).saturating_add(
                            self.config
                                .handshake_timeout
                                .as_millis()
                                .min(u128::from(u64::MAX)) as u64,
                        ),
                    })
                    .map_err(|_| {
                        HostRuntimeError::State(format!(
                            "credential needs attention for instance {}",
                            configuration.instance_id
                        ))
                    })?;
            }
        }
        let package = self
            .packages
            .resolve_artifact(
                &requirement.package_id,
                &requirement.package_version,
                &requirement.artifact_sha256,
            )
            .cloned()
            .map(Arc::new)
            .ok_or_else(|| HostRuntimeError::Package(requirement.package_id.to_string()))?;
        if !package
            .runtime_groups
            .contains_key(&requirement.runtime_group_id)
        {
            return Err(HostRuntimeError::Package(format!(
                "runtime group {} is absent from package {}",
                requirement.runtime_group_id, requirement.package_id
            )));
        }
        let contribution = package
            .contributions
            .get(&requirement.contribution_id)
            .ok_or_else(|| {
                HostRuntimeError::Package(format!(
                    "contribution {} is absent from package {}",
                    requirement.contribution_id, requirement.package_id
                ))
            })?;
        if contribution.runtime_group_id != requirement.runtime_group_id {
            return Err(HostRuntimeError::Package(
                "compiled runtime group does not own the required contribution".into(),
            ));
        }
        Ok((
            package,
            requirement.runtime_group_id.clone(),
            requirement.contribution_id.clone(),
        ))
    }
}
