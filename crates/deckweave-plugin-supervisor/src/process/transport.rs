// SPDX-License-Identifier: GPL-3.0-or-later
//! Bounded process pipes. No caller performs blocking plugin I/O.

use super::{ProcessConfig, ProcessDiagnostics, ProcessError};
use deckweave_protocol::{
    framing::{read_envelope, write_envelope},
    v1::ProtocolEnvelope,
};
use std::{
    collections::BTreeMap,
    io::{Read, Write},
    process::{ChildStderr, ChildStdin, ChildStdout},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicUsize, Ordering},
        mpsc,
    },
    thread,
    time::{Duration, Instant},
};

enum OutboundItem {
    Reliable(Vec<u8>),
    FlushLatest,
}

pub(super) struct ProcessTransport {
    outbound: Option<mpsc::SyncSender<OutboundItem>>,
    frames: mpsc::Receiver<Result<ProtocolEnvelope, deckweave_protocol::framing::FramingError>>,
    diagnostics: Arc<Mutex<ProcessDiagnostics>>,
    maximum_message_bytes: Arc<AtomicUsize>,
    latest: Arc<Mutex<BTreeMap<String, Vec<u8>>>>,
    maximum_coalesced_keys: usize,
    coalesced_outbound: Arc<AtomicUsize>,
    stdout_reader: Option<thread::JoinHandle<()>>,
    stderr_reader: Option<thread::JoinHandle<()>>,
    stdin_writer: Option<thread::JoinHandle<()>>,
    inbound_overflow: Arc<AtomicBool>,
    outbound_faulted: Arc<AtomicBool>,
    writer_failed: Arc<AtomicBool>,
}

