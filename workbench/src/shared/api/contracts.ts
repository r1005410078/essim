/** 配置通知序号跨IPC保持十进制字符串。 */
export type Sequence = string;
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
  points: PointDefinition[];
}
export interface ModelRef {
  id: string;
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
export type OverrideField = Exclude<keyof PointDefinition,"id"|"initial">;
export interface UnitOverrides { pointOverrides:Record<string,Partial<Pick<PointDefinition,OverrideField>>>; addedPoints:PointDefinition[]; removedPointIds:string[] }
export interface OverrideSelection {fields:Record<string,OverrideField[]>;added:string[];removed:string[]}
export interface PromotionPlan {endpoint:string;unit:string;selection:OverrideSelection;before:CollectionModel;after:CollectionModel;references:{endpoint:string;unit:UnitConfig}[]}
export interface InitialSample {point:string;value:RawValue;engineering:string;unixMs:string}
export interface UnitConfig {
  overrides?:UnitOverrides;
  note?: string;
  id: string;
  name: string;
  station: number;
  enabled: boolean;
  model: ModelRef;
  device: string | null;
}
export interface MasterSettings {connectTimeoutMs:number;requestTimeoutMs:number;reconnectMs:number;staleMultiplier:number}
export interface EndpointConfig {
  master?: MasterSettings;
  note?: string;
  id: string;
  name: string;
  role: Role;
  transport: Transport;
  units: UnitConfig[];
}
export interface UnitValues {
  model: CollectionModel | null;
  raw: Record<string, RawValue>;
  stale: boolean;
}
export interface Folder { id: string; name: string; parent: string | null; }
export interface Catalog {
  navigation?: { folders: Folder[]; members: Record<string, string>; modelMembers?: Record<string,string> };
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
  applicationMatches?: Record<string, Record<string, boolean>>;
  applicationStates?: Record<string, "stopped" | "running" | "releasing" | "failed">;
  sequence: Sequence;
  catalog: Catalog;
  capabilities: Capabilities;
}
export type Action =
  | {kind:"createEndpointWithModel";endpoint:EndpointConfig;model:CollectionModel}
  | {kind:"promoteOverrides";plan:PromotionPlan}
  | {kind:"extractUnitModel";endpoint:string;unit:string;id:string;name:string}
  | { kind: "createEndpoint"; endpoint: EndpointConfig }
  | { kind: "copyEndpoint"; source: string; id: string }
  | { kind: "deleteEndpoint"; id: string }
  | { kind: "resetUnit"; id: string; unit: string }
  | { kind: "saveFolder"; folder: Folder }
  | { kind: "deleteFolder"; id: string }
  | { kind: "moveEndpoint"; id: string; parent: string | null }
  | {
      kind: "copyModel";
      source: ModelRef;
      id: string;
      name: string;
    }
  | { kind: "deleteModel"; id: string }
  | { kind: "saveModel"; model: CollectionModel }
  | {
      kind: "saveEndpoint";
      endpoint: EndpointConfig;
    }
  | { kind: "createDevice"; device: Device }
  | {
      kind: "apply";
      id: string;
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
  sequence: Sequence;
}
export interface ApiError {
  endpointId?: string;
  unitId?: string;
  relatedUnitId?: string;
  code: string;
  message: string;
  pointId?: string;
  field?: string;
  relatedPoint?: string;
}
export interface DraftInput {
  id: string;
  name: string;
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
export interface SerialDevice { path: string; name: string; }
export interface PartitionRequest {source:string;batchId:string;prefix:string;groupSize:number;startIp:string;port:number;station:number;folder:string|null}
export interface PartitionPreviewResult {canCreate:boolean;localIssues:{ip:string;message:string}[];estimatedMemoryBytes:number;token:string;localIps:string[];preview:{source:string;totalPoints:number;groups:{ip:string;port:number;station:number;name:string;points:number;ranges:[string,number,number][]}[]}}
export type BatchOperation="apply"|"start"|"stop";
export type BatchScope={kind:"all"}|{kind:"selected";ids:string[]}|{kind:"folder";id:string};
export interface BatchRequest {id:string;operation:BatchOperation;scope:BatchScope;concurrency:number}
export interface DraftDisposition {unsaved:string[];retainWrites:string[]}
export interface LifecyclePreview {plan:{id:string;operation:BatchOperation;concurrency:number;targets:{endpoint:string;name:string;role:string;units:number;applied:boolean}[]};items:{endpoint:string;state:string;reason:string|null;changes:[number,number,number]}[]}
export interface BatchView {id:string;operation:BatchOperation;finished:boolean;occupied:number;counts:{pending:number;running:number;uncertain:number;succeeded:number;failed:number;skipped:number;cancelled:number};items:{endpoint:string;state:string;reason:string|null;waitMs:number|null;executionMs:number|null}[]}
export interface ResourceUsage {memoryBytes:number;endpoints:number;connections:number;queueBytes:number}
export interface ResourceView {recording?:{queuedBytes:number;files:number;syncs:number;workers:number;activeFiles:number;failedFiles:number;oldestWaitMs:number}|null;policy:{limit:ResourceUsage;batchTargets:number;previewBytes:number;endpointRecordBytes:number;recordBatchBytes:number;recordWaitMs:number};current:ResourceUsage;reserved:ResourceUsage;reservations:number;sampledMs:string;processRssBytes:number|null;configuredEndpoints:number;scope:string}
export interface WorkspaceClient {
  resolveUnit?(unit:UnitConfig):Promise<CollectionModel>;
  previewUnitPromotion?(endpoint:string,unit:string,selection:OverrideSelection):Promise<PromotionPlan>;
  captureInitials?(endpoint:string,unit:string,points:string[]):Promise<InitialSample[]>;
  exportUnitPoints?(endpoint:string,unit:string,format:string):Promise<boolean>;
  loadModel?(id:string):Promise<CollectionModel>;
  loadEndpointValues?(id:string):Promise<Record<string,UnitValues>>;
  queryResources?():Promise<ResourceView>;
  previewBatch?(request:BatchRequest,drafts:DraftDisposition):Promise<LifecyclePreview>;
  startBatch?(id:string):Promise<void>;
  queryBatch?():Promise<BatchView|null>;
  cancelBatch?(id:string):Promise<void>;
  discardBatch?(id:string):Promise<void>;
  retryBatch?(previous:string,id:string,drafts:DraftDisposition):Promise<LifecyclePreview>;
  previewPartition?(request:PartitionRequest):Promise<PartitionPreviewResult>;
  createPartition?(token:string):Promise<Receipt>;
  discardPartition?(token:string):Promise<void>;
  listSerialPorts?(): Promise<SerialDevice[]>;
  exportObservations?(content:string):Promise<boolean>;
  executeMaster?(command:MasterCommand):Promise<MasterView>;
  queryMaster?(query:MasterQuery):Promise<MasterView>;
  executeSlave?(command:SlaveCommand):Promise<SlaveView>;
  querySlave?(query:SlaveQuery):Promise<SlaveView>;
  query(): Promise<WorkspaceView>;
  execute(command: Command): Promise<Receipt>;
  inspectEndpoint?(endpoint: EndpointConfig): Promise<Record<string,boolean>>;
  previewPoints?(input: DraftInput): Promise<DraftPreview>;
  previewApply?(
    id: string,
    reset: boolean,
  ): Promise<ApplyPreview>;
  openPointFile?(signal?: AbortSignal): Promise<PointFile | null>;
  exportPointTemplate?(format: string): Promise<boolean>;
  exportPointFile?(
    id: string,
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

/** TCP从站实际运行，与配置提交独立。 */
export type SlaveAction =
 | {kind:"start";maxClients:number}
 | {kind:"stop"}
 | {kind:"setValues";unit:string;values:[string,RawValue][]}
 | {kind:"resetValues";unit:string;points:string[]};
export interface SlaveCommand {commandId:string;endpoint:string;action:SlaveAction}
export interface SlaveQuery {endpoint:string;unit:string|null;points:string[]}
export interface SlaveObservation {sequence:string;unixMs:string;event:string}
export interface SlaveView {
 pendingRecording?:boolean;
 endpoint:string;state:"idle"|"starting"|"running"|"stopping"|"stopped"|"failed";
 revision:string;clients:number;maxClients:number;requests:string;exceptions:string;
 lastRequestMicros:string;error:string|null;recordPath:string|null;lastReliable:string;uncertain:boolean;
 observations?:SlaveObservation[];events:string[];values:Record<string,RawValue>;
}

/** 主站命令与配置提交分离，操作接收不等于执行成功。 */
export type MasterAction={kind:"diagnose";unit:string;function:number;sub:number;data:number}|{kind:"setUnitPolling";unit:string;enabled:boolean}|{kind:"connect"|"disconnect"|"startPolling"|"stopPolling"}|{kind:"read";unit:string;points:string[]}|{kind:"write";unit:string;values:[string,RawValue][]};
export interface MasterCommand {commandId:string;endpoint:string;action:MasterAction}
export interface MasterQuery {endpoint:string;unit:string|null;points:string[]}
export interface MasterPoint {value:RawValue|null;unixMs:string|null;quality:string;error:string|null;writeResult:string|null;writeOperation?:string|null}
export interface MasterView {diagnostic?:string|null;endpoint:string;state:string;polling:boolean;pausedUnits:string[];busy:boolean;operation:string|null;completed:number;total:number;requests:number;error:string|null;revision:string;points:Record<string,MasterPoint>;observations:SlaveObservation[];recordPath:string|null}
