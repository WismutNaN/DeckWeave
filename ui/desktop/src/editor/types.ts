// SPDX-License-Identifier: GPL-3.0-or-later
export type Json =
  null | boolean | number | string | Json[] | { [key: string]: Json };
export type SchemaRef = {
  id: string;
  version: { major: number; minor: number };
};
export type Shape =
  | { type: "null" | "boolean" }
  | { type: "number"; minimum: number; maximum: number }
  | { type: "string"; maximumBytes: number; choices?: string[] }
  | {
      type: "record";
      fields: Record<string, { schema: Shape; optional?: boolean }>;
    }
  | { type: "array"; items: Shape; maximumItems: number }
  | { type: "nullable"; inner: Shape };
export type Schema = {
  schema: SchemaRef;
  shape:
    "unit" | "boolean" | "number" | "string" | "json" | { structured: Shape };
  unit: string | null;
  range: { minimum: number; maximum: number } | null;
};
export type Port = {
  portId: string;
  kind: string;
  schema: SchemaRef;
  availability: { status: string; reason?: string };
  tags?: string[];
  unit?: string;
  range?: { minimum: number; maximum: number };
};
export type Element = {
  elementId: string;
  label?: string;
  capabilities: { capabilityId: string; kind: string; ports: Port[] }[];
};
export type Bounds = { x: number; y: number; width: number; height: number };
export type Device = {
  identity: string;
  providerId: string;
  kind: "physical" | "virtual";
  label: string;
  elements: Element[];
  "org.deckweave.controlLayout"?: {
    version: number;
    width: number;
    height: number;
    elements: Record<string, Bounds>;
  };
};
export type Node = {
  nodeId: string;
  label?: string;
  descriptorRevision: number;
  devices: Device[];
};
export type PackageRef = {
  packageId: string;
  version: string;
  artifactSha256: string;
};
export type PackageLock = PackageRef & {
  source?: string | null;
  [key: string]: unknown;
};
export type Endpoint = {
  packageId: string;
  nodeId: string;
  target:
    | { kind: "device"; providerId: string; deviceIdentity: string }
    | { kind: "service"; providerId: string; serviceId: string };
  port: { elementId: string; capabilityId: string; portId: string };
};
export type CatalogEndpoint = {
  endpoint: Endpoint;
  contributionId: string;
  runtimeGroupId: string;
  port: Port;
  operation: null | {
    operationId: string;
    inputSchema: SchemaRef;
    resultSchema: SchemaRef;
    errorSchema: SchemaRef;
  };
};
export type Catalog = {
  instanceTemplates?: InstanceTemplate[];
  package: PackageRef;
  manifestSha256: string;
  schemas: Schema[];
  endpoints: CatalogEndpoint[];
  translations: {
    defaultLocale: string;
    messages: Record<string, Record<string, string>>;
  } | null;
};
export type InstanceTemplate = {
  contributionId: string;
  settingsSchema: SchemaRef;
  labelKey: string;
  requiredFeatures: Record<string, number>;
};
export type DraftDiagnostic = {
  code: string;
  nodeId: string | null;
  field: string | null;
};
export type ApplyReview = {
  draftVersion: number;
  expectedActive: ActiveSelection | null;
  before: GraphDocument | null;
  document: GraphDocument;
  requiresDisruption: boolean;
  consent: boolean;
};
export type Action = CatalogEndpoint & { catalog: Catalog; title: string };
export type ActiveSelection = { workspaceId: string; revisionId: string };
export type Snapshot = {
  registryRevision: number;
  installed: { package: PackageRef; templates: Node[] }[];
  nodes: { packageId: string; generation: number; descriptor: Node }[];
  runtimes: { packageId: string; status: string; detail?: string }[];
  active: ActiveSelection | null;
};
export type Binding =
  | { type: "constant"; value: Json }
  | { type: "trigger"; pointer: string }
  | { type: "state" | "run_result"; nodeId: string; pointer: string };
export type GraphPort = {
  id: string;
  kind: "flow" | "value";
  role: string;
  schema: SchemaRef;
  unit: string | null;
  range: { minimum: number; maximum: number } | null;
};
export type GraphNode = {
  id: string;
  kind: {
    type: string;
    endpoint?: Endpoint;
    operation_id?: string;
    availability?: string;
    operator?: string;
    operand?: Json;
  };
  inputs: GraphPort[];
  outputs: GraphPort[];
  extensions: Record<string, Json>;
};
export type GraphEdge = {
  id: string;
  kind: "flow" | "data";
  source: { nodeId: string; portId: string };
  target: { nodeId: string; portId: string };
  extensions: Record<string, Json>;
};
export type GraphDocument = {
  schemaVersion: { major: number; minor: number };
  graph: {
    nodes: GraphNode[];
    edges: GraphEdge[];
    actionArguments?: Record<
      string,
      { triggerPort: string; fields: Record<string, Binding> }
    >;
    nodeInstances?: Record<string, string>;
    extensions: Record<string, Json>;
  };
  layout: {
    nodes: Record<string, { x: number; y: number; collapsed: boolean }>;
    extensions: Record<string, Json>;
  };
  pluginInstances?: Record<
    string,
    {
      instanceId: string;
      packageId: string;
      contributionId: string;
      settingsRevision: number;
      settingsSchema: SchemaRef;
      settings: Json;
      [key: string]: unknown;
    }
  >;
  extensions: Record<string, Json>;
};
export type Draft = {
  draftId: string;
  workspaceId: string;
  version: number;
  baseRevisionId: string | null;
  packageLock: PackageLock[];
  document: GraphDocument;
};
export type Background = {
  mode: "solid" | "gradient";
  start: string;
  end: string;
  angle: number;
  brightness: number;
};
export type ControlStyle = {
  trigger?: Endpoint;
  when?: "true" | "false" | "any";
  label?: string;
  background?: Background;
  foreground?: string;
};
export type EditorDevice = {
  id: string;
  package: PackageRef;
  nodeId: string;
  descriptor: Device;
  x: number;
  y: number;
  appearance: Background;
  controls: Record<string, ControlStyle>;
};
export type EditorLayout = { version: 1; devices: EditorDevice[] };
export type ApiError = {
  code: string;
  messageKey: string;
  details: Record<string, Json>;
  traceId: string;
  retryable: boolean;
};
export type MutationEnvelope = {
  requestId: string;
  idempotencyKey: string;
  dedupEpoch: string;
  dryRun: boolean;
  command: Record<string, unknown>;
};
export type Locale = {
  revision: number;
  requestedLocale: string;
  timeZone: string;
};
export type Plan = {
  planHash: string;
  draftVersion: number;
  requiresDisruption: boolean;
  disruptionAllowed: boolean;
};
export type Operation = {
  intent: {
    operationId: string;
    candidate: ActiveSelection;
    draftVersion: number;
  };
  commitStatus: string;
  state: string;
  runtimeHealth: string;
  [key: string]: unknown;
};
