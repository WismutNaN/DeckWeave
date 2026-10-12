// SPDX-License-Identifier: GPL-3.0-or-later
//! Ownership and lifecycle of prepared and active runtime generations.

use super::{
    config::{HostRuntimeConfig, HostRuntimeError},
    instance::{ActiveWorker, RuntimeInstance, elapsed_millis},
};
use crate::package::{IndexedPackage, PackageIndex};
use deckweave_application::AuthorizationStore;
use deckweave_automation::{BoundExecutionGraph, CompiledExecutionPlan, RuntimeRequirement};
use deckweave_domain::ids::{ContributionId, HostBootId, OperationId, PrincipalId, RuntimeGroupId};
use deckweave_host::RuntimeCommitOutcome;
use deckweave_platform::ChildProcessPolicy;
use deckweave_registry::{CapabilityRegistry, RegistrySnapshot};
use deckweave_workspace::{ApplyCommitRecord, ApplyOperationRepository};
use std::{
    sync::{Arc, Mutex},
    time::Instant,
};

mod plan;
use plan::{bind_instances, validate_distinct_runtime_groups};

pub(super) struct PreparedRuntime {
    operation_id: OperationId,
    plan_hash: String,
    instances: Vec<RuntimeInstance>,
    bound_graph: Option<BoundExecutionGraph>,
}

impl PreparedRuntime {
    pub(super) fn matches(&self, operation_id: &OperationId, plan_hash: &str) -> bool {
        self.operation_id == *operation_id && self.plan_hash == plan_hash
    }

    pub(super) fn operation_id(&self) -> &OperationId {
        &self.operation_id
    }
}

#[derive(Default)]
pub(super) struct RuntimeState {
    pub(super) next_generation: u64,
    pub(super) prepared: Option<PreparedRuntime>,
    pub(super) active: Vec<ActiveWorker>,
    pub(super) active_binding: Option<BoundExecutionGraph>,
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
            state.active_binding = None;
            (state.prepared.take(), std::mem::take(&mut state.active))
        };
        let prepared_result = prepared.map_or(Ok(()), |prepared| self.stop_prepared(prepared));
        let active_result = self.shutdown_workers(active);
        prepared_result.and(active_result)
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

    pub(super) fn prepare_plan(
        &self,
        operation_id: &OperationId,
        plan_hash: &str,
        plan: &CompiledExecutionPlan,
    ) -> Result<PreparedRuntime, HostRuntimeError> {
        let requirements = plan.runtime_requirements();
        validate_distinct_runtime_groups(requirements)?;
        let mut instances = Vec::with_capacity(requirements.len());
        for requirement in requirements {
            let launched = self.launch_requirement(requirement);
            match launched {
                Ok(instance) => instances.push(instance),
                Err(error) => {
                    let _cleanup = self.stop_instances(instances);
                    return Err(error);
                }
            }
        }
        let bound_graph = match plan.executable_graph() {
            Some(_) => match bind_instances(plan, &instances) {
                Ok(bound) => Some(bound),
                Err(error) => {
                    let _cleanup = self.stop_instances(instances);
                    return Err(error);
                }
            },
            None => None,
        };
        Ok(PreparedRuntime {
            operation_id: operation_id.clone(),
            plan_hash: plan_hash.into(),
            instances,
            bound_graph,
        })
    }

    fn launch_requirement(
        &self,
        requirement: &RuntimeRequirement,
    ) -> Result<RuntimeInstance, HostRuntimeError> {
        let (package, group, contribution) = self.resolve_requirement(requirement)?;
        RuntimeInstance::launch(
            package,
            group,
            contribution,
            self.allocate_generation()?,
            &self.config,
            Arc::clone(&self.authorization),
            self.principal.clone(),
            self.process_policy.as_ref(),
            elapsed_millis(self.clock),
        )
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
        self.stop_instances(prepared.instances)
    }

    pub(super) fn activate_prepared(
        &self,
        prepared: PreparedRuntime,
    ) -> Result<RuntimeCommitOutcome, HostRuntimeError> {
        let active = {
            let mut state = self.lock_state()?;
            state.active_binding = None;
            std::mem::take(&mut state.active)
        };
        self.shutdown_workers(active)?;
        let bound_graph = prepared.bound_graph;
        let mut instances = prepared.instances;
        let publish_result = {
            let mut registry = self
                .registry
                .lock()
                .map_err(|_| HostRuntimeError::State("registry lock is poisoned".into()))?;
            let now = elapsed_millis(self.clock);
            instances
                .iter_mut()
                .try_for_each(|instance| instance.publish(&mut registry, now))
        };
        if let Err(error) = publish_result {
            let _cleanup = self.stop_instances(instances);
            return Err(error);
        }
        let workers = instances
            .into_iter()
            .map(|instance| {
                ActiveWorker::spawn(
                    instance,
                    Arc::clone(&self.registry),
                    self.config.clone(),
                    self.clock,
                )
            })
            .collect();
        let mut state = self.lock_state()?;
        state.active = workers;
        state.active_binding = bound_graph;
        state.dispatcher_closed = false;
        Ok(RuntimeCommitOutcome::Applied)
    }

    fn stop_instances(&self, instances: Vec<RuntimeInstance>) -> Result<(), HostRuntimeError> {
        let mut first_error = None;
        for instance in instances {
            let result = self
                .registry
                .lock()
                .map_err(|_| HostRuntimeError::State("registry lock is poisoned".into()))
                .and_then(|mut registry| {
                    instance.stop(
                        &mut registry,
                        self.config.stop_timeout,
                        elapsed_millis(self.clock),
                    )
                });
            if first_error.is_none() {
                first_error = result.err();
            }
        }
        first_error.map_or(Ok(()), Err)
    }

    fn shutdown_workers(&self, workers: Vec<ActiveWorker>) -> Result<(), HostRuntimeError> {
        let mut first_error = None;
        for worker in workers {
            let result = worker.shutdown(self.config.shutdown_timeout);
            if first_error.is_none() {
                first_error = result.err();
            }
        }
        first_error.map_or(Ok(()), Err)
    }

    pub(super) fn recover_plan(
        &self,
        commit: &ApplyCommitRecord,
        plan: &CompiledExecutionPlan,
    ) -> Result<RuntimeCommitOutcome, HostRuntimeError> {
        let prepared = self.prepare_plan(&commit.operation_id, &commit.plan_hash, plan)?;
        self.activate_prepared(prepared)
    }

    pub(super) fn revalidate_prepared(
        &self,
        operation_id: &OperationId,
        plan: &CompiledExecutionPlan,
    ) -> Result<(), HostRuntimeError> {
        let mut state = self.lock_state()?;
        let prepared = state
            .prepared
            .as_mut()
            .filter(|prepared| prepared.operation_id() == operation_id)
            .ok_or_else(|| {
                HostRuntimeError::State("prepared runtime does not match operation".into())
            })?;
        for instance in &mut prepared.instances {
            instance.ensure_alive()?;
        }
        let rebound = match plan.executable_graph() {
            Some(_) => Some(bind_instances(plan, &prepared.instances)?),
            None => None,
        };
        if rebound != prepared.bound_graph {
            return Err(HostRuntimeError::Protocol(
                "prepared runtime binding changed before barrier".into(),
            ));
        }
        Ok(())
    }
}

impl Drop for ProductionRuntimeCoordinator {
    fn drop(&mut self) {
        let _ignored = self.shutdown();
    }
}
