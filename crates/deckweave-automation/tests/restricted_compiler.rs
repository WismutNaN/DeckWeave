// SPDX-License-Identifier: GPL-3.0-or-later
use deckweave_automation::{
    CompileFence, PlanCompileError, PlanCompiler, RecoveryCompileInput, RestrictedFixtureCompiler,
    VIRTUAL_FIXTURE_CONTRIBUTION_ID, VIRTUAL_FIXTURE_PACKAGE_ID, VIRTUAL_FIXTURE_RUNTIME_GROUP_ID,
};
use deckweave_domain::ids::{
    DraftId, DraftVersion, GrantRevision, PackageId, RegistryRevision, WorkspaceId,
    WorkspaceRevisionId,
};
use deckweave_workspace::{PackageLockEntry, WorkspaceCandidate, WorkspaceDraft};
use std::{collections::BTreeMap, error::Error};

fn candidate(document: serde_json::Value) -> Result<WorkspaceCandidate, Box<dyn Error>> {
    WorkspaceCandidate::from_draft(WorkspaceDraft {
        schema_version: 1,
        draft_id: DraftId::try_new("compiler-draft")?,
        workspace_id: WorkspaceId::try_new("compiler-workspace")?,
        base_revision_id: None,
        version: DraftVersion::try_new(1)?,
        package_lock: vec![],
        document,
        assets: vec![],
        private_section: None,
        extensions: BTreeMap::new(),
        unknown_fields: BTreeMap::new(),
    })
    .map_err(Into::into)
}

fn fence() -> Result<CompileFence, Box<dyn Error>> {
    Ok(CompileFence {
        registry_revision: RegistryRevision::try_new(1)?,
        grant_revision: GrantRevision::try_new(1)?,
    })
}

#[test]
fn only_empty_and_exact_virtual_echo_fixtures_compile() -> Result<(), Box<dyn Error>> {
    let compiler = RestrictedFixtureCompiler;
    let empty = compiler.compile(&candidate(serde_json::json!({}))?, &fence()?)?;
    assert!(empty.runtime_requirements().is_empty());

    let mut virtual_draft = candidate(serde_json::json!({
        "fixture": {"kind": "virtual_echo"}
    }))?;
    let package = PackageLockEntry {
        package_id: PackageId::try_new(VIRTUAL_FIXTURE_PACKAGE_ID)?,
        version: "0.1.0".into(),
        artifact_sha256: "a".repeat(64),
        source: None,
        extensions: BTreeMap::new(),
    };
    let mut source = WorkspaceDraft {
        schema_version: 1,
        draft_id: virtual_draft.draft_id().clone(),
        workspace_id: virtual_draft.workspace_id().clone(),
        base_revision_id: None,
        version: virtual_draft.version(),
        package_lock: vec![package],
        document: virtual_draft.document().clone(),
        assets: vec![],
        private_section: None,
        extensions: BTreeMap::new(),
        unknown_fields: BTreeMap::new(),
    };
    virtual_draft = WorkspaceCandidate::from_draft(source.clone())?;
    let compiled = compiler.compile(&virtual_draft, &fence()?)?;
    let revision = virtual_draft.to_revision(WorkspaceRevisionId::try_new("compiler-revision")?);
    let recovered = compiler.compile_recovery(
        RecoveryCompileInput {
            revision: &revision,
            candidate_sha256: virtual_draft.content_sha256(),
            package_lock_sha256: virtual_draft.package_lock_sha256(),
        },
        &fence()?,
    )?;
    assert_eq!(recovered, compiled);
    assert_eq!(compiled.runtime_requirements().len(), 1);
    let requirement = &compiled.runtime_requirements()[0];
    assert_eq!(requirement.package_id.as_str(), VIRTUAL_FIXTURE_PACKAGE_ID);
    assert_eq!(
        requirement.runtime_group_id.as_str(),
        VIRTUAL_FIXTURE_RUNTIME_GROUP_ID
    );
    assert_eq!(
        requirement.contribution_id.as_str(),
        VIRTUAL_FIXTURE_CONTRIBUTION_ID
    );
    assert!(!compiled.requires_disruption());
    assert_eq!(compiled.clone(), compiled.seal()?.verify()?);

    source.document = serde_json::json!({
        "fixture": {"kind": "virtual_disruptive_echo"}
    });
    let disruptive = WorkspaceCandidate::from_draft(source.clone())?;
    assert!(
        compiler
            .compile(&disruptive, &fence()?)?
            .requires_disruption()
    );

    source.document = serde_json::json!({
        "fixture": {"kind": "virtual_echo"},
        "arbitraryGraph": []
    });
    let unsupported = WorkspaceCandidate::from_draft(source)?;
    assert_eq!(
        compiler.compile(&unsupported, &fence()?),
        Err(PlanCompileError::UnsupportedGraph)
    );
    assert_eq!(
        compiler.compile(&candidate(serde_json::json!({"bindings": []}))?, &fence()?),
        Err(PlanCompileError::UnsupportedGraph)
    );
    Ok(())
}
