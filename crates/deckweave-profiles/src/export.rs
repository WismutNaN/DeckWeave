// SPDX-License-Identifier: GPL-3.0-or-later
use super::*;
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use deckweave_domain::ids::{CredentialRef, DraftId, DraftVersion};
use std::collections::{BTreeMap, BTreeSet};

const EXPORT_FORMAT_VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct RevisionExportSelection {
    pub workspace_id: WorkspaceId,
    pub revision_id: WorkspaceRevisionId,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExportSelection {
    pub revisions: Vec<RevisionExportSelection>,
    pub include_assets: bool,
    pub include_private_sections: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportRevisionSummary {
    pub workspace_id: WorkspaceId,
    pub source_revision_id: WorkspaceRevisionId,
    pub has_private_section: bool,
    pub package_count: usize,
    pub asset_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportInspection {
    pub revisions: Vec<ImportRevisionSummary>,
    pub included_assets: Vec<AssetId>,
    pub missing_assets: Vec<AssetId>,
    pub unresolved_credential_refs: Vec<CredentialRef>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ImportResult {
    pub draft: LoadedDraft,
    pub installed_assets: Vec<AssetId>,
    pub unresolved_credential_refs: Vec<CredentialRef>,
}

#[derive(Debug, Error)]
pub enum ExportError {
    #[error(transparent)]
    Store(#[from] ProfileStoreError),
    #[error(transparent)]
    Draft(#[from] DraftMutationError),
    #[error("export selection is empty")]
    EmptySelection,
    #[error("export bundle exceeds configured size limit")]
    TooLarge,
    #[error("export bundle is invalid: {0}")]
    InvalidBundle(String),
    #[error("selected exported revision was not found")]
    RevisionNotFound,
    #[error("import is missing required assets")]
    MissingAssets,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PortableRevision {
    schema_version: u32,
    workspace_id: WorkspaceId,
    source_revision_id: WorkspaceRevisionId,
    package_lock: Vec<PackageLockEntry>,
    document: serde_json::Value,
    #[serde(default)]
    assets: Vec<AssetReference>,
    private_section: Option<serde_json::Value>,
    #[serde(default)]
    extensions: BTreeMap<String, serde_json::Value>,
    #[serde(flatten)]
    unknown_fields: BTreeMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PortableAsset {
    descriptor: AssetDescriptor,
    content_base64: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ExportBundle {
    format_version: u32,
    revisions: Vec<PortableRevision>,
    #[serde(default)]
    assets: Vec<PortableAsset>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct StoredExportBundle {
    storage_version: u32,
    checksum_sha256: String,
    bundle: ExportBundle,
}

struct ValidatedBundle {
    bundle: ExportBundle,
    asset_bytes: BTreeMap<AssetId, Vec<u8>>,
}

impl JsonProfileStore {
    pub fn export_revisions(&self, selection: &ExportSelection) -> Result<Vec<u8>, ExportError> {
        if selection.revisions.is_empty() {
            return Err(ExportError::EmptySelection);
        }
        let unique: BTreeSet<_> = selection.revisions.iter().cloned().collect();
        if unique.len() != selection.revisions.len() {
            return Err(ExportError::InvalidBundle(
                "export selection contains duplicate revisions".into(),
            ));
        }

        let mut revisions = Vec::with_capacity(selection.revisions.len());
        let mut referenced_assets = BTreeMap::<AssetId, AssetReference>::new();
        for selected in &selection.revisions {
            let revision = self.load_revision(&selected.workspace_id, &selected.revision_id)?;
            for asset in &revision.assets {
                match referenced_assets.get(&asset.id) {
                    Some(existing) if existing != asset => {
                        return Err(ExportError::InvalidBundle(
                            "one asset ID has conflicting metadata".into(),
                        ));
                    }
                    Some(_) => {}
                    None => {
                        referenced_assets.insert(asset.id.clone(), asset.clone());
                    }
                }
            }
            revisions.push(PortableRevision {
                schema_version: revision.schema_version,
                workspace_id: revision.workspace_id,
                source_revision_id: revision.revision_id,
                package_lock: revision.package_lock,
                document: revision.document,
                assets: revision.assets,
                private_section: selection
                    .include_private_sections
                    .then_some(revision.private_section)
                    .flatten(),
                extensions: revision.extensions,
                unknown_fields: revision.unknown_fields,
            });
        }

        let mut assets = Vec::new();
        if selection.include_assets {
            for reference in referenced_assets.values() {
                let (descriptor, bytes) = self.read_asset(&reference.id)?;
                if descriptor.sha256 != reference.sha256.to_ascii_lowercase()
                    || descriptor.media_type != reference.media_type
                    || descriptor.size_bytes != reference.size_bytes
                {
                    return Err(ExportError::InvalidBundle(
                        "asset reference does not match stored descriptor".into(),
                    ));
                }
                assets.push(PortableAsset {
                    descriptor,
                    content_base64: BASE64.encode(bytes),
                });
            }
        }
        let bundle = ExportBundle {
            format_version: EXPORT_FORMAT_VERSION,
            revisions,
            assets,
        };
        let checksum_sha256 = digest(&canonical_json(&bundle).map_err(ProfileStoreError::from)?);
        let encoded = canonical_json(&StoredExportBundle {
            storage_version: STORAGE_VERSION,
            checksum_sha256,
            bundle,
        })
        .map_err(ProfileStoreError::from)?;
        if u64::try_from(encoded.len()).unwrap_or(u64::MAX) > self.limits.maximum_export_bytes {
            return Err(ExportError::TooLarge);
        }
        Ok(encoded)
    }

    pub fn inspect_import(&self, bytes: &[u8]) -> Result<ImportInspection, ExportError> {
        let validated = self.decode_export(bytes)?;
        self.inspect_validated(&validated)
    }

    pub fn import_revision_as_draft(
        &self,
        bytes: &[u8],
        workspace_id: &WorkspaceId,
        source_revision_id: &WorkspaceRevisionId,
        draft_id: DraftId,
        allow_missing_assets: bool,
    ) -> Result<ImportResult, ExportError> {
        let validated = self.decode_export(bytes)?;
        let inspection = self.inspect_validated(&validated)?;
        if !allow_missing_assets && !inspection.missing_assets.is_empty() {
            return Err(ExportError::MissingAssets);
        }
        let revision = validated
            .bundle
            .revisions
            .iter()
            .find(|revision| {
                revision.workspace_id == *workspace_id
                    && revision.source_revision_id == *source_revision_id
            })
            .ok_or(ExportError::RevisionNotFound)?;

        let referenced_ids: BTreeSet<_> = revision
            .assets
            .iter()
            .map(|asset| asset.id.clone())
            .collect();
        let mut installed_assets = Vec::new();
        for asset_id in referenced_ids {
            let Some(bytes) = validated.asset_bytes.get(&asset_id) else {
                continue;
            };
            let reference = revision
                .assets
                .iter()
                .find(|reference| reference.id == asset_id)
                .ok_or_else(|| ExportError::InvalidBundle("asset reference disappeared".into()))?;
            let descriptor = self.put_asset(bytes, &reference.media_type)?;
            if descriptor.id != asset_id {
                return Err(ExportError::InvalidBundle(
                    "installed asset identity changed".into(),
                ));
            }
            installed_assets.push(asset_id);
        }

        let loaded = self.create_draft(&WorkspaceDraft {
            schema_version: revision.schema_version,
            draft_id,
            workspace_id: revision.workspace_id.clone(),
            base_revision_id: None,
            version: DraftVersion::try_new(1).map_err(|error| {
                ProfileStoreError::InvalidDocument(format!(
                    "invalid initial draft version: {error}"
                ))
            })?,
            package_lock: revision.package_lock.clone(),
            document: revision.document.clone(),
            assets: revision.assets.clone(),
            private_section: revision.private_section.clone(),
            extensions: revision.extensions.clone(),
            unknown_fields: revision.unknown_fields.clone(),
        })?;
        Ok(ImportResult {
            draft: loaded,
            installed_assets,
            unresolved_credential_refs: inspection.unresolved_credential_refs,
        })
    }

    fn decode_export(&self, bytes: &[u8]) -> Result<ValidatedBundle, ExportError> {
        if u64::try_from(bytes.len()).unwrap_or(u64::MAX) > self.limits.maximum_export_bytes {
            return Err(ExportError::TooLarge);
        }
        let stored: StoredExportBundle = serde_json::from_slice(bytes)
            .map_err(|error| ExportError::InvalidBundle(error.to_string()))?;
        if stored.storage_version != STORAGE_VERSION
            || stored.bundle.format_version != EXPORT_FORMAT_VERSION
        {
            return Err(ExportError::InvalidBundle(
                "unsupported export format version".into(),
            ));
        }
        let canonical = canonical_json(&stored.bundle).map_err(ProfileStoreError::from)?;
        if digest(&canonical) != stored.checksum_sha256 {
            return Err(ExportError::InvalidBundle(
                "bundle checksum does not match content".into(),
            ));
        }
        if stored.bundle.revisions.is_empty() {
            return Err(ExportError::InvalidBundle(
                "bundle contains no revisions".into(),
            ));
        }

        let mut revision_keys = BTreeSet::new();
        let mut referenced_assets = BTreeMap::<AssetId, AssetReference>::new();
        for revision in &stored.bundle.revisions {
            if !revision_keys.insert((
                revision.workspace_id.clone(),
                revision.source_revision_id.clone(),
            )) {
                return Err(ExportError::InvalidBundle(
                    "bundle contains duplicate revisions".into(),
                ));
            }
            if revision.schema_version != 1 {
                return Err(ExportError::InvalidBundle(
                    "unsupported portable revision schema".into(),
                ));
            }
            validate_workspace_content(
                &revision.package_lock,
                &revision.document,
                &revision.assets,
            )?;
            for asset in &revision.assets {
                match referenced_assets.get(&asset.id) {
                    Some(existing) if existing != asset => {
                        return Err(ExportError::InvalidBundle(
                            "one asset ID has conflicting references".into(),
                        ));
                    }
                    Some(_) => {}
                    None => {
                        referenced_assets.insert(asset.id.clone(), asset.clone());
                    }
                }
            }
        }

        let mut asset_bytes = BTreeMap::new();
        for asset in &stored.bundle.assets {
            if !referenced_assets.contains_key(&asset.descriptor.id) {
                return Err(ExportError::InvalidBundle(
                    "bundle contains an unreferenced asset".into(),
                ));
            }
            if asset_bytes.contains_key(&asset.descriptor.id) {
                return Err(ExportError::InvalidBundle(
                    "bundle contains a duplicate asset".into(),
                ));
            }
            let decoded = BASE64
                .decode(&asset.content_base64)
                .map_err(|error| ExportError::InvalidBundle(error.to_string()))?;
            if u64::try_from(decoded.len()).unwrap_or(u64::MAX) > self.limits.maximum_asset_bytes
                || u64::try_from(decoded.len()).unwrap_or(u64::MAX) != asset.descriptor.size_bytes
                || digest(&decoded) != asset.descriptor.sha256
                || asset.descriptor.id.as_str() != asset.descriptor.sha256
            {
                return Err(ExportError::InvalidBundle(
                    "embedded asset checksum or size is invalid".into(),
                ));
            }
            let reference = referenced_assets
                .get(&asset.descriptor.id)
                .ok_or_else(|| ExportError::InvalidBundle("asset reference is missing".into()))?;
            if reference.media_type != asset.descriptor.media_type
                || reference.size_bytes != asset.descriptor.size_bytes
                || reference.sha256.to_ascii_lowercase() != asset.descriptor.sha256
            {
                return Err(ExportError::InvalidBundle(
                    "embedded asset metadata conflicts with its reference".into(),
                ));
            }
            asset_bytes.insert(asset.descriptor.id.clone(), decoded);
        }
        Ok(ValidatedBundle {
            bundle: stored.bundle,
            asset_bytes,
        })
    }

    fn inspect_validated(
        &self,
        validated: &ValidatedBundle,
    ) -> Result<ImportInspection, ExportError> {
        let mut revisions = Vec::new();
        let mut referenced = BTreeSet::new();
        let mut credentials = BTreeSet::new();
        for revision in &validated.bundle.revisions {
            revisions.push(ImportRevisionSummary {
                workspace_id: revision.workspace_id.clone(),
                source_revision_id: revision.source_revision_id.clone(),
                has_private_section: revision.private_section.is_some(),
                package_count: revision.package_lock.len(),
                asset_count: revision.assets.len(),
            });
            referenced.extend(revision.assets.iter().map(|asset| asset.id.clone()));
            collect_credential_refs(&revision.document, &mut credentials)?;
            if let Some(private) = &revision.private_section {
                collect_credential_refs(private, &mut credentials)?;
            }
        }

        let mut missing_assets = Vec::new();
        for asset_id in &referenced {
            if validated.asset_bytes.contains_key(asset_id) {
                continue;
            }
            match self.read_asset(asset_id) {
                Ok(_) => {}
                Err(ProfileStoreError::NotFound) => missing_assets.push(asset_id.clone()),
                Err(error) => return Err(error.into()),
            }
        }
        Ok(ImportInspection {
            revisions,
            included_assets: validated.asset_bytes.keys().cloned().collect(),
            missing_assets,
            unresolved_credential_refs: credentials.into_iter().collect(),
        })
    }
}

fn collect_credential_refs(
    value: &serde_json::Value,
    output: &mut BTreeSet<CredentialRef>,
) -> Result<(), ExportError> {
    match value {
        serde_json::Value::Object(object) => {
            for (key, child) in object {
                match key.as_str() {
                    "credentialRef" => {
                        let raw = child.as_str().ok_or_else(|| {
                            ExportError::InvalidBundle("credentialRef must be a string".into())
                        })?;
                        let reference = CredentialRef::try_new(raw).map_err(|error| {
                            ExportError::InvalidBundle(format!("invalid credentialRef: {error}"))
                        })?;
                        output.insert(reference);
                    }
                    "credentialRefs" => {
                        let values = child.as_array().ok_or_else(|| {
                            ExportError::InvalidBundle("credentialRefs must be an array".into())
                        })?;
                        for raw in values {
                            let raw = raw.as_str().ok_or_else(|| {
                                ExportError::InvalidBundle(
                                    "credentialRefs entries must be strings".into(),
                                )
                            })?;
                            let reference = CredentialRef::try_new(raw).map_err(|error| {
                                ExportError::InvalidBundle(format!(
                                    "invalid credentialRefs entry: {error}"
                                ))
                            })?;
                            output.insert(reference);
                        }
                    }
                    _ => collect_credential_refs(child, output)?,
                }
            }
        }
        serde_json::Value::Array(values) => {
            for child in values {
                collect_credential_refs(child, output)?;
            }
        }
        _ => {}
    }
    Ok(())
}
