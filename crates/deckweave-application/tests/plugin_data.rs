// SPDX-License-Identifier: GPL-3.0-or-later
//! Conformance fake for a scoped adapter, not a production persistence claim.
use deckweave_application::plugin_data::*;
use std::{collections::BTreeMap, error::Error, sync::Mutex};

struct Documents {
    scope: PluginDataScope,
    values: Mutex<BTreeMap<String, PluginDocument>>,
}
impl PluginDocumentStore for Documents {
    fn scope(&self) -> &PluginDataScope {
        &self.scope
    }
    fn get(&self, id: &str) -> Result<Option<PluginDocument>, PluginDataError> {
        Ok(self
            .values
            .lock()
            .map_err(|_| PluginDataError::Unavailable("lock".into()))?
            .get(id)
            .cloned())
    }
    fn compare_exchange(
        &self,
        expected: u64,
        value: PluginDocument,
    ) -> Result<(), PluginDataError> {
        let mut records = self
            .values
            .lock()
            .map_err(|_| PluginDataError::Unavailable("lock".into()))?;
        if value.document_id.is_empty()
            || !deckweave_domain::schema::bounded_json(&value.value)
            || expected.checked_add(1) != Some(value.revision)
        {
            return Err(PluginDataError::Invalid);
        }
        if records.get(&value.document_id).map_or(0, |r| r.revision) != expected {
            return Err(PluginDataError::Conflict);
        }
        if records.len() >= 16 && !records.contains_key(&value.document_id) {
            return Err(PluginDataError::Quota);
        }
        records.insert(value.document_id.clone(), value);
        Ok(())
    }
    fn export_private(&self, maximum: usize) -> Result<Vec<PluginDocument>, PluginDataError> {
        let values: Vec<_> = self
            .values
            .lock()
            .map_err(|_| PluginDataError::Unavailable("lock".into()))?
            .values()
            .cloned()
            .collect();
        if serde_json::to_vec(&values)
            .map_err(|_| PluginDataError::Invalid)?
            .len()
            > maximum
        {
            return Err(PluginDataError::Quota);
        }
        Ok(values)
    }
}
#[test]
fn private_document_scope_revision_and_export_are_independent_of_workspace_revision()
-> Result<(), Box<dyn Error>> {
    let scope: PluginDataScope = serde_json::from_value(
        serde_json::json!({"packageId":"org.fixture.notes","instanceId":"account-a","dataRootId":"local"}),
    )?;
    let first = Documents {
        scope: scope.clone(),
        values: Mutex::new(BTreeMap::new()),
    };
    let mut other_scope = scope;
    other_scope.instance_id = deckweave_domain::ids::PluginInstanceId::try_new("account-b")?;
    let other = Documents {
        scope: other_scope,
        values: Mutex::new(BTreeMap::new()),
    };
    let mut note: PluginDocument = serde_json::from_value(
        serde_json::json!({"documentId":"note","revision":1,
        "schema":{"id":"org.fixture.note","version":{"major":1,"minor":0}},"value":{"text":"private"}}),
    )?;
    first.compare_exchange(0, note.clone())?;
    assert!(other.get("note")?.is_none());
    assert_eq!(
        first.compare_exchange(0, note.clone()),
        Err(PluginDataError::Conflict)
    );
    note.revision = 2;
    note.value = serde_json::json!({"text":"updated"});
    first.compare_exchange(1, note.clone())?;
    // Workspace/view revisions are not inputs to this port and cannot restore older note content.
    assert_eq!(first.get("note")?, Some(note));
    assert_eq!(first.export_private(1), Err(PluginDataError::Quota));
    assert_eq!(first.export_private(4096)?.len(), 1);
    Ok(())
}
