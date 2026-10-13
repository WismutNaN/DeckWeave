// SPDX-License-Identifier: GPL-3.0-or-later
//! Serializes activation with publication; an old preparation snapshot cannot overtake live state.
#[derive(Default)]
pub(crate) struct StateGate<T> {
    committed: bool,
    pending: Option<T>,
    sequence: u64,
}

impl<T> StateGate<T> {
    pub fn new() -> Self {
        Self {
            committed: false,
            pending: None,
            sequence: 0,
        }
    }

    pub fn publish(
        &mut self,
        state: T,
        send: impl FnOnce(u64, &T) -> Result<(), String>,
    ) -> Result<(), String> {
        if !self.committed {
            self.pending = Some(state);
            return Ok(());
        }
        self.sequence = self
            .sequence
            .checked_add(1)
            .ok_or("state sequence exhausted")?;
        send(self.sequence, &state)
    }

    pub fn commit(
        &mut self,
        send: impl FnOnce(u64, &T) -> Result<(), String>,
    ) -> Result<(), String> {
        self.committed = true;
        if let Some(state) = self.pending.take() {
            self.publish(state, send)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn preparation_coalesces_and_cannot_overtake_live_state() -> Result<(), String> {
        let mut gate = StateGate::new();
        gate.publish(1, |_, _| Err("published before commit".into()))?;
        gate.publish(2, |_, _| Err("published before commit".into()))?;
        let mut sent = Vec::new();
        gate.commit(|seq, state| {
            sent.push((seq, *state));
            Ok(())
        })?;
        gate.publish(3, |seq, state| {
            sent.push((seq, *state));
            Ok(())
        })?;
        gate.commit(|_, _| Err("duplicate commit replayed stale state".into()))?;
        assert_eq!(sent, [(1, 2), (2, 3)]);
        Ok(())
    }
}
