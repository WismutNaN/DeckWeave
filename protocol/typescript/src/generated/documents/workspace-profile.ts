// SPDX-License-Identifier: MIT OR Apache-2.0
// Generated from JSON Schema. Do not edit by hand.

export type Id = string;
export type NamespacedId = string;
export type Sha256 = string;

/**
 * Portable profile envelope. Full workspace semantics are introduced with the profile store.
 */
export interface WorkspaceProfile {
  schemaVersion: 1;
  workspaceId: Id;
  revisionId: Id;
  packageLock: {
    packageId: NamespacedId;
    version: string;
    artifactSha256: Sha256;
    source?: string;
    [k: string]: unknown;
  }[];
  document: {
    [k: string]: unknown;
  };
  assets?: {
    id: Id;
    sha256: Sha256;
    mediaType: string;
    sizeBytes: number;
    [k: string]: unknown;
  }[];
  /**
   * Optional separately selected export section; credential values remain forbidden.
   */
  privateSection?: {
    [k: string]: unknown;
  };
  extensions?: {
    [k: string]: unknown;
  };
  [k: string]: unknown;
}
