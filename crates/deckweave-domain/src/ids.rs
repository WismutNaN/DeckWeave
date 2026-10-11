// SPDX-License-Identifier: GPL-3.0-or-later
use serde::{Deserialize, Deserializer, Serialize};
use std::{fmt, str::FromStr};
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum IdError {
    #[error("{kind} не может быть пустым")]
    Empty { kind: &'static str },
    #[error("{kind} длиннее 128 байт")]
    TooLong { kind: &'static str },
    #[error("{kind} содержит недопустимые символы: {value}")]
    InvalidCharacters { kind: &'static str, value: String },
    #[error("{kind} должен быть namespaced: {value}")]
    NotNamespaced { kind: &'static str, value: String },
    #[error("{kind} должен быть больше нуля")]
    Zero { kind: &'static str },
    #[error("{kind} должен быть SHA-256 в hex")]
    InvalidDigest { kind: &'static str },
}

fn validate_text(kind: &'static str, value: &str) -> Result<(), IdError> {
    if value.is_empty() {
        return Err(IdError::Empty { kind });
    }
    if value.len() > 128 {
        return Err(IdError::TooLong { kind });
    }
    if !value
        .bytes()
        .all(|byte| byte.is_ascii_alphanumeric() || b"._-:/".contains(&byte))
    {
        return Err(IdError::InvalidCharacters {
            kind,
            value: value.into(),
        });
    }
    Ok(())
}

fn validate_namespaced(kind: &'static str, value: &str) -> Result<(), IdError> {
    validate_text(kind, value)?;
    let segments: Vec<_> = value.split('.').collect();
    if segments.len() < 2
        || segments.iter().any(|segment| {
            let mut bytes = segment.bytes();
            !bytes
                .next()
                .is_some_and(|byte| byte.is_ascii_alphanumeric())
                || !bytes.all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
        })
    {
        return Err(IdError::NotNamespaced {
            kind,
            value: value.into(),
        });
    }
    Ok(())
}

macro_rules! text_id {
    ($name:ident) => {
        #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
        #[serde(transparent)]
        pub struct $name(String);

        impl $name {
            pub fn try_new(value: impl Into<String>) -> Result<Self, IdError> {
                let value = value.into();
                validate_text(stringify!($name), &value)?;
                Ok(Self(value))
            }
            #[must_use]
            pub fn as_str(&self) -> &str {
                &self.0
            }
        }
        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(&self.0)
            }
        }
        impl FromStr for $name {
            type Err = IdError;
            fn from_str(value: &str) -> Result<Self, Self::Err> {
                Self::try_new(value)
            }
        }
        impl<'de> Deserialize<'de> for $name {
            fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
                let value = String::deserialize(deserializer)?;
                Self::try_new(value).map_err(serde::de::Error::custom)
            }
        }
    };
}

macro_rules! namespaced_id {
    ($name:ident) => {
        #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
        #[serde(transparent)]
        pub struct $name(String);

        impl $name {
            pub fn try_new(value: impl Into<String>) -> Result<Self, IdError> {
                let value = value.into();
                validate_namespaced(stringify!($name), &value)?;
                Ok(Self(value))
            }
            #[must_use]
            pub fn as_str(&self) -> &str {
                &self.0
            }
        }
        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(&self.0)
            }
        }
        impl FromStr for $name {
            type Err = IdError;
            fn from_str(value: &str) -> Result<Self, Self::Err> {
                Self::try_new(value)
            }
        }
        impl<'de> Deserialize<'de> for $name {
            fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
                let value = String::deserialize(deserializer)?;
                Self::try_new(value).map_err(serde::de::Error::custom)
            }
        }
    };
}

macro_rules! counter_id {
    ($name:ident) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
        #[serde(transparent)]
        pub struct $name(u64);
        impl $name {
            pub fn try_new(value: u64) -> Result<Self, IdError> {
                if value == 0 {
                    return Err(IdError::Zero {
                        kind: stringify!($name),
                    });
                }
                Ok(Self(value))
            }
            #[must_use]
            pub fn get(self) -> u64 {
                self.0
            }
        }
        impl<'de> Deserialize<'de> for $name {
            fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
                let value = u64::deserialize(deserializer)?;
                Self::try_new(value).map_err(serde::de::Error::custom)
            }
        }
    };
}

text_id!(WorkspaceId);
text_id!(WorkspaceRevisionId);
text_id!(HostBootId);
namespaced_id!(PackageId);
text_id!(ContributionId);
text_id!(ProviderId);
text_id!(PrincipalId);
text_id!(NodeId);
text_id!(DeviceIdentity);
text_id!(ServiceId);
text_id!(DeviceSessionId);
text_id!(PluginSessionId);
text_id!(ElementId);
text_id!(CapabilityId);
text_id!(PortId);
text_id!(GraphNodeId);
text_id!(ExecutionId);
text_id!(InvocationId);
text_id!(JobId);
text_id!(ClockDomainId);
text_id!(TraceId);
text_id!(CorrelationId);
text_id!(CausationId);
text_id!(RuntimeArtifactId);
text_id!(OperationId);
text_id!(ArtifactId);
text_id!(RuntimeGroupId);
text_id!(RuntimeLeaseId);
text_id!(ResourceLeaseId);
text_id!(GrantId);
text_id!(CredentialRef);
text_id!(CredentialLeaseId);
text_id!(LightUnitId);
text_id!(LightGroupId);

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(transparent)]
pub struct AssetId(String);

impl AssetId {
    pub fn try_new(value: impl Into<String>) -> Result<Self, IdError> {
        let value = value.into().to_ascii_lowercase();
        if value.len() != 64 || !value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return Err(IdError::InvalidDigest { kind: "AssetId" });
        }
        Ok(Self(value))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for AssetId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl<'de> Deserialize<'de> for AssetId {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let value = String::deserialize(deserializer)?;
        Self::try_new(value).map_err(serde::de::Error::custom)
    }
}

counter_id!(DraftVersion);
counter_id!(ActivationEpoch);
counter_id!(IngressOrdinal);
counter_id!(SessionGeneration);
counter_id!(DescriptorRevision);
counter_id!(RegistryRevision);
counter_id!(GrantRevision);

/// RunId is the workflow name for the same root identifier, not a second ID.
pub type RunId = ExecutionId;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(transparent)]
pub struct NamespacedKind(String);

impl NamespacedKind {
    pub fn try_new(value: impl Into<String>) -> Result<Self, IdError> {
        let value = value.into();
        validate_namespaced("NamespacedKind", &value)?;
        Ok(Self(value))
    }
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for NamespacedKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl<'de> Deserialize<'de> for NamespacedKind {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let value = String::deserialize(deserializer)?;
        Self::try_new(value).map_err(serde::de::Error::custom)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    proptest! {
        #[test]
        fn ids_never_accept_whitespace(value in ".*\\s+.*") {
            prop_assert!(PortId::try_new(value).is_err());
        }
    }

    #[test]
    fn namespaced_kind_requires_at_least_two_segments() {
        assert!(NamespacedKind::try_new("button.digital").is_ok());
        assert!(NamespacedKind::try_new("button").is_err());
        assert!(NamespacedKind::try_new("button..digital").is_err());
        assert!(NamespacedKind::try_new("button/digital.kind").is_err());
        assert!(PackageId::try_new("org.deckweave.fixture").is_ok());
        assert!(PackageId::try_new("not-namespaced").is_err());
    }
}
