// SPDX-License-Identifier: GPL-3.0-or-later
use deckweave_protocol::{
    framing::write_envelope,
    v1::{ProtocolEnvelope, ProtocolVersion, protocol_envelope},
};
use std::{
    io,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
        mpsc::{self, SyncSender},
    },
    thread::{self, JoinHandle},
};

const OUTBOUND_CAPACITY: usize = 64;

#[derive(Clone)]
pub(crate) struct WireSender {
    messages: SyncSender<protocol_envelope::Body>,
    maximum_message_bytes: Arc<AtomicUsize>,
}

impl WireSender {
    pub fn start(
        generation: u64,
        maximum_message_bytes: usize,
    ) -> Result<(Self, JoinHandle<Result<(), String>>), io::Error> {
        let (messages, receiver) = mpsc::sync_channel(OUTBOUND_CAPACITY);
        let maximum = Arc::new(AtomicUsize::new(maximum_message_bytes));
        let writer_maximum = Arc::clone(&maximum);
        let join = thread::Builder::new()
            .name("deckweave-windows-wire".into())
            .spawn(move || {
                let mut stdout = io::stdout().lock();
                let mut message_id = 0_u64;
                for body in receiver {
                    message_id = message_id
                        .checked_add(1)
                        .ok_or_else(|| "message ID exhausted".to_string())?;
                    write_envelope(
                        &mut stdout,
                        &ProtocolEnvelope {
                            wire_version: Some(ProtocolVersion { major: 1, minor: 0 }),
                            session_generation: generation,
                            message_id,
                            trace: None,
                            body: Some(body),
                        },
                        writer_maximum.load(Ordering::Acquire),
                    )
                    .map_err(|error| error.to_string())?;
                }
                Ok(())
            })?;
        Ok((
            Self {
                messages,
                maximum_message_bytes: maximum,
            },
            join,
        ))
    }

    pub fn send(&self, body: protocol_envelope::Body) -> Result<(), String> {
        self.messages
            .send(body)
            .map_err(|_| "wire writer stopped".into())
    }

    pub fn set_maximum_message_bytes(&self, maximum: usize) {
        self.maximum_message_bytes.store(maximum, Ordering::Release);
    }

    #[must_use]
    pub fn maximum_message_bytes(&self) -> usize {
        self.maximum_message_bytes.load(Ordering::Acquire)
    }
}
