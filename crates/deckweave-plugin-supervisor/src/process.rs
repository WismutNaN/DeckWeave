// SPDX-License-Identifier: GPL-3.0-or-later
use crate::package::{IndexedPackage, PackageError};
use deckweave_application::{ChildProcessGuard, ChildProcessPolicy, PlatformError};
use deckweave_domain::ids::RuntimeGroupId;
use deckweave_protocol::{
    framing::{FramingError, read_envelope, write_envelope},
    v1::ProtocolEnvelope,
};
use std::{
    collections::BTreeMap,
    io::Read,
    process::{Child, ChildStdin, Command, Stdio},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicUsize, Ordering},
        mpsc,
    },
    thread,
    time::Duration,
};
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProcessConfig {
    pub maximum_message_bytes: usize,
    pub maximum_stderr_bytes: usize,
    pub maximum_artifact_bytes: u64,
}

impl Default for ProcessConfig {
    fn default() -> Self {
        Self {
            maximum_message_bytes: 1024 * 1024,
            maximum_stderr_bytes: 64 * 1024,
            maximum_artifact_bytes: 128 * 1024 * 1024,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProcessDiagnostics {
    pub stderr: String,
    pub truncated: bool,
}

#[derive(Debug, Error)]
pub enum ProcessError {
    #[error("failed to launch plugin process: {0}")]
    Spawn(#[source] std::io::Error),
    #[error("plugin stdin/stdout pipe unavailable")]
    MissingPipe,
    #[error("plugin process I/O: {0}")]
    Framing(#[from] FramingError),
    #[error("plugin did not produce a frame before deadline")]
    Timeout,
    #[error("plugin stdout closed")]
    Disconnected,
    #[error("plugin process wait/kill failed: {0}")]
    Process(#[source] std::io::Error),
    #[error("plugin I/O reader thread panicked")]
    ReaderPanicked,
    #[error("plugin stdout exceeded the bounded control queue")]
    QueueOverflow,
    #[error("system random source unavailable: {0}")]
    Random(String),
    #[error("platform process guard unavailable: {0}")]
    Platform(#[from] PlatformError),
    #[error("invalid indexed package: {0}")]
    Package(#[from] PackageError),
    #[error("negotiated message limit must be greater than zero")]
    InvalidLimit,
}

pub struct ProcessRuntime {
    child: Child,
    process_guard: Box<dyn ChildProcessGuard>,
    stdin: ChildStdin,
    frames: mpsc::Receiver<Result<ProtocolEnvelope, FramingError>>,
    diagnostics: Arc<Mutex<ProcessDiagnostics>>,
    maximum_message_bytes: Arc<AtomicUsize>,
    stdout_reader: Option<thread::JoinHandle<()>>,
    stderr_reader: Option<thread::JoinHandle<()>>,
    queue_overflow: Arc<AtomicBool>,
}

impl ProcessRuntime {
    pub fn spawn_indexed(
        package: &IndexedPackage,
        runtime_group_id: &RuntimeGroupId,
        one_time_token: &[u8],
        session_generation: u64,
        config: ProcessConfig,
        extra_environment: &BTreeMap<String, String>,
        process_policy: &dyn ChildProcessPolicy,
    ) -> Result<Self, ProcessError> {
        let artifact = package.verify_artifact_for_current_platform(
            runtime_group_id,
            config.maximum_artifact_bytes,
        )?;
        let mut command = Command::new(&artifact.entrypoint);
        for (key, value) in extra_environment {
            command.env(key, value);
        }
        command
            .current_dir(&package.package_root)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .env("DECKWEAVE_PACKAGE_ID", package.package_id.as_str())
            .env("DECKWEAVE_RUNTIME_GROUP", runtime_group_id.as_str())
            .env("DECKWEAVE_ONE_TIME_TOKEN", encode_hex(one_time_token))
            .env(
                "DECKWEAVE_SESSION_GENERATION",
                session_generation.to_string(),
            )
            .env(
                "DECKWEAVE_MAXIMUM_MESSAGE_BYTES",
                config.maximum_message_bytes.to_string(),
            );
        process_policy.configure(&mut command)?;
        let mut child = command.spawn().map_err(ProcessError::Spawn)?;
        let process_guard = match process_policy.attach(&mut child) {
            Ok(guard) => guard,
            Err(error) => {
                let _ignored = child.kill();
                let _ignored = child.wait();
                return Err(ProcessError::Platform(error));
            }
        };
        let stdin = child.stdin.take().ok_or(ProcessError::MissingPipe)?;
        let mut stdout = child.stdout.take().ok_or(ProcessError::MissingPipe)?;
        let mut stderr = child.stderr.take().ok_or(ProcessError::MissingPipe)?;

        let (sender, frames) = mpsc::sync_channel(16);
        let queue_overflow = Arc::new(AtomicBool::new(false));
        let reader_overflow = Arc::clone(&queue_overflow);
        let maximum_message_bytes = Arc::new(AtomicUsize::new(config.maximum_message_bytes));
        let reader_limit = Arc::clone(&maximum_message_bytes);
        let stdout_reader = thread::Builder::new()
            .name("deckweave-plugin-stdout".into())
            .spawn(move || {
                loop {
                    match read_envelope(&mut stdout, reader_limit.load(Ordering::Acquire)) {
                        Ok(Some(envelope)) => {
                            if let Err(error) = sender.try_send(Ok(envelope)) {
                                if matches!(error, mpsc::TrySendError::Full(_)) {
                                    reader_overflow.store(true, Ordering::Release);
                                }
                                break;
                            }
                        }
                        Ok(None) => break,
                        Err(error) => {
                            if let Err(send_error) = sender.try_send(Err(error))
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

        Ok(Self {
            child,
            process_guard,
            stdin,
            frames,
            diagnostics,
            maximum_message_bytes,
            stdout_reader: Some(stdout_reader),
            stderr_reader: Some(stderr_reader),
            queue_overflow,
        })
    }

    pub fn send(&mut self, envelope: &ProtocolEnvelope) -> Result<(), ProcessError> {
        write_envelope(
            &mut self.stdin,
            envelope,
            self.maximum_message_bytes.load(Ordering::Acquire),
        )?;
        Ok(())
    }

    /// Applies the negotiated effective limit to all subsequent inbound and
    /// outbound frames. Negotiation may only tighten the host launch limit.
    pub fn constrain_maximum_message_bytes(&self, maximum: usize) -> Result<(), ProcessError> {
        if maximum == 0 {
            return Err(ProcessError::InvalidLimit);
        }
        self.maximum_message_bytes
            .fetch_min(maximum, Ordering::AcqRel);
        Ok(())
    }

    pub fn receive(&self, timeout: Duration) -> Result<ProtocolEnvelope, ProcessError> {
        match self.frames.recv_timeout(timeout) {
            Ok(Ok(envelope)) => Ok(envelope),
            Ok(Err(error)) => Err(ProcessError::Framing(error)),
            Err(mpsc::RecvTimeoutError::Timeout) => Err(ProcessError::Timeout),
            Err(mpsc::RecvTimeoutError::Disconnected)
                if self.queue_overflow.load(Ordering::Acquire) =>
            {
                Err(ProcessError::QueueOverflow)
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => Err(ProcessError::Disconnected),
        }
    }

    pub fn try_wait(&mut self) -> Result<Option<std::process::ExitStatus>, ProcessError> {
        let status = self.child.try_wait().map_err(ProcessError::Process)?;
        if status.is_some() {
            self.join_io_readers()?;
        }
        Ok(status)
    }

    pub fn wait_timeout(
        &mut self,
        timeout: Duration,
    ) -> Result<Option<std::process::ExitStatus>, ProcessError> {
        let deadline = std::time::Instant::now() + timeout;
        loop {
            if let Some(status) = self.try_wait()? {
                return Ok(Some(status));
            }
            if std::time::Instant::now() >= deadline {
                return Ok(None);
            }
            thread::sleep(Duration::from_millis(5));
        }
    }

    pub fn terminate(&mut self) -> Result<std::process::ExitStatus, ProcessError> {
        self.process_guard.terminate_tree()?;
        let status = self.child.wait().map_err(ProcessError::Process)?;
        self.join_io_readers()?;
        Ok(status)
    }

    #[must_use]
    pub fn diagnostics(&self) -> ProcessDiagnostics {
        self.diagnostics
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
    }

    pub fn random_token() -> Result<Vec<u8>, ProcessError> {
        let mut token = vec![0_u8; 32];
        getrandom::fill(&mut token).map_err(|error| ProcessError::Random(error.to_string()))?;
        Ok(token)
    }

    fn join_io_readers(&mut self) -> Result<(), ProcessError> {
        if let Some(reader) = self.stdout_reader.take() {
            reader.join().map_err(|_| ProcessError::ReaderPanicked)?;
        }
        if let Some(reader) = self.stderr_reader.take() {
            reader.join().map_err(|_| ProcessError::ReaderPanicked)?;
        }
        Ok(())
    }
}

impl Drop for ProcessRuntime {
    fn drop(&mut self) {
        let _ignored = self.process_guard.terminate_tree();
        let _ignored = self.child.wait();
        let _ignored = self.join_io_readers();
    }
}

fn encode_hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut encoded = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        encoded.push(char::from(HEX[usize::from(byte >> 4)]));
        encoded.push(char::from(HEX[usize::from(byte & 0x0f)]));
    }
    encoded
}
