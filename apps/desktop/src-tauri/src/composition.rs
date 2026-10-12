// SPDX-License-Identifier: GPL-3.0-or-later
//! Desktop production composition. Business invariants remain in their owners.

use deckweave_application::{
    ApiAuthScope, ApplicationGateway, InMemoryApiAuthorizer, LocalePreferenceStore,
    ScopedAuthorizationStore, locale::LocalePreferencesService,
};
use deckweave_domain::ids::{HostBootId, PrincipalId};
use deckweave_host::{ApplyCoordinator, ApplyPlanningService, HostRuntime};
use deckweave_platform_host::{
    JsonLocalePreferenceStore, NativeAtomicFileStore, NativeChildProcessPolicy,
};
use deckweave_plugin_supervisor::{
    host::{HostRuntimeConfig, ProductionRuntimeCoordinator},
    package::{PackageGraphCompiler, PackageIndex, StagingLimits},
};
use deckweave_profiles::{JsonProfileStore, ProfileStoreLimits};
use deckweave_registry::CapabilityRegistry;
use deckweave_workspace::{ApplyOperationRepository, CommandClaimRepository, DraftRepository};
use std::{
    error::Error,
    path::Path,
    sync::{Arc, Mutex},
    time::{SystemTime, UNIX_EPOCH},
};

pub(crate) struct ProductionServices {
    pub(crate) gateway: Arc<ApplicationGateway>,
    pub(crate) runtime: Arc<ProductionRuntimeCoordinator>,
    pub(crate) locale: Arc<LocalePreferencesService>,
    pub(crate) editor: Arc<deckweave_application::editor::EditorWorkspaceService>,
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
    let authorization = Arc::new(ScopedAuthorizationStore::default());
    let registry = Arc::new(Mutex::new(CapabilityRegistry::default()));
    let principal = PrincipalId::try_new("desktop-ui")?;
    let locale_store = Arc::new(JsonLocalePreferenceStore::new(
        root.join("host/locale.json"),
        Arc::new(NativeAtomicFileStore),
    ));
    let runtime_config = HostRuntimeConfig {
        locale: locale_store.load()?,
        ..Default::default()
    };
    let compiler = PackageGraphCompiler::new(Arc::clone(&packages))
        .with_authorization(authorization.clone(), principal.clone());
    let runtime = Arc::new(ProductionRuntimeCoordinator::new(
        packages.clone(),
        Arc::clone(&profiles) as Arc<dyn ApplyOperationRepository>,
        authorization,
        registry,
        Arc::new(NativeChildProcessPolicy),
        principal.clone(),
        host_boot_id()?,
        runtime_config,
    ));
    let planning = ApplyPlanningService::new(
        Arc::clone(&profiles) as Arc<dyn DraftRepository>,
        Arc::new(compiler),
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
    profiles.archive_terminal_operations()?;
    profiles.compact_command_claims()?;

    let authorizer = Arc::new(InMemoryApiAuthorizer::new());
    authorizer.grant(
        principal,
        [
            ApiAuthScope::Read,
            ApiAuthScope::Draft,
            ApiAuthScope::Simulation,
            ApiAuthScope::Apply,
            ApiAuthScope::Preferences,
        ],
    );
    let locale = LocalePreferencesService::new(locale_store, runtime.clone(), authorizer.clone());
    let editor = Arc::new(deckweave_application::editor::EditorWorkspaceService::new(
        profiles.clone(),
        authorizer.clone(),
        crate::editor_seed::initial(&profiles)?,
    ));
    let editor_source = Arc::new(deckweave_plugin_supervisor::host::ProductionEditorSource {
        runtime: runtime.clone(),
        packages: packages.clone(),
        profiles: profiles.clone(),
    });
    let gateway = ApplicationGateway::new(
        host as Arc<dyn deckweave_application::HostStatusSource>,
        Arc::clone(&profiles) as Arc<dyn DraftRepository>,
        profiles as Arc<dyn CommandClaimRepository>,
        authorizer,
        apply,
    )
    .with_runtime_diagnostics(runtime.clone())
    .with_plugin_catalog(packages)
    .with_editor_source(editor_source);
    Ok(ProductionServices {
        gateway: Arc::new(gateway),
        runtime,
        locale: Arc::new(locale),
        editor,
    })
}

fn host_boot_id() -> Result<HostBootId, Box<dyn Error>> {
    let started = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
    Ok(HostBootId::try_new(format!(
        "boot-{}-{started}",
        std::process::id()
    ))?)
}
