// SPDX-License-Identifier: GPL-3.0-or-later
//! Ownership and lifecycle of prepared and active runtime generations.

use super::{
    config::{HostRuntimeConfig, HostRuntimeError},
    instance::{ActiveWorker, RuntimeInstance, elapsed_millis},
};
use crate::package::{IndexedPackage, PackageIndex};
use deckweave_application::AuthorizationStore;
use deckweave_automation::{CompiledExecutionPlan, RuntimeRequirement};
use deckweave_domain::ids::{ContributionId, HostBootId, OperationId, PrincipalId, RuntimeGroupId};
use deckweave_host::RuntimeCommitOutcome;
use deckweave_platform::ChildProcessPolicy;
use deckweave_registry::{CapabilityRegistry, RegistrySnapshot};
use deckweave_workspace::{ApplyCommitRecord, ApplyOperationRepository};
use std::{
    sync::{Arc, Mutex},
    time::Instant,
};

pub(super) enum PreparedRuntime {
    Empty {
        operation_id: OperationId,
        plan_hash: String,
    },
    Virtual {
        operation_id: OperationId,
        plan_hash: String,
        instance: Box<RuntimeInstance>,
    },
}

impl PreparedRuntime {
    pub(super) fn matches(&self, operation_id: &OperationId, plan_hash: &str) -> bool {
        match self {
            Self::Empty {
                operation_id: current,
                plan_hash: hash,
            }
            | Self::Virtual {
                operation_id: current,
                plan_hash: hash,
                ..
            } => current == operation_id && hash == plan_hash,
        }
    }

    pub(super) fn operation_id(&self) -> &OperationId {
        match self {
            Self::Empty { operation_id, .. } | Self::Virtual { operation_id, .. } => operation_id,
        }
    }
}

#[derive(Default)]
pub(super) struct RuntimeState {
    pub(super) next_generation: u64,
    pub(super) prepared: Option<PreparedRuntime>,
    pub(super) active: Option<ActiveWorker>,
    pub(super) dispatcher_closed: bool,
}

pub struct ProductionRuntimeCoordinator {
    pub(super) packages: Arc<PackageIndex>,
    pub(super) operations: Arc<dyn ApplyOperationRepository>,
    pub(super) authorization: Arc<dyn AuthorizationStore>,
    pub(super) registry: Arc<Mutex<CapabilityRegistry>>,
    pub(super) process_policy: Arc<dyn ChildProcessPolicy>,
    pub(super) principal: PrincipalId,
    pub(super) host_boot_id: HostBootId,
    pub(super) config: HostRuntimeConfig,
    pub(super) clock: Instant,
    pub(super) state: Mutex<RuntimeState>,
}

