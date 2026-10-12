// SPDX-License-Identifier: GPL-3.0-or-later
//! Executable port conformance: layout/windows and hub/MQTT share the same client.
use deckweave_application::plugin_services::*;
use deckweave_domain::{ids::*, schema::SchemaRef};
use std::{
    collections::{BTreeMap, BTreeSet},
    error::Error,
    sync::Mutex,
};

struct Job {
    owner: ServiceOwner,
    state: ServiceJobState,
}
struct Lease {
    owner: ServiceOwner,
    generation: SessionGeneration,
    consumed: bool,
}
struct State {
    jobs: BTreeMap<String, Job>,
    leases: BTreeMap<String, Lease>,
    serial: u64,
    generation: SessionGeneration,
    grants: GrantRevision,
}
struct Broker {
    caller: PluginInstanceId,
    selected: BTreeMap<NamespacedKind, PluginInstanceId>,
    operations: BTreeSet<NamespacedKind>,
    schema: SchemaRef,
    state: Mutex<State>,
}
impl Broker {
    fn state(&self) -> Result<std::sync::MutexGuard<'_, State>, ServiceCallError> {
        self.state.lock().map_err(|_| ServiceCallError::Capacity)
    }
    fn owner(&self, owner: &ServiceOwner, state: &State) -> Result<(), ServiceCallError> {
        if owner.caller_instance != self.caller
            || owner.activation_epoch.get() != 1
            || owner.caller_generation.get() != 1
        {
            return Err(ServiceCallError::Stale);
        }
        if owner.grant_revision != state.grants {
            return Err(ServiceCallError::Denied);
        }
        Ok(())
    }
    fn provider_lost(&self) -> Result<(), Box<dyn Error>> {
        let mut state = self.state()?;
        state.generation = SessionGeneration::try_new(state.generation.get() + 1)?;
        for job in state.jobs.values_mut() {
            if job.state == ServiceJobState::Pending {
                job.state = ServiceJobState::UnknownOutcome;
            }
        }
        Ok(())
    }
    fn complete(
        &self,
        id: &str,
        generation: SessionGeneration,
        provider: &PluginInstanceId,
    ) -> Result<(), ServiceCallError> {
        let mut state = self.state()?;
        if state.generation != generation {
            return Err(ServiceCallError::Stale);
        }
        let job = state.jobs.get_mut(id).ok_or(ServiceCallError::Stale)?;
        if job.state != ServiceJobState::Pending {
            return Err(ServiceCallError::Stale);
        }
        job.state = ServiceJobState::Completed {
            schema: self.schema.clone(),
            value: serde_json::json!({"provider":provider}),
        };
        Ok(())
    }
}
impl PluginServiceClient for Broker {
    fn start(
        &self,
        role: &NamespacedKind,
        operation: &NamespacedKind,
        schema: &SchemaRef,
        input: serde_json::Value,
        context: &DelegationContext,
    ) -> Result<String, ServiceCallError> {
        let mut state = self.state()?;
        self.owner(&context.owner, &state)?;
        let provider = self
            .selected
            .get(role)
            .ok_or(ServiceCallError::Unresolved)?;
        context.child(
            provider.clone(),
            operation,
            &self.operations,
            context.deadline_host_ticks,
            10,
        )?;
        if schema != &self.schema || !deckweave_domain::schema::bounded_json(&input) {
            return Err(ServiceCallError::Invalid);
        }
        if state.jobs.len() >= 8 {
            return Err(ServiceCallError::Capacity);
        }
        state.serial += 1;
        let id = format!("job-{}", state.serial);
        state.jobs.insert(
            id.clone(),
            Job {
                owner: context.owner.clone(),
                state: ServiceJobState::Pending,
            },
        );
        Ok(id)
    }
    fn poll_job(
        &self,
        id: &str,
        owner: &ServiceOwner,
    ) -> Result<ServiceJobState, ServiceCallError> {
        let state = self.state()?;
        self.owner(owner, &state)?;
        let job = state.jobs.get(id).ok_or(ServiceCallError::Stale)?;
        if job.owner != *owner {
            return Err(ServiceCallError::Denied);
        }
        Ok(job.state.clone())
    }
    fn cancel_owner(&self, owner: &ServiceOwner) -> Result<(), ServiceCallError> {
        if owner.caller_instance != self.caller {
            return Err(ServiceCallError::Denied);
        }
        let mut state = self.state()?;
        for job in state.jobs.values_mut().filter(|j| j.owner == *owner) {
            if job.state == ServiceJobState::Pending {
                job.state = ServiceJobState::Cancelled;
            }
        }
        state.leases.retain(|_, lease| lease.owner != *owner);
        Ok(())
    }
    fn release_job(&self, id: &str, owner: &ServiceOwner) -> Result<(), ServiceCallError> {
        let mut state = self.state()?;
        if let Some(job) = state.jobs.get(id) {
            if job.owner != *owner {
                return Err(ServiceCallError::Denied);
            }
            if job.state == ServiceJobState::Pending {
                return Err(ServiceCallError::Invalid);
            }
        }
        state.jobs.remove(id);
        Ok(())
    }
    fn subscribe(
        &self,
        role: &NamespacedKind,
        owner: &ServiceOwner,
        maximum_items: usize,
    ) -> Result<String, ServiceCallError> {
        let mut state = self.state()?;
        self.owner(owner, &state)?;
        if !self.selected.contains_key(role) {
            return Err(ServiceCallError::Unresolved);
        }
        if maximum_items == 0 || maximum_items > 100 || state.leases.len() >= 8 {
            return Err(ServiceCallError::Capacity);
        }
        state.serial += 1;
        let id = format!("lease-{}", state.serial);
        let generation = state.generation;
        state.leases.insert(
            id.clone(),
            Lease {
                owner: owner.clone(),
                generation,
                consumed: false,
            },
        );
        Ok(id)
    }
    fn unsubscribe(&self, id: &str, owner: &ServiceOwner) -> Result<(), ServiceCallError> {
        let mut state = self.state()?;
        if state.leases.get(id).is_some_and(|l| l.owner != *owner) {
            return Err(ServiceCallError::Denied);
        }
        state.leases.remove(id);
        Ok(())
    }
    fn poll_subscription(
        &self,
        id: &str,
        owner: &ServiceOwner,
    ) -> Result<Option<ServiceUpdate>, ServiceCallError> {
        let mut state = self.state()?;
        self.owner(owner, &state)?;
        let generation = state.generation;
        let lease = state.leases.get_mut(id).ok_or(ServiceCallError::Stale)?;
        if lease.owner != *owner {
            return Err(ServiceCallError::Denied);
        }
        if lease.generation != generation {
            return Err(ServiceCallError::Stale);
        }
        if lease.consumed {
            return Ok(None);
        }
        lease.consumed = true;
        Ok(Some(ServiceUpdate {
            cursor: SubscriptionCursor {
                provider_generation: generation,
                revision: 1,
                sequence: 0,
            },
            kind: ServiceUpdateKind::Snapshot,
            schema: self.schema.clone(),
            value: serde_json::json!([]),
        }))
    }
}

