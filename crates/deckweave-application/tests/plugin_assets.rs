// SPDX-License-Identifier: GPL-3.0-or-later
//! Private storage contract fake; byte decoding/rendering belongs to the later adapter.
use deckweave_application::plugin_data::*;
use deckweave_domain::ids::{PackageId, PluginInstanceId};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    error::Error,
    sync::Mutex,
};

#[derive(Default)]
struct State {
    blobs: BTreeMap<String, (PluginAsset, Vec<u8>)>,
    documents: BTreeSet<String>,
    leases: BTreeMap<String, BTreeSet<String>>,
}
struct Assets {
    scope: PluginDataScope,
    state: Mutex<State>,
}
impl Assets {
    fn pin_document(&self, asset: &PluginAsset) -> Result<(), Box<dyn Error>> {
        self.state
            .lock()
            .map_err(|e| e.to_string())?
            .documents
            .insert(asset.sha256.clone());
        Ok(())
    }
    fn close_view(&self, owner: &str) -> Result<(), Box<dyn Error>> {
        self.state
            .lock()
            .map_err(|e| e.to_string())?
            .leases
            .remove(owner);
        Ok(())
    }
    fn collect_unreferenced(&self) -> Result<(), Box<dyn Error>> {
        let mut state = self.state.lock().map_err(|e| e.to_string())?;
        let mut retained = state.documents.clone();
        for lease in state.leases.values() {
            retained.extend(lease.iter().cloned());
        }
        state.blobs.retain(|id, _| retained.contains(id));
        Ok(())
    }
}
impl PluginAssetStore for Assets {
    fn scope(&self) -> &PluginDataScope {
        &self.scope
    }
    fn put(&self, mime_type: &str, bytes: &[u8]) -> Result<PluginAsset, PluginDataError> {
        if mime_type.len() > 128 || !mime_type.contains('/') {
            return Err(PluginDataError::Invalid);
        }
        let mut state = self
            .state
            .lock()
            .map_err(|e| PluginDataError::Unavailable(e.to_string()))?;
        let asset = PluginAsset {
            sha256: format!("{:x}", Sha256::digest(bytes)),
            mime_type: mime_type.into(),
            byte_length: bytes.len() as u64,
        };
        if let Some((old, _)) = state.blobs.get(&asset.sha256) {
            return if old == &asset {
                Ok(asset)
            } else {
                Err(PluginDataError::Invalid)
            };
        }
        if bytes.len() + state.blobs.values().map(|(_, b)| b.len()).sum::<usize>() > 4096 {
            return Err(PluginDataError::Quota);
        }
        state
            .blobs
            .insert(asset.sha256.clone(), (asset.clone(), bytes.to_vec()));
        Ok(asset)
    }
    fn read(&self, asset: &PluginAsset, maximum: usize) -> Result<Vec<u8>, PluginDataError> {
        let state = self
            .state
            .lock()
            .map_err(|e| PluginDataError::Unavailable(e.to_string()))?;
        let (stored, bytes) = state
            .blobs
            .get(&asset.sha256)
            .ok_or(PluginDataError::Denied)?;
        if stored != asset || asset.sha256 != format!("{:x}", Sha256::digest(bytes)) {
            return Err(PluginDataError::Invalid);
        }
        if bytes.len() > maximum {
            return Err(PluginDataError::Quota);
        }
        Ok(bytes.clone())
    }
}

#[test]
fn note_palette_assets_remain_scoped_and_survive_view_close_and_workspace_rollback()
-> Result<(), Box<dyn Error>> {
    let scope = PluginDataScope {
        package_id: PackageId::try_new("org.fixture.notes")?,
        instance_id: PluginInstanceId::try_new("a")?,
        data_root_id: "local".into(),
    };
    let first = Assets {
        scope: scope.clone(),
        state: Mutex::new(State::default()),
    };
    let other = Assets {
        scope: PluginDataScope {
            instance_id: PluginInstanceId::try_new("b")?,
            ..scope
        },
        state: Mutex::new(State::default()),
    };
    let image = first.put("image/png", b"opaque-palette-bytes")?;
    first.pin_document(&image)?;
    let temporary = first.put("audio/wav", b"opaque-preview-bytes")?;
    first
        .state
        .lock()
        .map_err(|e| e.to_string())?
        .leases
        .insert("view".into(), BTreeSet::from([temporary.sha256.clone()]));
    assert_eq!(first.put("image/png", b"opaque-palette-bytes")?, image);
    assert_eq!(other.read(&image, 4096), Err(PluginDataError::Denied));
    assert_eq!(first.read(&image, 1), Err(PluginDataError::Quota));
    assert_eq!(
        first.put("image/png", &[0; 4096]),
        Err(PluginDataError::Quota)
    );
    first.collect_unreferenced()?;
    assert!(first.read(&temporary, 4096).is_ok());
    first.close_view("view")?;
    first.collect_unreferenced()?;
    assert!(first.read(&temporary, 4096).is_err());
    assert_eq!(first.read(&image, 4096)?, b"opaque-palette-bytes");
    // Workspace revisions are not part of the private store's key or reference roots.
    let mut forged = image.clone();
    forged.mime_type = "audio/wav".into();
    assert_eq!(first.read(&forged, 4096), Err(PluginDataError::Invalid));
    Ok(())
}
