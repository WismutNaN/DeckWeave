// SPDX-License-Identifier: GPL-3.0-or-later
//! Minimal host-owned effects; the plugin keeps action semantics and presentation.
use serde::{Deserialize, Serialize};
use std::time::Instant;

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ExactLaunch {
    pub executable: String,
    pub arguments: Vec<String>,
    #[serde(default)]
    pub working_directory: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ShellTarget {
    pub target: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UserProcessRequest {
    Launch(ExactLaunch),
    Open(ShellTarget),
}

impl UserProcessRequest {
    pub fn validate(&self) -> Result<(), UserProcessFailure> {
        let text = |s: &str| !s.contains('\0') && s.len() <= 32768;
        match self {
            Self::Launch(v)
                if !v.executable.is_empty()
                    && text(&v.executable)
                    && v.arguments.len() <= 128
                    && v.arguments.iter().all(|v| text(v))
                    && v.working_directory
                        .as_ref()
                        .is_none_or(|v| !v.is_empty() && text(v))
                    && v.arguments.iter().map(|v| v.len()).sum::<usize>() + v.executable.len()
                        <= 32768 =>
            {
                Ok(())
            }
            Self::Open(v) if !v.target.is_empty() && text(&v.target) => Ok(()),
            _ => Err(UserProcessFailure::InvalidInput),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UserProcessReceipt {
    pub process_id: Option<u32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UserProcessFailure {
    InvalidInput,
    Unsupported,
    Rejected,
    Deadline,
    Busy,
    UnknownOutcome,
}

/// Implementations never attach the user's process to the plugin runtime job.
/// Deadline/unknown outcomes must not be retried implicitly.
pub trait UserProcessLauncher: Send + Sync + std::fmt::Debug {
    fn execute(
        &self,
        request: &UserProcessRequest,
        deadline: Instant,
    ) -> Result<UserProcessReceipt, UserProcessFailure>;
}
