// SPDX-License-Identifier: MIT OR Apache-2.0
import assert from "node:assert/strict";
import fs from "node:fs";
import { test } from "node:test";
import { create, fromBinary, toBinary } from "@bufbuild/protobuf";
import {
  EndpointPathSchema,
  ErrorCategory,
  HeartbeatSchema,
  ProtocolEnvelopeSchema,
  ProtocolVersionSchema,
  SignalSchema,
  StructuredErrorSchema,
} from "../src/generated/deckweave/v1/control_pb.js";

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

test("TypeScript reads and writes the shared control envelope", () => {
  const bytes = sharedFixture();
  const decoded = fromBinary(ProtocolEnvelopeSchema, bytes);
  assert.equal(decoded.sessionGeneration, 4n);
  assert.equal(decoded.messageId, 9n);
  assert.equal(decoded.body.case, "heartbeat");
  if (decoded.body.case === "heartbeat")
    assert.equal(decoded.body.value.activeJobs, 1);
  assert.deepEqual(toBinary(ProtocolEnvelopeSchema, decoded), bytes);
});

test("generated binary stays compatible with the shared fixture", () => {
  const envelope = create(ProtocolEnvelopeSchema, {
    wireVersion: create(ProtocolVersionSchema, { major: 1, minor: 2 }),
    sessionGeneration: 4n,
    messageId: 9n,
    body: {
      case: "heartbeat",
      value: create(HeartbeatSchema, { hostTicks: 12n, activeJobs: 1 }),
    },
  });
  assert.deepEqual(toBinary(ProtocolEnvelopeSchema, envelope), sharedFixture());
});

test("optional scalar presence is distinct from an absent value", async () => {
  const { FramePacketSchema } =
    await import("../src/generated/deckweave/v1/control_pb.js");
  const absent = create(FramePacketSchema, {
    targetPortId: "display",
    activationEpoch: 1n,
    descriptorRevision: 1n,
    frameId: 2n,
    chunkCount: 1,
  });
  const present = create(FramePacketSchema, { ...absent, baseFrameId: 0n });
  assert.notDeepEqual(
    toBinary(FramePacketSchema, absent),
    toBinary(FramePacketSchema, present),
  );
  assert.equal(
    fromBinary(FramePacketSchema, toBinary(FramePacketSchema, present))
      .baseFrameId,
    0n,
  );
});

test("signal source preserves the fully scoped endpoint path", () => {
  const signal = create(SignalSchema, {
    deviceSessionGeneration: 3n,
    sequence: 12n,
    capturedTicks: 44n,
    clockDomainId: "host-monotonic",
    source: create(EndpointPathSchema, {
      nodeId: "local",
      providerId: "hid",
      target: { case: "deviceIdentity", value: "keyboard-1" },
      elementId: "key-a",
      capabilityId: "button",
      portId: "pressed",
    }),
  });

  const decoded = fromBinary(SignalSchema, toBinary(SignalSchema, signal));
  assert.equal(decoded.source?.nodeId, "local");
  assert.equal(decoded.source?.providerId, "hid");
  assert.deepEqual(decoded.source?.target, {
    case: "deviceIdentity",
    value: "keyboard-1",
  });
  assert.equal(decoded.source?.elementId, "key-a");
  assert.equal(decoded.source?.capabilityId, "button");
  assert.equal(decoded.source?.portId, "pressed");
});

test("Protobuf-ES retains unknown fields and enum numbers", () => {
  const heartbeatWithUnknown = Uint8Array.from([
    0x08, 0x0c, 0x10, 0x01, 0xf8, 0x07, 0x01,
  ]);
  const heartbeat = fromBinary(HeartbeatSchema, heartbeatWithUnknown);
  assert.deepEqual(toBinary(HeartbeatSchema, heartbeat), heartbeatWithUnknown);

  const error = create(StructuredErrorSchema, {
    category: 31337 as ErrorCategory,
    code: "future",
    messageKey: "future",
  });
  const decoded = fromBinary(
    StructuredErrorSchema,
    toBinary(StructuredErrorSchema, error),
  );
  assert.equal(decoded.category, 31337);
});