impl ProcessTransport {
    pub(super) fn spawn(
        stdin: ChildStdin,
        mut stdout: ChildStdout,
        mut stderr: ChildStderr,
        config: &ProcessConfig,
    ) -> Result<Self, ProcessError> {
        if config.maximum_inbound_frames == 0
            || config.maximum_outbound_frames == 0
            || config.maximum_coalesced_keys == 0
        {
            return Err(ProcessError::InvalidQueueCapacity);
        }

        let maximum_message_bytes = Arc::new(AtomicUsize::new(config.maximum_message_bytes));
        let (frame_sender, frames) = mpsc::sync_channel(config.maximum_inbound_frames);
        let inbound_overflow = Arc::new(AtomicBool::new(false));
        let reader_overflow = Arc::clone(&inbound_overflow);
        let reader_limit = Arc::clone(&maximum_message_bytes);
        let stdout_reader = thread::Builder::new()
            .name("deckweave-plugin-stdout".into())
            .spawn(move || {
                loop {
                    match read_envelope(&mut stdout, reader_limit.load(Ordering::Acquire)) {
                        Ok(Some(envelope)) => {
                            if let Err(error) = frame_sender.try_send(Ok(envelope)) {
                                if matches!(error, mpsc::TrySendError::Full(_)) {
                                    reader_overflow.store(true, Ordering::Release);
                                }
                                break;
                            }
                        }
                        Ok(None) => break,
                        Err(error) => {
                            if let Err(send_error) = frame_sender.try_send(Err(error))
                                && matches!(send_error, mpsc::TrySendError::Full(_))
                            {
                                reader_overflow.store(true, Ordering::Release);
                            }
                            break;
                        }
                    }
                }
            })
            .map_err(ProcessError::Spawn)?;

        let diagnostics = Arc::new(Mutex::new(ProcessDiagnostics {
            stderr: String::new(),
            truncated: false,
            coalesced_outbound: 0,
        }));
        let diagnostics_writer = Arc::clone(&diagnostics);
        let maximum_stderr_bytes = config.maximum_stderr_bytes;
        let stderr_reader = thread::Builder::new()
            .name("deckweave-plugin-stderr".into())
            .spawn(move || {
                let mut bytes = Vec::with_capacity(maximum_stderr_bytes);
                let mut truncated = false;
                let mut chunk = [0_u8; 8192];
                loop {
                    match stderr.read(&mut chunk) {
                        Ok(0) => break,
                        Ok(read) => {
                            let retained =
                                read.min(maximum_stderr_bytes.saturating_sub(bytes.len()));
                            bytes.extend_from_slice(&chunk[..retained]);
                            truncated |= retained < read;
                        }
                        Err(error) if error.kind() == std::io::ErrorKind::Interrupted => {}
                        Err(_) => break,
                    }
                }
                let mut diagnostics = diagnostics_writer
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                diagnostics.stderr = String::from_utf8_lossy(&bytes).into_owned();
                diagnostics.truncated = truncated;
            })
            .map_err(ProcessError::Spawn)?;

        let (outbound, outbound_receiver) = mpsc::sync_channel(config.maximum_outbound_frames);
        let latest = Arc::new(Mutex::new(BTreeMap::<String, Vec<u8>>::new()));
        let writer_latest = Arc::clone(&latest);
        let writer_failed = Arc::new(AtomicBool::new(false));
        let writer_failed_signal = Arc::clone(&writer_failed);
        let stdin_writer = thread::Builder::new()
            .name("deckweave-plugin-stdin".into())
            .spawn(move || {
                let mut stdin = stdin;
                while let Ok(item) = outbound_receiver.recv() {
                    if let OutboundItem::Reliable(bytes) = item
                        && write_frame(&mut stdin, &bytes).is_err()
                    {
                        writer_failed_signal.store(true, Ordering::Release);
                        return;
                    }
                    let pending = {
                        let mut latest = writer_latest
                            .lock()
                            .unwrap_or_else(std::sync::PoisonError::into_inner);
                        std::mem::take(&mut *latest)
                    };
                    for bytes in pending.into_values() {
                        if write_frame(&mut stdin, &bytes).is_err() {
                            writer_failed_signal.store(true, Ordering::Release);
                            return;
                        }
                    }
                }
            })
            .map_err(ProcessError::Spawn)?;

        Ok(Self {
            outbound: Some(outbound),
            frames,
            diagnostics,
            maximum_message_bytes,
            latest,
            maximum_coalesced_keys: config.maximum_coalesced_keys,
            coalesced_outbound: Arc::new(AtomicUsize::new(0)),
            stdout_reader: Some(stdout_reader),
            stderr_reader: Some(stderr_reader),
            stdin_writer: Some(stdin_writer),
            inbound_overflow,
            outbound_faulted: Arc::new(AtomicBool::new(false)),
            writer_failed,
        })
    }

    pub(super) fn send_reliable(&self, envelope: &ProtocolEnvelope) -> Result<(), ProcessError> {
        if self.outbound_faulted.load(Ordering::Acquire) {
            return Err(ProcessError::OutboundQueueOverflow);
        }
        if self.writer_failed.load(Ordering::Acquire) {
            return Err(ProcessError::OutboundDisconnected);
        }
        let bytes = self.encode(envelope)?;
        match self
            .outbound
            .as_ref()
            .ok_or(ProcessError::OutboundDisconnected)?
            .try_send(OutboundItem::Reliable(bytes))
        {
            Ok(()) => Ok(()),
            Err(mpsc::TrySendError::Full(_)) => {
                self.outbound_faulted.store(true, Ordering::Release);
                Err(ProcessError::OutboundQueueOverflow)
            }
            Err(mpsc::TrySendError::Disconnected(_)) => Err(ProcessError::OutboundDisconnected),
        }
    }

