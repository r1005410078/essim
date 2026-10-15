import { Channel, invoke } from '@tauri-apps/api/core';
import type { EssimClient } from './client';
import type { BackendInfo, SimulatorSnapshot } from './contracts';
const instanceId = 'demo-bms';
export function createTauriClient(): EssimClient {
  return {
    getBackendInfo: () => invoke<BackendInfo>('get_backend_info'),
    getSnapshot: () => invoke('get_simulator_snapshot', { instanceId }),
    start: (address, port) => invoke('start_simulator', { instanceId, address, port }),
    configure: communication => invoke('configure_simulator', { instanceId, communication }),
    startRtu: config => invoke('start_rtu_simulator', { instanceId, config }),
    stop: () => invoke('stop_simulator', { instanceId }),
    addUnit: unitId => invoke('add_simulator_unit', { instanceId, unitId }),
    updateSoc: (raw, unitId = 1) => invoke('update_simulator_soc', { instanceId, pointId: 'soc', raw, unitId }),
    subscribe: async listener => {
      const channel = new Channel<SimulatorSnapshot>();
      channel.onmessage = listener;
      const subscriptionId = await invoke<string>('subscribe_simulator', { instanceId, channel });
      let cancelled = false;
      return async () => {
        if (cancelled) return;
        cancelled = true;
        channel.onmessage = () => {};
        await invoke('unsubscribe_simulator', { subscriptionId });
      };
    },
  };
}
