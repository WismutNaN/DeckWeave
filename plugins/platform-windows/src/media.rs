// SPDX-License-Identifier: GPL-3.0-or-later
use crate::PlatformFailure;
use deckweave_protocol::v1::{
    InvocationRequest, OpaquePayload, SchemaReference, TerminalOutcome, endpoint_path,
    invocation_reply, terminal_outcome,
};
use serde::Deserialize;

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
enum Command {
    PlayPause,
    Next,
    Previous,
    Stop,
}

#[cfg_attr(not(windows), allow(dead_code))]
#[derive(Debug, PartialEq, Eq)]
enum Outcome {
    Sent,
    Blocked,
    Partial { released: bool },
}

#[cfg(any(windows, test))]
trait MediaBackend {
    fn pair(&mut self, command: Command) -> u32;
    fn release(&mut self, command: Command) -> bool;
}

#[cfg(any(windows, test))]
fn apply(backend: &mut impl MediaBackend, command: Command) -> Outcome {
    match backend.pair(command) {
        2 => Outcome::Sent,
        0 => Outcome::Blocked,
        _ => Outcome::Partial {
            released: backend.release(command),
        },
    }
}

pub(crate) fn invoke(
    request: &InvocationRequest,
) -> Result<Option<invocation_reply::Reply>, PlatformFailure> {
    let target = request
        .target
        .as_ref()
        .ok_or(PlatformFailure::InvalidInput)?;
    if target.node_id != "windows-control-node"
        || target.provider_id != "org.deckweave.platform.windows.control"
        || target.element_id != "media"
        || target.capability_id != "transport-control"
        || target.port_id != "invoke"
        || !matches!(&target.target, Some(endpoint_path::Target::ServiceId(id)) if id == "windows-platform")
    {
        return Ok(None);
    }
    if request.operation_id != "media-transport" {
        return Err(PlatformFailure::InvalidInput);
    }
    if request.remaining_budget_ms.unwrap_or(0) == 0 {
        return Err(PlatformFailure::DeadlineExceeded);
    }
    let input = request
        .input
        .as_ref()
        .ok_or(PlatformFailure::InvalidInput)?;
    if input.value.len() > 64
        || !input.schema.as_ref().is_some_and(|s| {
            s.id == "org.deckweave.windows.media_transport" && s.major == 1 && s.minor == 0
        })
    {
        return Err(PlatformFailure::InvalidInput);
    }
    let command =
        serde_json::from_slice(&input.value).map_err(|_| PlatformFailure::InvalidInput)?;
    let outcome = native(command)?;
    let result = match outcome {
        Outcome::Sent => terminal_outcome::Result::Succeeded(OpaquePayload {
            schema: Some(SchemaReference {
                id: "org.deckweave.windows.unit".into(),
                major: 1,
                minor: 0,
            }),
            value: b"null".to_vec(),
        }),
        Outcome::Blocked => {
            return Ok(Some(invocation_reply::Reply::Rejected(
                PlatformFailure::InputBlocked.structured(),
            )));
        }
        Outcome::Partial { .. } => {
            terminal_outcome::Result::UnknownOutcome(PlatformFailure::PartialInput.structured())
        }
    };
    Ok(Some(invocation_reply::Reply::Terminal(TerminalOutcome {
        invocation_id: request.invocation_id.clone(),
        result: Some(result),
    })))
}

#[cfg(not(windows))]
fn native(_: Command) -> Result<Outcome, PlatformFailure> {
    Err(PlatformFailure::UnsupportedOperatingSystem)
}

#[cfg(windows)]
fn native(command: Command) -> Result<Outcome, PlatformFailure> {
    native::send(command)
}

#[cfg(windows)]
mod native {
    #![allow(unsafe_code)]
    use super::*;
    use std::sync::OnceLock;
    use windows::Win32::UI::Input::KeyboardAndMouse::{
        INPUT, INPUT_0, INPUT_KEYBOARD, KEYBD_EVENT_FLAGS, KEYBDINPUT, KEYEVENTF_KEYUP, SendInput,
        VIRTUAL_KEY,
    };
    static ORIGIN: OnceLock<usize> = OnceLock::new();
    struct Backend {
        origin: usize,
    }
    impl Backend {
        fn event(&self, command: Command, up: bool) -> INPUT {
            let key = match command {
                Command::PlayPause => 0xb3,
                Command::Next => 0xb0,
                Command::Previous => 0xb1,
                Command::Stop => 0xb2,
            };
            INPUT {
                r#type: INPUT_KEYBOARD,
                Anonymous: INPUT_0 {
                    ki: KEYBDINPUT {
                        wVk: VIRTUAL_KEY(key),
                        dwFlags: if up {
                            KEYEVENTF_KEYUP
                        } else {
                            KEYBD_EVENT_FLAGS(0)
                        },
                        dwExtraInfo: self.origin,
                        ..Default::default()
                    },
                },
            }
        }
    }
    impl MediaBackend for Backend {
        fn pair(&mut self, command: Command) -> u32 {
            let events = [self.event(command, false), self.event(command, true)];
            // SAFETY: initialized keyboard events; no held output escapes a successful batch.
            unsafe { SendInput(&events, size_of::<INPUT>() as i32) }
        }
        fn release(&mut self, command: Command) -> bool {
            // SAFETY: a failed partial batch gets exactly one matching key-up attempt.
            unsafe { SendInput(&[self.event(command, true)], size_of::<INPUT>() as i32) == 1 }
        }
    }
    pub(super) fn send(command: Command) -> Result<Outcome, PlatformFailure> {
        let origin = match ORIGIN.get() {
            Some(value) => *value,
            None => {
                let mut bytes = [0; size_of::<usize>()];
                getrandom::fill(&mut bytes).map_err(|_| PlatformFailure::OsApiUnavailable)?;
                *ORIGIN.get_or_init(|| usize::from_ne_bytes(bytes) | 1)
            }
        };
        Ok(apply(&mut Backend { origin }, command))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    struct Backend {
        count: u32,
        releases: u32,
        released: bool,
    }
    impl MediaBackend for Backend {
        fn pair(&mut self, _: Command) -> u32 {
            self.count
        }
        fn release(&mut self, _: Command) -> bool {
            self.releases += 1;
            self.released
        }
    }
    #[test]
    fn partial_batch_never_reports_success_even_when_release_succeeds() {
        for released in [true, false] {
            let mut backend = Backend {
                count: 1,
                releases: 0,
                released,
            };
            assert_eq!(
                apply(&mut backend, Command::Next),
                Outcome::Partial { released }
            );
            assert_eq!(backend.releases, 1);
        }
    }
    #[test]
    fn zero_or_complete_send_does_not_release_someone_elses_key() {
        for (count, expected) in [(0, Outcome::Blocked), (2, Outcome::Sent)] {
            let mut backend = Backend {
                count,
                releases: 0,
                released: true,
            };
            assert_eq!(apply(&mut backend, Command::PlayPause), expected);
            assert_eq!(backend.releases, 0);
        }
    }
}