    pub(super) fn send_latest(
        &self,
        key: &str,
        envelope: &ProtocolEnvelope,
    ) -> Result<(), ProcessError> {
        if key.is_empty() || key.len() > 256 || key.chars().any(char::is_control) {
            return Err(ProcessError::InvalidCoalescingKey);
        }
        if self.outbound_faulted.load(Ordering::Acquire) {
            return Err(ProcessError::OutboundQueueOverflow);
        }
        if self.writer_failed.load(Ordering::Acquire) {
            return Err(ProcessError::OutboundDisconnected);
        }
        let bytes = self.encode(envelope)?;
        {
            let mut latest = self
                .latest
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            if latest.contains_key(key) {
                self.coalesced_outbound.fetch_add(1, Ordering::AcqRel);
            } else if latest.len() >= self.maximum_coalesced_keys {
                self.outbound_faulted.store(true, Ordering::Release);
                return Err(ProcessError::OutboundQueueOverflow);
            }
            latest.insert(key.to_owned(), bytes);
        }

        match self
            .outbound
            .as_ref()
            .ok_or(ProcessError::OutboundDisconnected)?
            .try_send(OutboundItem::FlushLatest)
        {
            Ok(()) | Err(mpsc::TrySendError::Full(_)) => Ok(()),
            Err(mpsc::TrySendError::Disconnected(_)) => Err(ProcessError::OutboundDisconnected),
        }
    }

    pub(super) fn constrain_maximum_message_bytes(
        &self,
        maximum: usize,
    ) -> Result<(), ProcessError> {
        if maximum == 0 {
            return Err(ProcessError::InvalidLimit);
        }
        self.maximum_message_bytes
            .fetch_min(maximum, Ordering::AcqRel);
        Ok(())
    }

    pub(super) fn receive(&self, timeout: Duration) -> Result<ProtocolEnvelope, ProcessError> {
        if self.inbound_overflow.load(Ordering::Acquire) {
            return Err(ProcessError::QueueOverflow);
        }
        match self.frames.recv_timeout(timeout) {
            Ok(Ok(envelope)) => Ok(envelope),
            Ok(Err(error)) => Err(ProcessError::Framing(error)),
            Err(mpsc::RecvTimeoutError::Timeout) => Err(ProcessError::Timeout),
            Err(mpsc::RecvTimeoutError::Disconnected)
                if self.inbound_overflow.load(Ordering::Acquire) =>
            {
                Err(ProcessError::QueueOverflow)
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => Err(ProcessError::Disconnected),
        }
    }

    pub(super) fn diagnostics(&self) -> ProcessDiagnostics {
        let mut diagnostics = self
            .diagnostics
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone();
        diagnostics.coalesced_outbound = self.coalesced_outbound.load(Ordering::Acquire);
        diagnostics
    }

    pub(super) fn close_outbound(&mut self) {
        self.outbound.take();
    }

    pub(super) fn reap_finished(&mut self) -> Result<(), ProcessError> {
        join_if_finished(&mut self.stdin_writer)?;
        join_if_finished(&mut self.stdout_reader)?;
        join_if_finished(&mut self.stderr_reader)?;
        Ok(())
    }

    pub(super) fn finish_until(&mut self, deadline: Instant) -> Result<(), ProcessError> {
        self.close_outbound();
        loop {
            self.reap_finished()?;
            if self.stdin_writer.is_none()
                && self.stdout_reader.is_none()
                && self.stderr_reader.is_none()
            {
                return Ok(());
            }
            if Instant::now() >= deadline {
                return Err(ProcessError::IoShutdownTimeout);
            }
            thread::sleep(Duration::from_millis(2));
        }
    }

    fn encode(&self, envelope: &ProtocolEnvelope) -> Result<Vec<u8>, ProcessError> {
        let mut bytes = Vec::new();
        write_envelope(
            &mut bytes,
            envelope,
            self.maximum_message_bytes.load(Ordering::Acquire),
        )?;
        Ok(bytes)
    }
}

fn write_frame(stdin: &mut ChildStdin, bytes: &[u8]) -> Result<(), std::io::Error> {
    stdin.write_all(bytes)?;
    stdin.flush()
}

fn join_if_finished(handle: &mut Option<thread::JoinHandle<()>>) -> Result<(), ProcessError> {
    if handle.as_ref().is_some_and(thread::JoinHandle::is_finished) {
        let Some(finished) = handle.take() else {
            return Ok(());
        };
        finished
            .join()
            .map_err(|_| ProcessError::IoThreadPanicked)?;
    }
    Ok(())
}
