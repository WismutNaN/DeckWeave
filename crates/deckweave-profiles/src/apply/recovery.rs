// SPDX-License-Identifier: GPL-3.0-or-later
use super::*;
impl JsonProfileStore {
    pub(crate) fn recoverable_operations_locked(
        &self,
        maximum: usize,
    ) -> Result<Vec<ApplyOperationSnapshot>, ApplyRepositoryError> {
        let directory = self.apply_operations_directory();
        let entries = match fs::read_dir(&directory) {
            Ok(entries) => entries,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(error) => return Err(ApplyRepositoryError::Unavailable(error.to_string())),
        };
        let active = self.active_commit_inner()?;
        let mut pending = Vec::new();
        for entry in entries {
            let entry =
                entry.map_err(|error| ApplyRepositoryError::Unavailable(error.to_string()))?;
            if !entry
                .file_type()
                .map_err(|error| ApplyRepositoryError::Unavailable(error.to_string()))?
                .is_dir()
            {
                return Err(ApplyRepositoryError::InvalidDocument(
                    "unexpected file in apply operations directory".into(),
                ));
            }
            let intent_path = entry.path().join("intent.json");
            let intent: ApplyOperationIntent = self.read_apply_document(&intent_path)?;
            let mut snapshot = self.operation_inner(&intent.operation_id)?;
            if snapshot.state.is_terminal() {
                continue;
            }
            // Migration repair for data written before the previous-head
            // sealing rule: the current commit still proves that its direct
            // predecessor selection was once durably committed.
            if active
                .as_ref()
                .and_then(|commit| commit.previous_active.as_ref())
                .is_some_and(|selection| *selection == snapshot.intent.candidate)
            {
                snapshot.commit_status = ApplyCommitStatus::Committed;
            }
            pending.push(snapshot);
            if pending.len() > maximum {
                return Err(ApplyRepositoryError::CapacityExceeded);
            }
        }
        pending.sort_by(|left, right| left.intent.operation_id.cmp(&right.intent.operation_id));
        Ok(pending)
    }
}
