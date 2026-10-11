// SPDX-License-Identifier: MIT OR Apache-2.0

export class FramingError extends Error {}

export function encodeLengthDelimited(
  payload: Uint8Array,
  maximumBytes: number,
): Uint8Array {
  validateLimit(payload.byteLength, maximumBytes);
  const prefix: number[] = [];
  let remaining = payload.byteLength;
  do {
    let byte = remaining & 0x7f;
    remaining = Math.floor(remaining / 128);
    if (remaining > 0) byte |= 0x80;
    prefix.push(byte);
  } while (remaining > 0);
  const frame = new Uint8Array(prefix.length + payload.byteLength);
  frame.set(prefix);
  frame.set(payload, prefix.length);
  return frame;
}

export function decodeLengthDelimited(
  frame: Uint8Array,
  maximumBytes: number,
): Uint8Array {
  let length = 0;
  let multiplier = 1;
  let prefixBytes = 0;
  for (; prefixBytes < Math.min(frame.byteLength, 10); prefixBytes += 1) {
    const byte = frame[prefixBytes];
    length += (byte & 0x7f) * multiplier;
    if (!Number.isSafeInteger(length))
      throw new FramingError("length prefix exceeds safe integer");
    if ((byte & 0x80) === 0) {
      prefixBytes += 1;
      validateLimit(length, maximumBytes);
      if (frame.byteLength !== prefixBytes + length) {
        throw new FramingError("frame is truncated or contains trailing bytes");
      }
      return frame.slice(prefixBytes);
    }
    multiplier *= 128;
  }
  throw new FramingError("malformed length prefix");
}

function validateLimit(actual: number, maximum: number): void {
  if (!Number.isSafeInteger(maximum) || maximum < 0 || actual > maximum) {
    throw new FramingError(
      `control frame exceeds negotiated limit: ${actual} > ${maximum}`,
    );
  }
}
