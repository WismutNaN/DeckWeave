// SPDX-License-Identifier: MIT OR Apache-2.0
import assert from "node:assert/strict";
import fs from "node:fs";
import { test } from "node:test";
import {
  decodeLengthDelimited,
  encodeLengthDelimited,
  FramingError,
} from "../src/framing.js";

function sharedFixture(): Uint8Array {
  const hex = fs
    .readFileSync(
      new URL("../../fixtures/wire/heartbeat-envelope.hex", import.meta.url),
      "utf8",
    )
    .trim();
  return Uint8Array.from(hex.match(/.{2}/g) ?? [], (pair) =>
    Number.parseInt(pair, 16),
  );
}

test("TypeScript framing round-trips the shared wire fixture", () => {
  const payload = sharedFixture();
  assert.deepEqual(
    decodeLengthDelimited(encodeLengthDelimited(payload, 1024), 1024),
    payload,
  );
});

test("TypeScript framing rejects oversized and malformed frames", () => {
  assert.throws(() => encodeLengthDelimited(sharedFixture(), 1), FramingError);
  assert.throws(
    () => decodeLengthDelimited(Uint8Array.from([0x80, 0x80]), 1024),
    FramingError,
  );
});
