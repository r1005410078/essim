/** u64版本和事件序号跨IPC保持十进制字符串。 */
export type Revision = string;
export type Role = "master" | "slave";
export type Category = "measurement" | "indication" | "control" | "adjustment";
export type ValueType =
  | "bool"
  | "i16"
  | "u16"
  | "i32"
  | "u32"
  | "i64"
  | "u64"
  | "f32"
  | "f64";
export type RawValue =
  | { kind: "bool"; value: boolean }
  | { kind: "signed" | "unsigned" | "float"; value: string };
export interface PointDefinition {
  id: string;
  name: string;
  category: Category;
  area: "holding" | "input" | "coil" | "discrete";
  address: number;
  valueType: ValueType;
  byteOrder: "big" | "little";
  wordOrder: "big" | "little";
  note: string;
  initialEngineering: string;
  scale: string;
  unit: string;
  initial: RawValue;
  slaveWritable: boolean;
  masterPeriodMs: number;
}
export interface CollectionModel {
  id: string;
  name: string;
  version: Revision;
  points: PointDefinition[];
}
export interface ModelRef {
  id: string;
  version: Revision;
}
export interface Device {
  id: string;
  name: string;
  kind: string;
}
export type Transport =
  | { kind: "tcp"; address: string; port: number }
  | {
      kind: "rtu";
      path: string;
      baud: number;
      parity: "none" | "even" | "odd";
      dataBits: 8;
      stopBits: 1 | 2;
    };
export interface UnitConfig {
  id: string;
  name: string;
  station: number;
  enabled: boolean;
  model: ModelRef;
  device: string | null;
}
export interface EndpointConfig {
  id: string;
  name: string;
  revision: Revision;
  appliedRevision: Revision | null;
  role: Role;
  transport: Transport;
  units: UnitConfig[];
}
export interface UnitValues {
  model: ModelRef | null;
  raw: Record<string, RawValue>;
  stale: boolean;
}
export interface Catalog {
  models: CollectionModel[];
  endpoints: EndpointConfig[];
  devices: Device[];
  applied: EndpointConfig[];
  values: Record<string, UnitValues>;
}
export interface Capabilities {
  configuration: boolean;
  execution: boolean;
  recording: boolean;
  replay: boolean;
  sessionOnly: boolean;
  reason: string;
}
export interface WorkspaceView {
  applicationStates?: Record<string, "stopped">;
  sequence: Revision;
  catalog: Catalog;
  capabilities: Capabilities;
}
export type Action =
  | {
      kind: "copyModel";
      source: ModelRef;
      id: string;
      name: string;
      expectedRevision: Revision;
    }
  | { kind: "deleteModel"; id: string; expectedRevision: Revision }
  | { kind: "saveModel"; model: CollectionModel; expectedRevision: Revision }
  | {
      kind: "saveEndpoint";
      endpoint: EndpointConfig;
      expectedRevision: Revision;
    }
  | { kind: "createDevice"; device: Device }
  | {
      kind: "apply";
      id: string;
      expectedRevision: Revision;
      resetIncompatible: boolean;
    }
  | { kind: "start" | "readRemote" | "writeRemote"; id: string }
  | { kind: "replay"; run: string };
export interface Command {
  commandId: string;
  action: Action;
}
export interface Receipt {
  objectId: string;
  revision: Revision;
  sequence: Revision;
}
export interface ApiError {
  code: string;
  message: string;
  pointId?: string;
  field?: string;
  relatedPoint?: string;
}
export interface DraftInput {
  id: string;
  name: string;
  version: Revision;
  points: Record<string, string>[];
}
export interface DraftPreview {
  model: CollectionModel | null;
  errors: ApiError[];
  previews: {
    pointId: string;
    raw: RawValue;
    engineering: string;
    registers: number[];
  }[];
}
export interface ApplyPreview {
  endpoint: EndpointConfig;
  units: Record<
    string,
    {
      values: UnitValues;
      scaleChanged: string[];
      engineeringChanged: { point: string; previous: string; next: string }[];
      initialized: string[];
      removed: string[];
    }
  >;
}
export interface PointFile {
  name: string;
  sheets: { name: string; headers: string[]; rows: string[][] }[];
}
export interface WorkspaceClient {
  query(): Promise<WorkspaceView>;
  execute(command: Command): Promise<Receipt>;
  previewPoints?(input: DraftInput): Promise<DraftPreview>;
  previewApply?(
    id: string,
    revision: Revision,
    reset: boolean,
  ): Promise<ApplyPreview>;
  openPointFile?(signal?: AbortSignal): Promise<PointFile | null>;
  exportPointTemplate?(format: string): Promise<boolean>;
  exportPointFile?(
    id: string,
    version: Revision,
    format: string,
  ): Promise<boolean>;
  subscribe(
    listener: (view: WorkspaceView) => void,
  ): Promise<() => Promise<void>>;
}
/** 完整快照允许合并中间通知；忽略旧快照且不转换成Number。 */
export function newerView(
  current: WorkspaceView | null,
  next: WorkspaceView,
): boolean {
  return current === null || BigInt(next.sequence) > BigInt(current.sequence);
}
