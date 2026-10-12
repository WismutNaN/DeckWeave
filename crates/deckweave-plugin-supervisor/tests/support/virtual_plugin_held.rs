// SPDX-License-Identifier: GPL-3.0-or-later
//! Process fixture: retained outputs are released only by an exact epoch/run cleanup.
use deckweave_protocol::v1::{InvocationRequest, OwnerCleanup};
use std::{
    collections::{BTreeMap, BTreeSet},
    error::Error,
};

#[derive(Default)]
pub(super) struct HeldOutputs {
    invocations: BTreeMap<String, InvocationRequest>,
    completed: BTreeSet<String>,
}
impl HeldOutputs {
    pub(super) fn hold(&mut self, request: &InvocationRequest) -> Result<bool, Box<dyn Error>> {
        if !std::env::current_exe()?
            .with_file_name("held-mode")
            .exists()
        {
            return Ok(false);
        }
        self.invocations
            .insert(request.invocation_id.clone(), request.clone());
        self.record()?;
        Ok(true)
    }
    pub(super) fn on_locale(&mut self) -> Result<Option<InvocationRequest>, Box<dyn Error>> {
        if std::env::current_exe()?
            .with_file_name("held-crash")
            .exists()
        {
            return Err("held provider crashed".into());
        }
        let flag = std::env::current_exe()?.with_file_name("held-complete-one");
        if !flag.exists() {
            return Ok(None);
        }
        std::fs::remove_file(flag)?;
        let request = self
            .invocations
            .values()
            .find(|r| !self.completed.contains(&r.invocation_id))
            .cloned();
        if let Some(request) = &request {
            self.completed.insert(request.invocation_id.clone());
        }
        Ok(request)
    }
    pub(super) fn cleanup(&mut self, cleanup: &OwnerCleanup) -> Result<(), Box<dyn Error>> {
        self.invocations.retain(|_, request| {
            request.activation_epoch != cleanup.activation_epoch
                || cleanup
                    .run_id
                    .as_ref()
                    .is_some_and(|run| run != &request.run_id)
        });
        self.record()
    }
    fn record(&self) -> Result<(), Box<dyn Error>> {
        if !std::env::current_exe()?
            .with_file_name("held-mode")
            .exists()
        {
            return Ok(());
        }
        let owners = self
            .invocations
            .values()
            .map(|r| serde_json::json!({"run":r.run_id,"epoch":r.activation_epoch}))
            .collect::<Vec<_>>();
        std::fs::write(
            std::env::current_exe()?.with_file_name(format!("held-{}.json", std::process::id())),
            serde_json::to_vec(&owners)?,
        )?;
        Ok(())
    }
}
