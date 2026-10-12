// SPDX-License-Identifier: GPL-3.0-or-later
//! Read-only проекция: никакие handles/leases не выходят в editor.
use super::*;
use crate::package::PackageIndex;
use deckweave_application::editor::*;
use deckweave_workspace::ApplyOperationRepository;
use std::sync::Arc;

pub struct ProductionEditorSource {
    pub runtime: Arc<ProductionRuntimeCoordinator>,
    pub packages: Arc<PackageIndex>,
    pub profiles: Arc<dyn ApplyOperationRepository>,
}
impl EditorReadSource for ProductionEditorSource {
    fn editor_snapshot(&self) -> Result<EditorSnapshot, String> {
        let snapshot = self
            .runtime
            .registry_snapshot()
            .map_err(|e| e.to_string())?;
        if snapshot.nodes.len() > 4096 {
            return Err("editor node limit exceeded".into());
        }
        let active = self
            .profiles
            .active_commit()
            .map_err(|e| e.to_string())?
            .map(|c| c.candidate);
        Ok(EditorSnapshot {
            registry_revision: snapshot.revision.get(),
            installed: self.packages.editor_packages().map_err(|e| e.to_string())?,
            nodes: snapshot
                .nodes
                .into_iter()
                .map(|node| EditorNodeView {
                    package_id: node.session.package_id,
                    generation: node.session.generation.get(),
                    descriptor: node.descriptor,
                })
                .collect(),
            runtimes: self
                .runtime
                .runtime_health()
                .runtimes
                .into_iter()
                .map(|entry| EditorRuntimeView {
                    package_id: entry.package_id,
                    status: format!("{:?}", entry.status).to_ascii_lowercase(),
                    detail: entry.detail,
                })
                .collect(),
            active,
        })
    }
}
