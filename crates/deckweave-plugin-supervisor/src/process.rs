// SPDX-License-Identifier: GPL-3.0-or-later
use crate::package::{IndexedPackage, PackageError};
use deckweave_domain::ids::RuntimeGroupId;
use deckweave_platform::{ChildProcessGuard, ChildProcessPolicy, PlatformError};
use deckweave_protocol::{framing::FramingError, v1::ProtocolEnvelope};
use std::{
    collections::BTreeMap,
    process::{Child, Command, Stdio},
    thread,
    time::{Duration, Instant},
};
use thiserror::Error;

mod transport;
use transport::ProcessTransport;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProcessConfig {
    pub maximum_message_bytes: usize,
    pub maximum_stderr_bytes: usize,
    pub maximum_artifact_bytes: u64,
    pub maximum_inbound_frames: usize,
    pub maximum_outbound_frames: usize,
    pub maximum_coalesced_keys: usize,
    pub io_shutdown_timeout: Duration,
}

impl Default for ProcessConfig {
    fn default() -> Self {
        Self {
            maximum_message_bytes: 1024 * 1024,
            maximum_stderr_bytes: 64 * 1024,
            maximum_artifact_bytes: 128 * 1024 * 1024,
            maximum_inbound_frames: 16,
            maximum_outbound_frames: 16,
            maximum_coalesced_keys: 64,
            io_shutdown_timeout: Duration::from_secs(2),
        }
    }
}

impl ProcessConfig {
    fn validate(&self) -> Result<(), ProcessError> {
        if self.maximum_message_bytes == 0 {
            return Err(ProcessError::InvalidLimit);
        }
        if self.maximum_inbound_frames == 0
            || self.maximum_outbound_frames == 0
            || self.maximum_coalesced_keys == 0
        {
            return Err(ProcessError::InvalidQueueCapacity);
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProcessDiagnostics {
    pub stderr: String,
    pub truncated: bool,
    pub coalesced_outbound: usize,
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
    #[error("plugin I/O thread panicked")]
    IoThreadPanicked,
    #[error("plugin stdout exceeded the bounded control queue")]
    QueueOverflow,
    #[error("plugin stdin exceeded the bounded reliable-control queue")]
    OutboundQueueOverflow,
    #[error("plugin stdin writer disconnected")]
    OutboundDisconnected,
    #[error("coalescing key is empty, too long or contains control characters")]
    InvalidCoalescingKey,
    #[error("process I/O queue capacity must be greater than zero")]
    InvalidQueueCapacity,
    #[error("plugin I/O threads did not stop before deadline")]
    IoShutdownTimeout,
    #[error("plugin process did not terminate before deadline")]
    TerminationTimeout,
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
    transport: ProcessTransport,
    io_shutdown_timeout: Duration,
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
        config.validate()?;
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
        let stdout = child.stdout.take().ok_or(ProcessError::MissingPipe)?;
        let stderr = child.stderr.take().ok_or(ProcessError::MissingPipe)?;
        let transport = ProcessTransport::spawn(stdin, stdout, stderr, &config)?;

        Ok(Self {
            child,
            process_guard,
            transport,
            io_shutdown_timeout: config.io_shutdown_timeout,
        })
    }

    pub fn send(&mut self, envelope: &ProtocolEnvelope) -> Result<(), ProcessError> {
        self.send_with_fencing(|transport| transport.send_reliable(envelope))
    }

    /// Sends replaceable data through a bounded latest-value lane. Reusing a
    /// key replaces an unsent value instead of growing the reliable queue.
    pub fn send_latest(
        &mut self,
        key: &str,
        envelope: &ProtocolEnvelope,
    ) -> Result<(), ProcessError> {
        self.send_with_fencing(|transport| transport.send_latest(key, envelope))
    }

    /// Applies the negotiated effective limit to all subsequent inbound and
    /// outbound frames. Negotiation may only tighten the host launch limit.
    pub fn constrain_maximum_message_bytes(&self, maximum: usize) -> Result<(), ProcessError> {
        self.transport.constrain_maximum_message_bytes(maximum)
    }

    pub fn receive(&mut self, timeout: Duration) -> Result<ProtocolEnvelope, ProcessError> {
        match self.transport.receive(timeout) {
            Err(error @ ProcessError::QueueOverflow) => {
                self.process_guard.terminate_tree()?;
                Err(error)
            }
            result => result,
        }
    }

    pub fn try_wait(&mut self) -> Result<Option<std::process::ExitStatus>, ProcessError> {
        let status = self.child.try_wait().map_err(ProcessError::Process)?;
        if status.is_some() {
            self.transport.close_outbound();
            self.process_guard.terminate_tree()?;
            self.transport.reap_finished()?;
        }
        Ok(status)
    }

    pub fn wait_timeout(
        &mut self,
        timeout: Duration,
    ) -> Result<Option<std::process::ExitStatus>, ProcessError> {
        let deadline = Instant::now()
            .checked_add(timeout)
            .unwrap_or_else(Instant::now);
        loop {
            if let Some(status) = self.try_wait()? {
                self.transport.finish_until(deadline)?;
                return Ok(Some(status));
            }
            if Instant::now() >= deadline {
                return Ok(None);
            }
            thread::sleep(Duration::from_millis(5));
        }
    }

    pub fn terminate(&mut self) -> Result<std::process::ExitStatus, ProcessError> {
        self.process_guard.terminate_tree()?;
        self.transport.close_outbound();
        let deadline = Instant::now()
            .checked_add(self.io_shutdown_timeout)
            .unwrap_or_else(Instant::now);
        let status = loop {
            if let Some(status) = self.child.try_wait().map_err(ProcessError::Process)? {
                break status;
            }
            if Instant::now() >= deadline {
                return Err(ProcessError::TerminationTimeout);
            }
            thread::sleep(Duration::from_millis(2));
        };
        self.transport.finish_until(deadline)?;
        Ok(status)
    }

    #[must_use]
    pub fn diagnostics(&self) -> ProcessDiagnostics {
        self.transport.diagnostics()
    }

    pub fn random_token() -> Result<Vec<u8>, ProcessError> {
        let mut token = vec![0_u8; 32];
        getrandom::fill(&mut token).map_err(|error| ProcessError::Random(error.to_string()))?;
        Ok(token)
    }

    fn send_with_fencing(
        &mut self,
        send: impl FnOnce(&ProcessTransport) -> Result<(), ProcessError>,
    ) -> Result<(), ProcessError> {
        match send(&self.transport) {
            Err(
                error @ (ProcessError::OutboundQueueOverflow | ProcessError::OutboundDisconnected),
            ) => {
                self.process_guard.terminate_tree()?;
                Err(error)
            }
            result => result,
        }
    }
}

impl Drop for ProcessRuntime {
    fn drop(&mut self) {
        let _ignored = self.process_guard.terminate_tree();
        self.transport.close_outbound();
        let deadline = Instant::now()
            .checked_add(self.io_shutdown_timeout)
            .unwrap_or_else(Instant::now);
        while Instant::now() < deadline {
            match self.child.try_wait() {
                Ok(Some(_)) | Err(_) => break,
                Ok(None) => thread::sleep(Duration::from_millis(2)),
            }
        }
        let _ignored = self.transport.reap_finished();
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
