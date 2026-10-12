// SPDX-License-Identifier: MIT OR Apache-2.0
/// Bytes held by a transparent relay. A typed Prost decode/re-encode is not
/// transparent because unknown fields are not retained.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpaqueProtocolFrame(Vec<u8>);

impl OpaqueProtocolFrame {
    #[must_use]
    pub fn new(bytes: Vec<u8>) -> Self {
        Self(bytes)
    }
    #[must_use]
    pub fn as_bytes(&self) -> &[u8] {
        &self.0
    }
    #[must_use]
    pub fn into_bytes(self) -> Vec<u8> {
        self.0
    }
}