impl ProductionRuntimeCoordinator {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        packages: Arc<PackageIndex>,
        operations: Arc<dyn ApplyOperationRepository>,
        authorization: Arc<dyn AuthorizationStore>,
        registry: Arc<Mutex<CapabilityRegistry>>,
        process_policy: Arc<dyn ChildProcessPolicy>,
        principal: PrincipalId,
        host_boot_id: HostBootId,
        config: HostRuntimeConfig,
    ) -> Self {
        Self {
            packages,
            operations,
            authorization,
            registry,
            process_policy,
            principal,
            host_boot_id,
            config,
            clock: Instant::now(),
            state: Mutex::new(RuntimeState {
                dispatcher_closed: true,
                ..RuntimeState::default()
            }),
        }
    }

    pub fn registry_snapshot(&self) -> Result<RegistrySnapshot, HostRuntimeError> {
        self.registry
            .lock()
            .map_err(|_| HostRuntimeError::State("registry lock is poisoned".into()))?
            .snapshot()
            .map_err(Into::into)
    }

    pub fn shutdown(&self) -> Result<(), HostRuntimeError> {
        let (prepared, active) = {
            let mut state = self.lock_state()?;
            state.dispatcher_closed = true;
            (state.prepared.take(), state.active.take())
        };
        if let Some(prepared) = prepared {
            self.stop_prepared(prepared)?;
        }
        if let Some(active) = active {
            active.shutdown(self.config.shutdown_timeout)?;
        }
        Ok(())
    }

    pub(super) fn lock_state(
        &self,
    ) -> Result<std::sync::MutexGuard<'_, RuntimeState>, HostRuntimeError> {
        self.state
            .lock()
            .map_err(|_| HostRuntimeError::State("runtime state lock is poisoned".into()))
    }

    fn allocate_generation(&self) -> Result<u64, HostRuntimeError> {
        let mut state = self.lock_state()?;
        state.next_generation = state
            .next_generation
            .checked_add(1)
            .ok_or_else(|| HostRuntimeError::State("session generation exhausted".into()))?;
        Ok(state.next_generation)
    }

    pub(super) fn prepare_requirements(
        &self,
        operation_id: &OperationId,
        plan_hash: &str,
        requirements: &[RuntimeRequirement],
    ) -> Result<PreparedRuntime, HostRuntimeError> {
        if requirements.len() > 1 {
            return Err(HostRuntimeError::Package(
                "multiple runtime groups are not supported by this coordinator yet".into(),
            ));
        }
        let Some(requirement) = requirements.first() else {
            return Ok(PreparedRuntime::Empty {
                operation_id: operation_id.clone(),
                plan_hash: plan_hash.into(),
            });
        };
        let (package, group, contribution) = self.resolve_requirement(requirement)?;
        let instance = RuntimeInstance::launch(
            package,
            group,
            contribution,
            self.allocate_generation()?,
            &self.config,
            Arc::clone(&self.authorization),
            self.principal.clone(),
            self.process_policy.as_ref(),
            elapsed_millis(self.clock),
        )?;
        Ok(PreparedRuntime::Virtual {
            operation_id: operation_id.clone(),
            plan_hash: plan_hash.into(),
            instance: Box::new(instance),
        })
    }

    fn resolve_requirement(
        &self,
        requirement: &RuntimeRequirement,
    ) -> Result<(Arc<IndexedPackage>, RuntimeGroupId, ContributionId), HostRuntimeError> {
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

    pub(super) fn stop_prepared(&self, prepared: PreparedRuntime) -> Result<(), HostRuntimeError> {
        if let PreparedRuntime::Virtual { instance, .. } = prepared {
            let now = elapsed_millis(self.clock);
            let mut registry = self
                .registry
                .lock()
                .map_err(|_| HostRuntimeError::State("registry lock is poisoned".into()))?;
            (*instance).stop(&mut registry, self.config.stop_timeout, now)?;
        }
        Ok(())
    }

    pub(super) fn activate_prepared(
        &self,
        prepared: PreparedRuntime,
    ) -> Result<RuntimeCommitOutcome, HostRuntimeError> {
        let active = self.lock_state()?.active.take();
        if let Some(active) = active {
            active.shutdown(self.config.shutdown_timeout)?;
        }
        let worker = match prepared {
            PreparedRuntime::Empty { .. } => None,
            PreparedRuntime::Virtual { mut instance, .. } => {
                {
                    let mut registry = self
                        .registry
                        .lock()
                        .map_err(|_| HostRuntimeError::State("registry lock is poisoned".into()))?;
                    instance.publish(&mut registry, elapsed_millis(self.clock))?;
                }
                Some(ActiveWorker::spawn(
                    *instance,
                    Arc::clone(&self.registry),
                    self.config.clone(),
                    self.clock,
                ))
            }
        };
        let mut state = self.lock_state()?;
        state.active = worker;
        state.dispatcher_closed = false;
        Ok(RuntimeCommitOutcome::Applied)
    }

    pub(super) fn recover_plan(
        &self,
        commit: &ApplyCommitRecord,
        plan: &CompiledExecutionPlan,
    ) -> Result<RuntimeCommitOutcome, HostRuntimeError> {
        let prepared = self.prepare_requirements(
            &commit.operation_id,
            &commit.plan_hash,
            plan.runtime_requirements(),
        )?;
        self.activate_prepared(prepared)
    }
}

impl Drop for ProductionRuntimeCoordinator {
    fn drop(&mut self) {
        let _ignored = self.shutdown();
    }
}
