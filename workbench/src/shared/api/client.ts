import type { BackendInfo, Communication, RtuConfig, SimulatorSnapshot } from './contracts';
export interface EssimClient {
  getBackendInfo(): Promise<BackendInfo>;
  getSnapshot(): Promise<SimulatorSnapshot>;
  start(address: string, port: number): Promise<SimulatorSnapshot>;
  configure(communication: Communication): Promise<SimulatorSnapshot>;
  startRtu(config: RtuConfig): Promise<SimulatorSnapshot>;
  stop(): Promise<SimulatorSnapshot>;
  addUnit(unitId: number): Promise<SimulatorSnapshot>;
  updateSoc(raw: number, unitId?: number): Promise<SimulatorSnapshot>;
  subscribe(listener: (snapshot: SimulatorSnapshot) => void): Promise<() => Promise<void>>;
}
