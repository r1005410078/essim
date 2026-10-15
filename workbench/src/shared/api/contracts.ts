export interface RtuConfig {
  device: string; baud: number; parity: 'N' | 'E' | 'O'; dataBits: 8; stopBits: 1 | 2;
  slave: number; receiveTimeoutMs: number; byteTimeoutMs: number;
}
export type Communication = { transport: 'tcp'; address: string; port: number } | { transport: 'rtu'; config: RtuConfig };
export interface BackendInfo { name: string; version: string; protocolEngineReady: boolean }
export interface ApiError { code: string; message: string }
export interface UnitSnapshot { unitId: number; socRaw: number; socPercent: number }
export interface SimulatorSnapshot {
  instanceId: string; pointId: string; sequence: string; generation: string;
  state: 'stopped' | 'starting' | 'listening' | 'stopping' | 'failed';
  address: string; port: number; socRaw: number; socPercent: number; connections: number;
  units: UnitSnapshot[];
  communication: Communication;
  rtu: RtuConfig | null;
  error: ApiError | null;
  logs: { direction: string; hex: string; message: string }[];
}
// Decimal strings preserve u64 sequence precision across the IPC boundary.
export function newerSnapshot(current: SimulatorSnapshot | undefined, next: SimulatorSnapshot) {
  if (next.instanceId !== 'demo-bms') return current;
  if (!current) return next;
  const generation = BigInt(next.generation) - BigInt(current.generation);
  return generation > 0n || (generation === 0n && BigInt(next.sequence) > BigInt(current.sequence)) ? next : current;
}
