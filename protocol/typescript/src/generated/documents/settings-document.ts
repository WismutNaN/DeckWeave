// SPDX-License-Identifier: MIT OR Apache-2.0
// Generated from JSON Schema. Do not edit by hand.

export interface SettingsDocument {
  schemaId: string;
  schemaVersion: {
    major: number;
    minor: number;
  };
  values: {
    [k: string]: unknown;
  };
  extensions?: {
    [k: string]: unknown;
  };
  [k: string]: unknown;
}
