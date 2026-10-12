// SPDX-License-Identifier: GPL-3.0-or-later
import assert from "node:assert/strict";
import test from "node:test";

import {
  countLines,
  validateLocalDependencies,
  validateSizeRatchet,
} from "./boundary-rules.mjs";

test("size ratchet accepts a reviewed baseline and rejects growth", () => {
  const input = {
    files: new Map([["crate/src/lib.rs", "one\ntwo\nthree\n"]]),
    crateRoots: new Set(["crate/src/lib.rs"]),
    limits: { crateRootLines: 2, productionModuleLines: 4 },
    exceptions: {
      "crate/src/lib.rs": {
        maxLines: 3,
        owner: "Example context",
        reason: "Characterized legacy root",
        reviewMilestone: "R2",
      },
    },
  };
  assert.deepEqual(validateSizeRatchet(input), []);

  input.files.set("crate/src/lib.rs", "one\ntwo\nthree\nfour\n");
  assert.match(validateSizeRatchet(input).join("\n"), /ratchet вырос 3 -> 4/);
});

test("size exception requires ownership and is removed after extraction", () => {
  const errors = validateSizeRatchet({
    files: new Map([["crate/src/lib.rs", "one\n"]]),
    crateRoots: new Set(["crate/src/lib.rs"]),
    limits: { crateRootLines: 2, productionModuleLines: 4 },
    exceptions: {
      "crate/src/lib.rs": {
        maxLines: 3,
        owner: "",
        reason: "",
        reviewMilestone: "",
      },
    },
  }).join("\n");
  assert.match(errors, /требует owner/);
  assert.match(errors, /удалите size exception/);
});

test("context policy rejects an undeclared local dependency", () => {
  const packages = [
    { name: "domain", dependencies: [] },
    {
      name: "adapter",
      dependencies: [{ name: "domain", kind: null }],
    },
  ];
  const policies = {
    domain: { context: "Domain", allowedDependencies: [] },
    adapter: { context: "Adapter", allowedDependencies: [] },
  };
  assert.match(
    validateLocalDependencies(packages, policies).join("\n"),
    /Adapter.*Domain.*нарушена граница зависимости/,
  );
});

test("line count ignores the trailing newline", () => {
  assert.equal(countLines("one\ntwo\n"), 2);
  assert.equal(countLines("one\r\ntwo"), 2);
  assert.equal(countLines(""), 0);
});
