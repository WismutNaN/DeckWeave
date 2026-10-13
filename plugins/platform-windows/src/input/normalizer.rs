// SPDX-License-Identifier: GPL-3.0-or-later
use deckweave_platform_input::{
    InputObservation, InputOrigin, KeyEdge, KeyLocation, NormalizedKeyIdentity,
};
use serde::Deserialize;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Deserialize)]
pub(super) struct Key {
    pub scan: u16,
    pub extended: bool,
    pub key: String,
    pub location: KeyLocation,
}

pub(super) fn keys() -> Result<Vec<Key>, serde_json::Error> {
    serde_json::from_str(include_str!("keys.json"))
}

pub(super) struct Normalized {
    pub observation: InputObservation,
    pub key: String,
    pub pressed: Option<bool>,
}

pub(super) struct Normalizer {
    keys: Vec<Key>,
    held: BTreeSet<(usize, usize)>,
    counts: BTreeMap<usize, usize>,
    quarantined: BTreeSet<usize>,
}

impl Normalizer {
    pub fn new(keys: Vec<Key>, quarantined: BTreeSet<usize>) -> Self {
        Self {
            keys,
            held: BTreeSet::new(),
            counts: BTreeMap::new(),
            quarantined,
        }
    }

    pub fn key(
        &mut self,
        device: usize,
        scan: u16,
        flags: u16,
        vkey: u16,
        neutral: bool,
    ) -> Result<Option<Normalized>, &'static str> {
        if scan == 0xff {
            return Err("keyboard input overrun");
        }
        // Raw Input cannot authenticate injected origins. Excluding null-device/consumer
        // media records prevents our allowlisted SendInput fallback from feeding bindings.
        if device == 0
            || scan == 0
            || vkey >= 0xff
            || (0xb0..=0xb3).contains(&vkey)
            || flags & 4 != 0
        {
            return Ok(None);
        }
        let Some(index) = self
            .keys
            .iter()
            .position(|k| k.scan == scan && k.extended == (flags & 2 != 0))
        else {
            return Ok(None);
        };
        let up = flags & 1 != 0;
        if self.quarantined.contains(&index) {
            if up && neutral {
                self.quarantined.remove(&index);
            }
            return Ok(None);
        }
        let count = self.counts.entry(index).or_default();
        let (edge, pressed) = if up {
            if !self.held.remove(&(device, index)) {
                return Ok(None);
            }
            *count = count.saturating_sub(1);
            (KeyEdge::Released, (*count == 0).then_some(false))
        } else if self.held.contains(&(device, index)) {
            (KeyEdge::Repeated, None)
        } else {
            if self.held.len() >= 4096 {
                return Err("keyboard held-state limit");
            }
            self.held.insert((device, index));
            *count += 1;
            (KeyEdge::Pressed, (*count == 1).then_some(true))
        };
        let key = &self.keys[index];
        Ok(Some(Normalized {
            key: key.key.clone(),
            pressed,
            observation: InputObservation {
                key: NormalizedKeyIdentity::try_new(
                    format!("org.deckweave.key.{}", key.key),
                    key.location,
                )
                .map_err(|_| "key identity")?,
                edge,
                origin: InputOrigin::DeviceOrUnknown,
            },
        }))
    }

    pub fn remove(&mut self, device: usize) -> Vec<String> {
        let removed: Vec<_> = self
            .held
            .iter()
            .copied()
            .filter(|(d, _)| *d == device)
            .collect();
        let mut released = Vec::new();
        for pair @ (_, index) in removed {
            self.held.remove(&pair);
            if let Some(count) = self.counts.get_mut(&index) {
                *count = count.saturating_sub(1);
                if *count == 0 {
                    released.push(self.keys[index].key.clone());
                }
            }
        }
        released
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn aggregate_counts_two_keyboards_and_does_not_turn_repeat_into_press()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut normal = Normalizer::new(keys()?, BTreeSet::new());
        assert_eq!(
            normal.key(1, 30, 0, 65, true)?.ok_or("down")?.pressed,
            Some(true)
        );
        let repeat = normal.key(1, 30, 0, 65, true)?.ok_or("repeat")?;
        assert_eq!(repeat.pressed, None);
        assert_eq!(repeat.key, "KeyA");
        assert_eq!(repeat.observation.edge, KeyEdge::Repeated);
        assert_eq!(
            normal.key(2, 30, 0, 65, true)?.ok_or("second")?.pressed,
            None
        );
        assert!(normal.remove(1).is_empty());
        assert_eq!(
            normal.key(2, 30, 1, 65, true)?.ok_or("up")?.pressed,
            Some(false)
        );
        assert!(normal.key(2, 30, 1, 65, true)?.is_none());
        Ok(())
    }
    #[test]
    fn rearm_drops_orphan_edges_and_media_feedback() -> Result<(), Box<dyn std::error::Error>> {
        let keys = keys()?;
        let index = keys.iter().position(|k| k.key == "KeyA").ok_or("A")?;
        let mut normal = Normalizer::new(keys, BTreeSet::from([index]));
        assert!(normal.key(1, 30, 0, 65, true)?.is_none());
        assert!(normal.key(1, 30, 1, 65, false)?.is_none());
        assert!(normal.key(2, 30, 0, 65, false)?.is_none());
        assert!(normal.key(1, 30, 1, 65, true)?.is_none());
        assert_eq!(
            normal.key(1, 30, 0, 65, true)?.ok_or("rearmed")?.pressed,
            Some(true)
        );
        assert_eq!(normal.remove(1), ["KeyA"]);
        assert!(normal.remove(1).is_empty());
        assert!(normal.key(0, 30, 0, 65, true)?.is_none());
        for key in 0xb0..=0xb3 {
            assert!(normal.key(1, 30, 0, key, true)?.is_none());
        }
        Ok(())
    }
}
