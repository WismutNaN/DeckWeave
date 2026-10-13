// SPDX-License-Identifier: GPL-3.0-or-later
#[cfg(windows)]
mod native;
#[cfg(any(windows, test))]
mod normalizer;

#[cfg(windows)]
mod bridge;
#[cfg(windows)]
pub(crate) use bridge::InputWorker;

#[cfg(not(windows))]
pub(crate) struct InputWorker;
#[cfg(not(windows))]
impl InputWorker {
    pub fn start(_: crate::wire::WireSender, _: u64) -> Result<Self, String> {
        Err("keyboard observation requires Windows".into())
    }
}
