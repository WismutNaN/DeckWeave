// SPDX-License-Identifier: MIT OR Apache-2.0
//! Bounded synchronous framing helpers for native control SDKs.

use crate::v1::ProtocolEnvelope;
use prost::Message;
use std::io::{self, Read, Write};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum FramingError {
    #[error("I/O error: {0}")]
    Io(#[from] io::Error),
    #[error("malformed length-delimited varint prefix")]
    InvalidLengthPrefix,
    #[error("control frame exceeds negotiated limit: {actual} > {limit}")]
    TooLarge { actual: u64, limit: usize },
    #[error("invalid Protocol Buffer: {0}")]
    Decode(#[from] prost::DecodeError),
}

pub fn write_envelope(
    writer: &mut impl Write,
    envelope: &ProtocolEnvelope,
    maximum_message_bytes: usize,
) -> Result<(), FramingError> {
    let length = envelope.encoded_len();
    if length > maximum_message_bytes {
        return Err(FramingError::TooLarge {
            actual: u64::try_from(length).unwrap_or(u64::MAX),
            limit: maximum_message_bytes,
        });
    }
    write_varint(
        writer,
        u64::try_from(length).map_err(|_| FramingError::TooLarge {
            actual: u64::MAX,
            limit: maximum_message_bytes,
        })?,
    )?;
    let bytes = envelope.encode_to_vec();
    writer.write_all(&bytes)?;
    writer.flush()?;
    Ok(())
}

pub fn read_envelope(
    reader: &mut impl Read,
    maximum_message_bytes: usize,
) -> Result<Option<ProtocolEnvelope>, FramingError> {
    let Some(length) = read_varint(reader)? else {
        return Ok(None);
    };
    if length > u64::try_from(maximum_message_bytes).unwrap_or(u64::MAX) {
        return Err(FramingError::TooLarge {
            actual: length,
            limit: maximum_message_bytes,
        });
    }
    let length = usize::try_from(length).map_err(|_| FramingError::TooLarge {
        actual: u64::MAX,
        limit: maximum_message_bytes,
    })?;
    let mut bytes = vec![0; length];
    reader.read_exact(&mut bytes)?;
    Ok(Some(ProtocolEnvelope::decode(bytes.as_slice())?))
}

fn read_varint(reader: &mut impl Read) -> Result<Option<u64>, FramingError> {
    let mut value = 0_u64;
    for index in 0..10_u32 {
        let mut byte = [0_u8; 1];
        match reader.read_exact(&mut byte) {
            Ok(()) => {}
            Err(error) if error.kind() == io::ErrorKind::UnexpectedEof && index == 0 => {
                return Ok(None);
            }
            Err(error) => return Err(FramingError::Io(error)),
        }
        if index == 9 && byte[0] > 1 {
            return Err(FramingError::InvalidLengthPrefix);
        }
        value |= u64::from(byte[0] & 0x7f) << (index * 7);
        if byte[0] & 0x80 == 0 {
            return Ok(Some(value));
        }
    }
    Err(FramingError::InvalidLengthPrefix)
}

fn write_varint(writer: &mut impl Write, mut value: u64) -> Result<(), io::Error> {
    loop {
        let mut byte = u8::try_from(value & 0x7f).unwrap_or(0x7f);
        value >>= 7;
        if value != 0 {
            byte |= 0x80;
        }
        writer.write_all(&[byte])?;
        if value == 0 {
            return Ok(());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::v1::{Heartbeat, ProtocolVersion, protocol_envelope};
    use std::io::Cursor;

    fn envelope() -> ProtocolEnvelope {
        ProtocolEnvelope {
            wire_version: Some(ProtocolVersion { major: 1, minor: 0 }),
            session_generation: 1,
            message_id: 2,
            trace: None,
            body: Some(protocol_envelope::Body::Heartbeat(Heartbeat {
                host_ticks: 3,
                active_jobs: 0,
            })),
        }
    }

    #[test]
    fn bounded_frame_round_trip() -> Result<(), Box<dyn std::error::Error>> {
        let mut bytes = Vec::new();
        write_envelope(&mut bytes, &envelope(), 1024)?;
        let decoded = read_envelope(&mut Cursor::new(bytes), 1024)?.ok_or("missing frame")?;
        assert_eq!(decoded, envelope());
        Ok(())
    }

    #[test]
    fn oversized_prefix_is_rejected_before_allocation() {
        let bytes = [0x80, 0x80, 0x80, 0x80, 0x10];
        assert!(matches!(
            read_envelope(&mut Cursor::new(bytes), 1024),
            Err(FramingError::TooLarge { .. })
        ));
    }

    #[test]
    fn malformed_ten_byte_prefix_is_rejected() {
        let bytes = [0xff; 10];
        assert!(matches!(
            read_envelope(&mut Cursor::new(bytes), 1024),
            Err(FramingError::InvalidLengthPrefix)
        ));
    }
}
