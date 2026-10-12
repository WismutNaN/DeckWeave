// SPDX-License-Identifier: GPL-3.0-or-later
use crate::{DraftCommand, DraftMutationError, JsonProfileStore, MutationKey, MutationOutcome};
use deckweave_domain::ids::{DraftId, DraftVersion};
use std::{collections::VecDeque, sync::Arc};

pub struct DraftEditor {
    store: Arc<JsonProfileStore>,
    draft_id: DraftId,
    head_version: DraftVersion,
    undo: VecDeque<DraftCommand>,
    redo: VecDeque<DraftCommand>,
    maximum_history: usize,
}

impl DraftEditor {
    pub fn open(
        store: Arc<JsonProfileStore>,
        draft_id: DraftId,
        maximum_history: usize,
    ) -> Result<Self, DraftMutationError> {
        let loaded = store.load_draft(&draft_id)?;
        Ok(Self {
            store,
            draft_id,
            head_version: loaded.draft.version,
            undo: VecDeque::new(),
            redo: VecDeque::new(),
            maximum_history,
        })
    }

    pub fn apply(
        &mut self,
        key: MutationKey,
        expected_version: DraftVersion,
        command: DraftCommand,
    ) -> Result<MutationOutcome, DraftMutationError> {
        let before = self.store.load_draft(&self.draft_id)?.draft;
        let outcome = self
            .store
            .mutate_draft(&self.draft_id, key, expected_version, &command)?;
        if !outcome.replayed {
            if self.head_version != before.version || before.version != expected_version {
                self.undo.clear();
            }
            // Read and mutation are separate: retain history only for the verified base.
            if before.version == expected_version {
                push_bounded(&mut self.undo, configuration(before), self.maximum_history);
            }
            self.redo.clear();
            self.head_version = outcome.receipt.after_version;
        }
        Ok(outcome)
    }

    pub fn undo(
        &mut self,
        key: MutationKey,
    ) -> Result<Option<MutationOutcome>, DraftMutationError> {
        let Some(previous) = self.undo.back().cloned() else {
            return Ok(None);
        };
        let current = self.store.load_draft(&self.draft_id)?.draft;
        let outcome = self
            .store
            .mutate_draft(&self.draft_id, key, self.head_version, &previous)?;
        if !outcome.replayed {
            self.undo.pop_back();
            push_bounded(&mut self.redo, configuration(current), self.maximum_history);
            self.head_version = outcome.receipt.after_version;
        }
        Ok(Some(outcome))
    }

    pub fn redo(
        &mut self,
        key: MutationKey,
    ) -> Result<Option<MutationOutcome>, DraftMutationError> {
        let Some(next) = self.redo.back().cloned() else {
            return Ok(None);
        };
        let current = self.store.load_draft(&self.draft_id)?.draft;
        let outcome = self
            .store
            .mutate_draft(&self.draft_id, key, self.head_version, &next)?;
        if !outcome.replayed {
            self.redo.pop_back();
            push_bounded(&mut self.undo, configuration(current), self.maximum_history);
            self.head_version = outcome.receipt.after_version;
        }
        Ok(Some(outcome))
    }

    #[must_use]
    pub fn has_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    #[must_use]
    pub fn has_redo(&self) -> bool {
        !self.redo.is_empty()
    }
}

fn push_bounded(history: &mut VecDeque<DraftCommand>, value: DraftCommand, maximum: usize) {
    if maximum == 0 {
        return;
    }
    if history.len() == maximum {
        history.pop_front();
    }
    history.push_back(value);
}

fn configuration(draft: crate::WorkspaceDraft) -> DraftCommand {
    DraftCommand::ReplaceConfiguration {
        document: draft.document,
        package_lock: draft.package_lock,
    }
}
