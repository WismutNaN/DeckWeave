// SPDX-License-Identifier: MIT OR Apache-2.0
// Generated from JSON Schema. Do not edit by hand.

export type NamespacedId = string;
export type Semver = string;
export type LocalId = string;
export type PackagePath = string;
export type Sha256 = string;

/**
 * Static package metadata; indexing this document never executes plugin code.
 */
export interface PluginManifest {
  manifestVersion: 1;
  packageId: NamespacedId;
  version: Semver;
  license: string;
  source: {
    repository: string;
    revision: string;
    provenance?: string;
    [k: string]: unknown;
  };
  /**
   * @minItems 1
   */
  artifacts: [
    {
      id: LocalId;
      os: "windows" | "linux" | "macos";
      architecture: "x86_64" | "aarch64";
      entrypoint: PackagePath;
      sha256: Sha256;
      [k: string]: unknown;
    },
    ...{
      id: LocalId;
      os: "windows" | "linux" | "macos";
      architecture: "x86_64" | "aarch64";
      entrypoint: PackagePath;
      sha256: Sha256;
      [k: string]: unknown;
    }[]
  ];
  runtimeGroups: {
    id: LocalId;
    runtimeClass:
      | "isolated_control"
      | "openaction_compatibility"
      | "widget"
      | "trusted_media"
      | "remote_agent"
      | "built_in_audited";
    artifactId: LocalId;
    [k: string]: unknown;
  }[];
  contributions: {
    id: LocalId;
    type: NamespacedId;
    runtimeGroupId: LocalId;
    descriptor: {
      [k: string]: unknown;
    };
    activationConditions?: NamespacedId[];
    [k: string]: unknown;
  }[];
  protocolRange: VersionRange;
  permissions: {
    scope: NamespacedId;
    accessMode: "observe" | "read" | "write" | "execute" | "shared" | "exclusive" | "provision";
    resourceSelector: unknown;
    reason: string;
    optional?: boolean;
    /**
     * @minItems 1
     */
    runtimeGroupIds?: [LocalId, ...LocalId[]];
    [k: string]: unknown;
  }[];
  schemas: {
    id: NamespacedId;
    version: SchemaVersion;
    path: PackagePath;
    sha256: Sha256;
    [k: string]: unknown;
  }[];
  assets?: {
    [k: string]: unknown;
  }[];
  dependencies?: {
    [k: string]: unknown;
  }[];
  migrations?: {
    [k: string]: unknown;
  }[];
  [k: string]: unknown;
}
export interface VersionRange {
  minimum: SchemaVersion;
  maximum: SchemaVersion;
}
export interface SchemaVersion {
  major: number;
  minor: number;
}
