// SPDX-License-Identifier: GPL-3.0-or-later
//! Desktop production composition. Business invariants remain in their owners.

use deckweave_application::{
    ApiAuthScope, ApplicationGateway, DenyAllAuthorizationStore, InMemoryApiAuthorizer,
};
use deckweave_automation::RestrictedFixtureCompiler;
use deckweave_domain::ids::{HostBootId, PrincipalId};
use deckweave_host::{ApplyCoordinator, ApplyPlanningService, HostRuntime};
use deckweave_platform_host::{NativeAtomicFileStore, NativeChildProcessPolicy};
use deckweave_plugin_supervisor::{
    host::{HostRuntimeConfig, ProductionRuntimeCoordinator},
    package::{PackageIndex, StagingLimits},
};
use deckweave_profiles::{JsonProfileStore, ProfileStoreLimits};
use deckweave_registry::CapabilityRegistry;
use deckweave_workspace::{ApplyOperationRepository, DraftRepository};
use std::{
    error::Error,
    path::Path,
    sync::{Arc, Mutex},
    time::{SystemTime, UNIX_EPOCH},
};

pub(crate) struct ProductionServices {
    pub(crate) gateway: ApplicationGateway,
    pub(crate) runtime: Arc<ProductionRuntimeCoordinator>,
}

pub(crate) fn compose(
    root: &Path,
    host: Arc<HostRuntime>,
) -> Result<ProductionServices, Box<dyn Error>> {
    let package_root = root.join("packages");
    std::fs::create_dir_all(&package_root)?;
    let packages = Arc::new(PackageIndex::scan(
        &package_root,
        &StagingLimits::default(),
    )?);
    let profiles = Arc::new(JsonProfileStore::new(
        root.join("profiles"),
        Arc::new(NativeAtomicFileStore),
        ProfileStoreLimits::default(),
    ));
    let authorization = Arc::new(DenyAllAuthorizationStore);
    let registry = Arc::new(Mutex::new(CapabilityRegistry::default()));
    let principal = PrincipalId::try_new("desktop-ui")?;
    let runtime = Arc::new(ProductionRuntimeCoordinator::new(
        packages,
        Arc::clone(&profiles) as Arc<dyn DraftRepository>,
        Arc::clone(&profiles) as Arc<dyn ApplyOperationRepository>,
        authorization,
        registry,
        Arc::new(NativeChildProcessPolicy),
        principal.clone(),
        host_boot_id()?,
        HostRuntimeConfig::default(),
    ));
    let planning = ApplyPlanningService::new(
        Arc::clone(&profiles) as Arc<dyn DraftRepository>,
        Arc::new(RestrictedFixtureCompiler),
        Arc::clone(&runtime) as Arc<dyn deckweave_host::PlanningEvidenceSource>,
        Arc::clone(&runtime) as Arc<dyn deckweave_host::RuntimeActivation>,
        Arc::clone(&runtime) as Arc<dyn deckweave_host::PlanningClock>,
        30_000,
    )?;
    let apply = Arc::new(ApplyCoordinator::new(
        planning,
        Arc::clone(&profiles) as Arc<dyn DraftRepository>,
        Arc::clone(&profiles) as Arc<dyn ApplyOperationRepository>,
        Arc::clone(&runtime) as Arc<dyn deckweave_host::RuntimeCutover>,
        64,
    )?);
    apply.recover(256)?;

    let authorizer = Arc::new(InMemoryApiAuthorizer::new());
    authorizer.grant(
        principal,
        [
            ApiAuthScope::Read,
            ApiAuthScope::Draft,
            ApiAuthScope::Simulation,
            ApiAuthScope::Apply,
        ],
    );
    let gateway = ApplicationGateway::new(
        host as Arc<dyn deckweave_application::HostStatusSource>,
        profiles as Arc<dyn DraftRepository>,
        authorizer,
        apply,
    );
    Ok(ProductionServices { gateway, runtime })
}

fn host_boot_id() -> Result<HostBootId, Box<dyn Error>> {
    let started = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
    Ok(HostBootId::try_new(format!(
        "boot-{}-{started}",
        std::process::id()
    ))?)
}
