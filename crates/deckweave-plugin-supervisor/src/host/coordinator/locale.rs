// SPDX-License-Identifier: GPL-3.0-or-later
use super::*;
impl deckweave_application::locale::LocalePublisher for ProductionRuntimeCoordinator {
    fn publish_locale(
        &self,
        context: deckweave_domain::localization::LocaleContext,
    ) -> Result<(), String> {
        self.set_locale(context).map_err(|e| e.to_string())
    }
}
impl ProductionRuntimeCoordinator {
    pub fn set_locale(
        &self,
        context: deckweave_domain::localization::LocaleContext,
    ) -> Result<(), HostRuntimeError> {
        context
            .validate()
            .map_err(|e| HostRuntimeError::Protocol(e.into()))?;
        let mut locale = self
            .locale
            .lock()
            .map_err(|_| HostRuntimeError::State("locale lock poisoned".into()))?;
        if context.revision < locale.revision
            || (context.revision == locale.revision && context != *locale)
        {
            return Err(HostRuntimeError::State("stale locale revision".into()));
        }
        *locale = context.clone();
        let state = self.lock_state()?;
        for worker in &state.active {
            worker
                .sender()
                .try_send(WorkerCommand::Locale(context.clone()))
                .map_err(|_| {
                    HostRuntimeError::State(
                        "locale delivery queue unavailable; retry same revision".into(),
                    )
                })?;
        }
        Ok(())
    }
}
