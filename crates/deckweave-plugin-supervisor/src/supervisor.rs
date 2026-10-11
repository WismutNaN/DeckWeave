// SPDX-License-Identifier: GPL-3.0-or-later
use crate::package::IndexedPackage;
use deckweave_application::{AuthorizationError, AuthorizationStore};
use deckweave_domain::{
    capability::NodeDescriptor,
    ids::{
        ContributionId, DescriptorRevision, PackageId, PluginSessionId, PrincipalId,
        RuntimeGroupId, RuntimeLeaseId, SessionGeneration,
    },
    security::{PermissionGrant, PermissionRequest},
};
use deckweave_protocol::{
    HostProtocolSupport, NegotiationError, negotiate_hello,
    v1::{Hello, HelloAck, ProtocolEnvelope, ProtocolLimits},
};
use deckweave_registry::{CapabilityRegistry, RegistryError, SessionIdentity};
use std::{collections::VecDeque, sync::Arc};
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeKey {
    pub package_id: PackageId,
    pub artifact_sha256: String,
    pub runtime_group_id: RuntimeGroupId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeStatus {
    Indexed,
    PermissionCheck,
    Starting,
    Negotiating,
    Active,
    Stopping,
    Backoff,
    CircuitOpen,
    NeedsAttention,
    Disabled,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RuntimeLeaseReason {
    Binding,
    Discovery { scope: String },
    Probe,
    PropertyEditor,
    Subscription,
    PendingJob,
    Diagnostics,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeLease {
    pub lease_id: RuntimeLeaseId,
    pub reason: RuntimeLeaseReason,
    pub expires_at_host_ticks: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RestartPolicy {
    pub maximum_crashes: usize,
    pub crash_window_ticks: u64,
    pub base_backoff_ticks: u64,
    pub maximum_backoff_ticks: u64,
    pub startup_timeout_ticks: u64,
    pub heartbeat_timeout_ticks: u64,
    pub idle_grace_ticks: u64,
    pub stop_timeout_ticks: u64,
}

impl Default for RestartPolicy {
    fn default() -> Self {
        Self {
            maximum_crashes: 3,
            crash_window_ticks: 60_000,
            base_backoff_ticks: 250,
            maximum_backoff_ticks: 30_000,
            startup_timeout_ticks: 10_000,
            heartbeat_timeout_ticks: 15_000,
            idle_grace_ticks: 2_000,
            stop_timeout_ticks: 5_000,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SupervisorAction {
    None,
    CheckPermissions,
    StartProcess,
    RequestGracefulStop,
    ForceTerminate,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PluginSession {
    pub identity: SessionIdentity,
    pub negotiated_limits: ProtocolLimits,
    pub last_heartbeat_host_ticks: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PluginHealth {
    pub key: RuntimeKey,
    pub status: RuntimeStatus,
    pub lease_count: usize,
    pub session: Option<SessionIdentity>,
    pub crash_count_in_window: usize,
    pub retry_at_host_ticks: Option<u64>,
    pub last_error: Option<String>,
}

#[derive(Debug, Error)]
pub enum SupervisorError {
    #[error("runtime group отсутствует в indexed package")]
    UnknownRuntimeGroup,
    #[error("artifact отсутствует или не подходит текущей платформе: {0}")]
    Artifact(String),
    #[error("некорректная restart policy")]
    InvalidPolicy,
    #[error("operation недопустима в состоянии {0:?}")]
    InvalidState(RuntimeStatus),
    #[error("permission check не пройден: {0}")]
    Permission(String),
    #[error("one-time session token не совпадает")]
    InvalidSessionToken,
    #[error("hello identity не совпадает с runtime key")]
    InvalidHelloIdentity,
    #[error("сообщение принадлежит старой session generation")]
    LateSessionMessage,
    #[error("session ещё не создана")]
    MissingSession,
    #[error("lease не найдена")]
    UnknownLease,
    #[error("counter исчерпан")]
    CounterExhausted,
    #[error(transparent)]
    Negotiation(#[from] NegotiationError),
    #[error(transparent)]
    Registry(#[from] RegistryError),
}

pub struct RuntimeController {
    package: Arc<IndexedPackage>,
    key: RuntimeKey,
    policy: RestartPolicy,
    status: RuntimeStatus,
    leases: Vec<RuntimeLease>,
    grants: Vec<PermissionGrant>,
    session: Option<PluginSession>,
    one_time_token: Option<Vec<u8>>,
    next_lease: u64,
    next_generation: u64,
    transition_deadline: Option<u64>,
    retry_at: Option<u64>,
    crashes: VecDeque<u64>,
    registered: bool,
    restart_blocked: bool,
    last_error: Option<String>,
}

impl RuntimeController {
    pub fn new(
        package: Arc<IndexedPackage>,
        runtime_group_id: RuntimeGroupId,
        policy: RestartPolicy,
    ) -> Result<Self, SupervisorError> {
        validate_policy(&policy)?;
        let artifact = package
            .artifact_for_current_platform(&runtime_group_id)
            .map_err(|error| SupervisorError::Artifact(error.to_string()))?;
        Ok(Self {
            key: RuntimeKey {
                package_id: package.package_id.clone(),
                artifact_sha256: artifact.sha256.clone(),
                runtime_group_id,
            },
            package,
            policy,
            status: RuntimeStatus::Indexed,
            leases: Vec::new(),
            grants: Vec::new(),
            session: None,
            one_time_token: None,
            next_lease: 0,
            next_generation: 0,
            transition_deadline: None,
            retry_at: None,
            crashes: VecDeque::new(),
            registered: false,
            restart_blocked: false,
            last_error: None,
        })
    }

    pub fn acquire_lease(
        &mut self,
        reason: RuntimeLeaseReason,
        expires_at_host_ticks: Option<u64>,
    ) -> Result<(RuntimeLease, SupervisorAction), SupervisorError> {
        if matches!(
            self.status,
            RuntimeStatus::Disabled | RuntimeStatus::CircuitOpen
        ) {
            return Err(SupervisorError::InvalidState(self.status));
        }
        self.next_lease = self
            .next_lease
            .checked_add(1)
            .ok_or(SupervisorError::CounterExhausted)?;
        let lease = RuntimeLease {
            lease_id: RuntimeLeaseId::try_new(format!("runtime-lease-{}", self.next_lease))
                .map_err(|_| SupervisorError::CounterExhausted)?,
            reason,
            expires_at_host_ticks,
        };
        self.leases.push(lease.clone());
        if self.status == RuntimeStatus::Active {
            self.transition_deadline = None;
        }
        let action = match self.status {
            RuntimeStatus::Indexed => {
                self.status = RuntimeStatus::PermissionCheck;
                SupervisorAction::CheckPermissions
            }
            RuntimeStatus::Backoff => SupervisorAction::None,
            RuntimeStatus::NeedsAttention => SupervisorAction::CheckPermissions,
            _ => SupervisorAction::None,
        };
        Ok((lease, action))
    }

    pub fn release_lease(
        &mut self,
        lease_id: &RuntimeLeaseId,
        now_host_ticks: u64,
    ) -> Result<SupervisorAction, SupervisorError> {
        let original = self.leases.len();
        self.leases.retain(|lease| lease.lease_id != *lease_id);
        if original == self.leases.len() {
            return Err(SupervisorError::UnknownLease);
        }
        self.transition_after_last_lease(now_host_ticks)
    }

    pub fn check_permissions(
        &mut self,
        principal_id: &PrincipalId,
        authorization: &dyn AuthorizationStore,
    ) -> Result<SupervisorAction, SupervisorError> {
        if !matches!(
            self.status,
            RuntimeStatus::PermissionCheck | RuntimeStatus::NeedsAttention
        ) {
            return Err(SupervisorError::InvalidState(self.status));
        }
        let mut grants = Vec::new();
        for permission in self
            .package
            .requested_permissions
            .iter()
            .filter(|permission| {
                permission.runtime_group_ids.is_empty()
                    || permission
                        .runtime_group_ids
                        .contains(&self.key.runtime_group_id)
            })
        {
            let request = PermissionRequest {
                principal_id: principal_id.clone(),
                package_id: self.key.package_id.clone(),
                contribution_id: None::<ContributionId>,
                scope: permission.scope.clone(),
                resource_selector: permission.resource_selector.clone(),
                access_mode: permission.access_mode,
                reason: permission.reason.clone(),
                extensions: Default::default(),
            };
            match authorization.authorize(&request) {
                Ok(grant) => grants.push(grant),
                Err(AuthorizationError::Denied) if permission.optional => {}
                Err(error) => {
                    self.status = RuntimeStatus::NeedsAttention;
                    self.restart_blocked = true;
                    self.last_error = Some(error.to_string());
                    return Err(SupervisorError::Permission(error.to_string()));
                }
            }
        }
        self.grants = grants;
        self.restart_blocked = false;
        self.status = RuntimeStatus::Starting;
        self.last_error = None;
        Ok(SupervisorAction::StartProcess)
    }

    /// Rechecks current grants immediately before privileged dispatch. A
    /// revoke removes routable capabilities first and then requests bounded
    /// runtime cleanup; it never permits the pending operation.
    pub fn revalidate_permissions(
        &mut self,
        principal_id: &PrincipalId,
        authorization: &dyn AuthorizationStore,
        registry: &mut CapabilityRegistry,
        now_host_ticks: u64,
    ) -> Result<SupervisorAction, SupervisorError> {
        if self.status != RuntimeStatus::Active {
            return Err(SupervisorError::InvalidState(self.status));
        }
        let mut grants = Vec::new();
        for permission in self
            .package
            .requested_permissions
            .iter()
            .filter(|permission| {
                permission.runtime_group_ids.is_empty()
                    || permission
                        .runtime_group_ids
                        .contains(&self.key.runtime_group_id)
            })
        {
            let request = PermissionRequest {
                principal_id: principal_id.clone(),
                package_id: self.key.package_id.clone(),
                contribution_id: None::<ContributionId>,
                scope: permission.scope.clone(),
                resource_selector: permission.resource_selector.clone(),
                access_mode: permission.access_mode,
                reason: permission.reason.clone(),
                extensions: Default::default(),
            };
            match authorization.authorize(&request) {
                Ok(grant) => grants.push(grant),
                Err(AuthorizationError::Denied) if permission.optional => {}
                Err(error) => {
                    if self.registered {
                        let session = self
                            .session
                            .as_ref()
                            .ok_or(SupervisorError::MissingSession)?;
                        registry.invalidate_session(&session.identity)?;
                        self.registered = false;
                    }
                    self.grants.clear();
                    self.restart_blocked = true;
                    self.status = RuntimeStatus::Stopping;
                    self.transition_deadline = Some(
                        now_host_ticks
                            .checked_add(self.policy.stop_timeout_ticks)
                            .ok_or(SupervisorError::CounterExhausted)?,
                    );
                    self.last_error = Some(format!("permission revoked: {error}"));
                    return Ok(SupervisorAction::RequestGracefulStop);
                }
            }
        }
        self.grants = grants;
        self.restart_blocked = false;
        Ok(SupervisorAction::None)
    }

    pub fn process_started(
        &mut self,
        plugin_session_id: PluginSessionId,
        one_time_token: Vec<u8>,
        now_host_ticks: u64,
    ) -> Result<(), SupervisorError> {
        if self.status != RuntimeStatus::Starting || one_time_token.len() < 32 {
            return Err(SupervisorError::InvalidState(self.status));
        }
        self.next_generation = self
            .next_generation
            .checked_add(1)
            .ok_or(SupervisorError::CounterExhausted)?;
        let identity = SessionIdentity {
            package_id: self.key.package_id.clone(),
            runtime_group_id: self.key.runtime_group_id.clone(),
            plugin_session_id,
            generation: SessionGeneration::try_new(self.next_generation)
                .map_err(|_| SupervisorError::CounterExhausted)?,
        };
        self.session = Some(PluginSession {
            identity,
            negotiated_limits: ProtocolLimits::default(),
            last_heartbeat_host_ticks: now_host_ticks,
        });
        self.one_time_token = Some(one_time_token);
        self.status = RuntimeStatus::Negotiating;
        self.transition_deadline = Some(
            now_host_ticks
                .checked_add(self.policy.startup_timeout_ticks)
                .ok_or(SupervisorError::CounterExhausted)?,
        );
        Ok(())
    }

    pub fn accept_hello(
        &mut self,
        hello: &Hello,
        support: &HostProtocolSupport,
    ) -> Result<HelloAck, SupervisorError> {
        if self.status != RuntimeStatus::Negotiating {
            return Err(SupervisorError::InvalidState(self.status));
        }
        if hello.package_id != self.key.package_id.as_str()
            || hello.runtime_group != self.key.runtime_group_id.as_str()
        {
            return Err(SupervisorError::InvalidHelloIdentity);
        }
        if self.one_time_token.as_deref() != Some(hello.one_time_token.as_slice()) {
            return Err(SupervisorError::InvalidSessionToken);
        }
        let session = self
            .session
            .as_mut()
            .ok_or(SupervisorError::MissingSession)?;
        let ack = negotiate_hello(
            hello,
            support,
            session.identity.plugin_session_id.to_string(),
        )?;
        session.negotiated_limits = ack.limits.unwrap_or_default();
        self.one_time_token = None;
        Ok(ack)
    }

    pub fn activate_descriptors(
        &mut self,
        registry: &mut CapabilityRegistry,
        descriptor_revision: DescriptorRevision,
        nodes: Vec<NodeDescriptor>,
        now_host_ticks: u64,
    ) -> Result<(), SupervisorError> {
        if self.status != RuntimeStatus::Negotiating || self.one_time_token.is_some() {
            return Err(SupervisorError::InvalidState(self.status));
        }
        let session = self
            .session
            .as_mut()
            .ok_or(SupervisorError::MissingSession)?;
        registry.start_session(session.identity.clone())?;
        registry.register_snapshot(&session.identity, descriptor_revision, nodes)?;
        session.last_heartbeat_host_ticks = now_host_ticks;
        self.registered = true;
        self.status = RuntimeStatus::Active;
        self.transition_deadline = None;
        Ok(())
    }

    pub fn accept_envelope(
        &mut self,
        envelope: &ProtocolEnvelope,
        now_host_ticks: u64,
    ) -> Result<(), SupervisorError> {
        let session = self
            .session
            .as_mut()
            .ok_or(SupervisorError::MissingSession)?;
        if envelope.session_generation != session.identity.generation.get() {
            return Err(SupervisorError::LateSessionMessage);
        }
        session.last_heartbeat_host_ticks = now_host_ticks;
        Ok(())
    }

    pub fn tick(&mut self, now_host_ticks: u64) -> Result<SupervisorAction, SupervisorError> {
        let had_leases = !self.leases.is_empty();
        self.leases.retain(|lease| {
            !lease
                .expires_at_host_ticks
                .is_some_and(|deadline| deadline <= now_host_ticks)
        });
        if had_leases && self.leases.is_empty() {
            let action = self.transition_after_last_lease(now_host_ticks)?;
            if action != SupervisorAction::None {
                return Ok(action);
            }
        }
        match self.status {
            RuntimeStatus::Negotiating
                if self
                    .transition_deadline
                    .is_some_and(|deadline| deadline <= now_host_ticks) =>
            {
                self.status = RuntimeStatus::Stopping;
                self.transition_deadline = Some(
                    now_host_ticks
                        .checked_add(self.policy.stop_timeout_ticks)
                        .ok_or(SupervisorError::CounterExhausted)?,
                );
                self.last_error = Some("startup timeout".into());
                Ok(SupervisorAction::ForceTerminate)
            }
            RuntimeStatus::Active
                if self.session.as_ref().is_some_and(|session| {
                    now_host_ticks.saturating_sub(session.last_heartbeat_host_ticks)
                        >= self.policy.heartbeat_timeout_ticks
                }) =>
            {
                self.status = RuntimeStatus::Stopping;
                self.transition_deadline = Some(
                    now_host_ticks
                        .checked_add(self.policy.stop_timeout_ticks)
                        .ok_or(SupervisorError::CounterExhausted)?,
                );
                self.last_error = Some("heartbeat timeout".into());
                Ok(SupervisorAction::ForceTerminate)
            }
            RuntimeStatus::Active
                if self.leases.is_empty()
                    && self
                        .transition_deadline
                        .is_some_and(|deadline| deadline <= now_host_ticks) =>
            {
                self.status = RuntimeStatus::Stopping;
                self.transition_deadline = Some(
                    now_host_ticks
                        .checked_add(self.policy.stop_timeout_ticks)
                        .ok_or(SupervisorError::CounterExhausted)?,
                );
                Ok(SupervisorAction::RequestGracefulStop)
            }
            RuntimeStatus::Stopping
                if self
                    .transition_deadline
                    .is_some_and(|deadline| deadline <= now_host_ticks) =>
            {
                Ok(SupervisorAction::ForceTerminate)
            }
            RuntimeStatus::Backoff
                if self
                    .retry_at
                    .is_some_and(|deadline| deadline <= now_host_ticks) =>
            {
                self.retry_at = None;
                if self.leases.is_empty() {
                    self.status = RuntimeStatus::Indexed;
                    Ok(SupervisorAction::None)
                } else {
                    self.status = RuntimeStatus::PermissionCheck;
                    Ok(SupervisorAction::CheckPermissions)
                }
            }
            _ => Ok(SupervisorAction::None),
        }
    }

    pub fn process_exited(
        &mut self,
        registry: &mut CapabilityRegistry,
        now_host_ticks: u64,
        expected: bool,
    ) -> Result<SupervisorAction, SupervisorError> {
        if self.registered {
            if let Some(session) = &self.session {
                registry.invalidate_session(&session.identity)?;
            }
            self.registered = false;
        }
        self.session = None;
        self.one_time_token = None;
        self.transition_deadline = None;

        if self.restart_blocked {
            self.status = RuntimeStatus::NeedsAttention;
            self.retry_at = None;
            return Ok(SupervisorAction::None);
        }

        if expected {
            if self.leases.is_empty() {
                self.status = RuntimeStatus::Indexed;
                self.retry_at = None;
                return Ok(SupervisorAction::None);
            }
            self.status = RuntimeStatus::PermissionCheck;
            self.retry_at = None;
            return Ok(SupervisorAction::CheckPermissions);
        }
        if self.leases.is_empty() {
            self.status = RuntimeStatus::Indexed;
            self.retry_at = None;
            return Ok(SupervisorAction::None);
        }

        self.crashes.push_back(now_host_ticks);
        while self.crashes.front().is_some_and(|crash| {
            now_host_ticks.saturating_sub(*crash) > self.policy.crash_window_ticks
        }) {
            self.crashes.pop_front();
        }
        if self.crashes.len() >= self.policy.maximum_crashes {
            self.status = RuntimeStatus::CircuitOpen;
            self.retry_at = None;
            self.last_error = Some("crash circuit opened".into());
            return Ok(SupervisorAction::None);
        }
        let exponent = u32::try_from(self.crashes.len().saturating_sub(1)).unwrap_or(u32::MAX);
        let multiplier = 2_u64.checked_pow(exponent).unwrap_or(u64::MAX);
        let delay = self
            .policy
            .base_backoff_ticks
            .saturating_mul(multiplier)
            .min(self.policy.maximum_backoff_ticks);
        self.retry_at = Some(
            now_host_ticks
                .checked_add(delay)
                .ok_or(SupervisorError::CounterExhausted)?,
        );
        self.status = RuntimeStatus::Backoff;
        self.last_error = Some("plugin process crashed".into());
        Ok(SupervisorAction::None)
    }

    pub fn disable(
        &mut self,
        registry: &mut CapabilityRegistry,
    ) -> Result<SupervisorAction, SupervisorError> {
        self.leases.clear();
        if self.registered {
            if let Some(session) = &self.session {
                registry.invalidate_session(&session.identity)?;
            }
            self.registered = false;
        }
        let was_running = matches!(
            self.status,
            RuntimeStatus::Starting
                | RuntimeStatus::Negotiating
                | RuntimeStatus::Active
                | RuntimeStatus::Stopping
        );
        self.status = RuntimeStatus::Disabled;
        self.restart_blocked = false;
        self.retry_at = None;
        Ok(if was_running {
            SupervisorAction::ForceTerminate
        } else {
            SupervisorAction::None
        })
    }

    pub fn reset_circuit(&mut self) -> Result<(), SupervisorError> {
        if !matches!(
            self.status,
            RuntimeStatus::CircuitOpen | RuntimeStatus::Disabled
        ) {
            return Err(SupervisorError::InvalidState(self.status));
        }
        self.crashes.clear();
        self.restart_blocked = false;
        self.last_error = None;
        self.status = RuntimeStatus::Indexed;
        Ok(())
    }

    #[must_use]
    pub fn status(&self) -> RuntimeStatus {
        self.status
    }

    #[must_use]
    pub fn session(&self) -> Option<&PluginSession> {
        self.session.as_ref()
    }

    #[must_use]
    pub fn health(&self) -> PluginHealth {
        PluginHealth {
            key: self.key.clone(),
            status: self.status,
            lease_count: self.leases.len(),
            session: self
                .session
                .as_ref()
                .map(|session| session.identity.clone()),
            crash_count_in_window: self.crashes.len(),
            retry_at_host_ticks: self.retry_at,
            last_error: self.last_error.clone(),
        }
    }

    fn transition_after_last_lease(
        &mut self,
        now_host_ticks: u64,
    ) -> Result<SupervisorAction, SupervisorError> {
        if !self.leases.is_empty() {
            return Ok(SupervisorAction::None);
        }
        match self.status {
            RuntimeStatus::Active => {
                self.transition_deadline = Some(
                    now_host_ticks
                        .checked_add(self.policy.idle_grace_ticks)
                        .ok_or(SupervisorError::CounterExhausted)?,
                );
                Ok(SupervisorAction::None)
            }
            RuntimeStatus::Negotiating => {
                self.status = RuntimeStatus::Stopping;
                self.transition_deadline = Some(
                    now_host_ticks
                        .checked_add(self.policy.stop_timeout_ticks)
                        .ok_or(SupervisorError::CounterExhausted)?,
                );
                Ok(SupervisorAction::ForceTerminate)
            }
            RuntimeStatus::PermissionCheck
            | RuntimeStatus::Starting
            | RuntimeStatus::Backoff
            | RuntimeStatus::NeedsAttention => {
                self.status = RuntimeStatus::Indexed;
                self.grants.clear();
                self.retry_at = None;
                self.transition_deadline = None;
                Ok(SupervisorAction::None)
            }
            _ => Ok(SupervisorAction::None),
        }
    }
}

fn validate_policy(policy: &RestartPolicy) -> Result<(), SupervisorError> {
    if policy.maximum_crashes == 0
        || policy.crash_window_ticks == 0
        || policy.base_backoff_ticks == 0
        || policy.maximum_backoff_ticks < policy.base_backoff_ticks
        || policy.startup_timeout_ticks == 0
        || policy.heartbeat_timeout_ticks == 0
        || policy.stop_timeout_ticks == 0
    {
        return Err(SupervisorError::InvalidPolicy);
    }
    Ok(())
}
