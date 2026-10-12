// SPDX-License-Identifier: GPL-3.0-or-later
//! Ownership and lifecycle of prepared and active runtime generations.

use super::{
    config::{HostRuntimeConfig, HostRuntimeError},
    instance::{ActiveWorker, RuntimeInstance, elapsed_millis},
};
use crate::package::{IndexedPackage, PackageIndex};
use deckweave_application::AuthorizationStore;
use deckweave_domain::ids::{ContributionId, HostBootId, OperationId, PrincipalId, RuntimeGroupId};
use deckweave_host::{ApplyPlan, RuntimeCommitOutcome};
use deckweave_platform::ChildProcessPolicy;
use deckweave_registry::{CapabilityRegistry, RegistrySnapshot};
use deckweave_workspace::{
    ApplyCommitRecord, ApplyOperationRepository, DraftRepository, PackageLockEntry,
    WorkspaceCandidate, WorkspaceRevision,
};
use sha2::{Digest, Sha256};
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
    pub(super) drafts: Arc<dyn DraftRepository>,
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
        drafts: Arc<dyn DraftRepository>,
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
            drafts,
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

    pub(super) fn prepare_virtual(
        &self,
        operation_id: &OperationId,
        plan: &ApplyPlan,
    ) -> Result<PreparedRuntime, HostRuntimeError> {
        let candidate = self.drafts.get_candidate(plan.token().draft_id())?;
        let (package, group, contribution) = self.resolve_candidate(&candidate)?;
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
            plan_hash: plan.token().plan_hash().into(),
            instance: Box::new(instance),
        })
    }

    fn resolve_candidate(
        &self,
        candidate: &WorkspaceCandidate,
    ) -> Result<(Arc<IndexedPackage>, RuntimeGroupId, ContributionId), HostRuntimeError> {
        if candidate.package_lock().len() != 1 {
            return Err(HostRuntimeError::Package(
                "virtual fixture requires one package lock".into(),
            ));
        }
        self.resolve_package(&candidate.package_lock()[0])
    }

    fn resolve_package(
        &self,
        lock: &PackageLockEntry,
    ) -> Result<(Arc<IndexedPackage>, RuntimeGroupId, ContributionId), HostRuntimeError> {
        let package = self
            .packages
            .resolve_artifact(&lock.package_id, &lock.version, &lock.artifact_sha256)
            .cloned()
            .map(Arc::new)
            .ok_or_else(|| HostRuntimeError::Package(lock.package_id.to_string()))?;
        let mut groups = package.runtime_groups.keys();
        let group = groups
            .next()
            .cloned()
            .ok_or_else(|| HostRuntimeError::Package("package has no runtime group".into()))?;
        if groups.next().is_some() {
            return Err(HostRuntimeError::Package(
                "restricted virtual package must have one runtime group".into(),
            ));
        }
        let mut contributions = package
            .contributions
            .values()
            .filter(|contribution| contribution.runtime_group_id == group);
        let contribution = contributions
            .next()
            .map(|value| value.contribution_id.clone())
            .ok_or_else(|| HostRuntimeError::Package("runtime group has no contribution".into()))?;
        if contributions.next().is_some() {
            return Err(HostRuntimeError::Package(
                "restricted virtual runtime must have one contribution".into(),
            ));
        }
        Ok((package, group, contribution))
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

    pub(super) fn recover_revision(
        &self,
        commit: &ApplyCommitRecord,
        revision: &WorkspaceRevision,
    ) -> Result<RuntimeCommitOutcome, HostRuntimeError> {
        if revision.workspace_id != commit.candidate.workspace_id
            || revision.revision_id != commit.candidate.revision_id
            || digest_json(&revision.package_lock)? != commit.package_lock_sha256
        {
            return Err(HostRuntimeError::State(
                "committed revision does not match recovery record".into(),
            ));
        }
        let prepared = if revision
            .document
            .as_object()
            .is_some_and(serde_json::Map::is_empty)
            && revision.package_lock.is_empty()
        {
            PreparedRuntime::Empty {
                operation_id: commit.operation_id.clone(),
                plan_hash: commit.plan_hash.clone(),
            }
        } else {
            let expected = serde_json::json!({"fixture": {"kind": "virtual_echo"}});
            if revision.document != expected || revision.package_lock.len() != 1 {
                return Err(HostRuntimeError::Package(
                    "committed graph is unsupported by fixture runtime".into(),
                ));
            }
            let (package, group, contribution) = self.resolve_package(&revision.package_lock[0])?;
            PreparedRuntime::Virtual {
                operation_id: commit.operation_id.clone(),
                plan_hash: commit.plan_hash.clone(),
                instance: Box::new(RuntimeInstance::launch(
                    package,
                    group,
                    contribution,
                    self.allocate_generation()?,
                    &self.config,
                    Arc::clone(&self.authorization),
                    self.principal.clone(),
                    self.process_policy.as_ref(),
                    elapsed_millis(self.clock),
                )?),
            }
        };
        self.activate_prepared(prepared)
    }
}

impl Drop for ProductionRuntimeCoordinator {
    fn drop(&mut self) {
        let _ignored = self.shutdown();
    }
}

fn digest_json(value: &impl serde::Serialize) -> Result<String, HostRuntimeError> {
    serde_json::to_vec(value)
        .map(|bytes| format!("{:x}", Sha256::digest(bytes)))
        .map_err(|error| HostRuntimeError::State(error.to_string()))
}
