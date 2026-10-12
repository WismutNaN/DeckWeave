// SPDX-License-Identifier: GPL-3.0-or-later
use crate::{DraftCommand, WorkspaceDraft, validate_draft};
use deckweave_domain::ids::DraftVersion;
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum DraftEditError {
    #[error("draft version conflict: expected {expected}, current {current}")]
    VersionConflict { expected: u64, current: u64 },
    #[error("draft command is invalid: {0}")]
    InvalidCommand(String),
    #[error("draft version counter exhausted")]
    CounterExhausted,
}

pub fn preview_draft_mutation(
    draft: &WorkspaceDraft,
    expected_version: DraftVersion,
    command: &DraftCommand,
) -> Result<WorkspaceDraft, DraftEditError> {
    if draft.version != expected_version {
        return Err(DraftEditError::VersionConflict {
            expected: expected_version.get(),
            current: draft.version.get(),
        });
    }
    let mut next = draft.clone();
    apply_draft_command(&mut next.document, command)?;
    let next_version = next
        .version
        .get()
        .checked_add(1)
        .ok_or(DraftEditError::CounterExhausted)?;
    next.version =
        DraftVersion::try_new(next_version).map_err(|_| DraftEditError::CounterExhausted)?;
    validate_draft(&next).map_err(|error| DraftEditError::InvalidCommand(error.to_string()))?;
    Ok(next)
}

pub fn apply_draft_command(
    document: &mut serde_json::Value,
    command: &DraftCommand,
) -> Result<(), DraftEditError> {
    match command {
        DraftCommand::ReplaceDocument { document: next } => {
            *document = next.clone();
            Ok(())
        }
        DraftCommand::Set { pointer, value } => set_pointer(document, pointer, value.clone()),
        DraftCommand::Remove { pointer } => remove_pointer(document, pointer),
    }
}

fn pointer_tokens(pointer: &str) -> Result<Vec<String>, DraftEditError> {
    if pointer.is_empty() || !pointer.starts_with('/') {
        return Err(DraftEditError::InvalidCommand(
            "JSON pointer must be non-empty and start with '/'".into(),
        ));
    }
    pointer[1..]
        .split('/')
        .map(|token| {
            let mut decoded = String::new();
            let mut chars = token.chars();
            while let Some(character) = chars.next() {
                if character != '~' {
                    decoded.push(character);
                    continue;
                }
                match chars.next() {
                    Some('0') => decoded.push('~'),
                    Some('1') => decoded.push('/'),
                    _ => {
                        return Err(DraftEditError::InvalidCommand(
                            "invalid JSON pointer escape".into(),
                        ));
                    }
                }
            }
            Ok(decoded)
        })
        .collect()
}

fn parent_and_token<'a>(
    document: &'a mut serde_json::Value,
    pointer: &str,
) -> Result<(&'a mut serde_json::Value, String), DraftEditError> {
    let mut tokens = pointer_tokens(pointer)?;
    let token = tokens
        .pop()
        .ok_or_else(|| DraftEditError::InvalidCommand("pointer has no token".into()))?;
    let mut parent = document;
    for segment in tokens {
        parent = match parent {
            serde_json::Value::Object(object) => object.get_mut(&segment),
            serde_json::Value::Array(array) => segment
                .parse::<usize>()
                .ok()
                .and_then(|index| array.get_mut(index)),
            _ => None,
        }
        .ok_or_else(|| DraftEditError::InvalidCommand("pointer parent is missing".into()))?;
    }
    Ok((parent, token))
}

fn set_pointer(
    document: &mut serde_json::Value,
    pointer: &str,
    value: serde_json::Value,
) -> Result<(), DraftEditError> {
    let (parent, token) = parent_and_token(document, pointer)?;
    match parent {
        serde_json::Value::Object(object) => {
            object.insert(token, value);
            Ok(())
        }
        serde_json::Value::Array(array) if token == "-" => {
            array.push(value);
            Ok(())
        }
        serde_json::Value::Array(array) => {
            let index = token
                .parse::<usize>()
                .map_err(|_| DraftEditError::InvalidCommand("array index is invalid".into()))?;
            if index > array.len() {
                return Err(DraftEditError::InvalidCommand(
                    "array index is out of bounds".into(),
                ));
            }
            if index == array.len() {
                array.push(value);
            } else {
                array[index] = value;
            }
            Ok(())
        }
        _ => Err(DraftEditError::InvalidCommand(
            "pointer parent is not a container".into(),
        )),
    }
}

fn remove_pointer(document: &mut serde_json::Value, pointer: &str) -> Result<(), DraftEditError> {
    let (parent, token) = parent_and_token(document, pointer)?;
    match parent {
        serde_json::Value::Object(object) => object
            .remove(&token)
            .map(|_| ())
            .ok_or_else(|| DraftEditError::InvalidCommand("field is missing".into())),
        serde_json::Value::Array(array) => {
            let index = token
                .parse::<usize>()
                .map_err(|_| DraftEditError::InvalidCommand("array index is invalid".into()))?;
            if index >= array.len() {
                return Err(DraftEditError::InvalidCommand(
                    "array index is out of bounds".into(),
                ));
            }
            array.remove(index);
            Ok(())
        }
        _ => Err(DraftEditError::InvalidCommand(
            "pointer parent is not a container".into(),
        )),
    }
}