#[test]
fn selected_services_return_jobs_and_subscriptions_with_owner_cleanup_and_restart_fences()
-> Result<(), Box<dyn Error>> {
    for (caller, role, provider, operation) in [
        (
            "layout",
            "org.fixture.windows",
            "windows-chosen",
            "org.fixture.restore",
        ),
        (
            "hub",
            "org.fixture.mqtt",
            "mqtt-chosen",
            "org.fixture.publish",
        ),
    ] {
        let caller = PluginInstanceId::try_new(caller)?;
        let role = NamespacedKind::try_new(role)?;
        let provider = PluginInstanceId::try_new(provider)?;
        let operation = NamespacedKind::try_new(operation)?;
        let broker = Broker {
            caller: caller.clone(),
            selected: BTreeMap::from([(role.clone(), provider.clone())]),
            operations: BTreeSet::from([operation.clone()]),
            schema: serde_json::from_value(
                serde_json::json!({"id":"org.fixture.record","version":{"major":1,"minor":0}}),
            )?,
            state: Mutex::new(State {
                jobs: BTreeMap::new(),
                leases: BTreeMap::new(),
                serial: 0,
                generation: SessionGeneration::try_new(1)?,
                grants: GrantRevision::try_new(1)?,
            }),
        };
        let owner = ServiceOwner {
            execution_id: ExecutionId::try_new("a")?,
            activation_epoch: ActivationEpoch::try_new(1)?,
            caller_instance: caller.clone(),
            caller_generation: SessionGeneration::try_new(1)?,
            grant_revision: GrantRevision::try_new(1)?,
        };
        let context = DelegationContext {
            owner: owner.clone(),
            deadline_host_ticks: 100,
            call_path: vec![caller],
            allowed_operations: broker.operations.clone(),
        };
        let first = broker.start(
            &role,
            &operation,
            &broker.schema,
            serde_json::json!({}),
            &context,
        )?;
        let subscription = broker.subscribe(&role, &owner, 16)?;
        let update = broker
            .poll_subscription(&subscription, &owner)?
            .ok_or("snapshot")?;
        assert_eq!(update.kind, ServiceUpdateKind::Snapshot);
        assert!(broker.poll_subscription(&subscription, &owner)?.is_none());
        let second_context = DelegationContext {
            owner: ServiceOwner {
                execution_id: ExecutionId::try_new("b")?,
                ..owner.clone()
            },
            ..context.clone()
        };
        let second = broker.start(
            &role,
            &operation,
            &broker.schema,
            serde_json::json!({}),
            &second_context,
        )?;
        assert_eq!(
            broker.poll_job(&second, &owner),
            Err(ServiceCallError::Denied)
        );
        broker.cancel_owner(&owner)?;
        assert_eq!(broker.poll_job(&first, &owner)?, ServiceJobState::Cancelled);
        assert_eq!(
            broker.poll_job(&second, &second_context.owner)?,
            ServiceJobState::Pending
        );
        assert!(broker.poll_subscription(&subscription, &owner).is_err());
        broker.complete(&second, update.cursor.provider_generation, &provider)?;
        assert_eq!(
            broker.poll_job(&second, &second_context.owner)?,
            ServiceJobState::Completed {
                schema: broker.schema.clone(),
                value: serde_json::json!({"provider":provider})
            }
        );
        let third = broker.start(
            &role,
            &operation,
            &broker.schema,
            serde_json::json!({}),
            &second_context,
        )?;
        let subscription = broker.subscribe(&role, &second_context.owner, 16)?;
        broker.provider_lost()?;
        assert_eq!(
            broker.poll_job(&third, &second_context.owner)?,
            ServiceJobState::UnknownOutcome
        );
        assert_eq!(
            broker.complete(&third, update.cursor.provider_generation, &provider),
            Err(ServiceCallError::Stale)
        );
        assert!(matches!(
            broker.poll_subscription(&subscription, &second_context.owner),
            Err(ServiceCallError::Stale)
        ));
        broker.state()?.grants = GrantRevision::try_new(2)?;
        assert_eq!(
            broker.start(
                &role,
                &operation,
                &broker.schema,
                serde_json::json!({}),
                &second_context
            ),
            Err(ServiceCallError::Denied)
        );
        broker.unsubscribe(&subscription, &second_context.owner)?;
        broker.cancel_owner(&second_context.owner)?;
        broker.release_job(&first, &owner)?;
        broker.release_job(&second, &second_context.owner)?;
        broker.release_job(&third, &second_context.owner)?;
        assert!(broker.state()?.jobs.is_empty());
        assert!(broker.state()?.leases.is_empty());
    }
    Ok(())
}
