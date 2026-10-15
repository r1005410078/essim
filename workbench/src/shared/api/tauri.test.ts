import { expect, it, vi } from 'vitest';
const invoke = vi.hoisted(() => vi.fn());
vi.mock('@tauri-apps/api/core', () => ({ invoke, Channel: class { onmessage: unknown } }));
import { createTauriClient } from './tauri';
it('Tauri 适配器转发核心信息，并保留调用失败', async () => {
  const client = createTauriClient();
  invoke.mockResolvedValueOnce({ name: 'ESSim', version: '0.1.0', protocolEngineReady: false });
  await expect(client.getBackendInfo()).resolves.toMatchObject({ protocolEngineReady: false });
  expect(invoke).toHaveBeenCalledWith('get_backend_info');
  const error = { code: 'CORE_UNAVAILABLE', message: '核心不可用' };
  invoke.mockRejectedValueOnce(error);
  await expect(client.getBackendInfo()).rejects.toEqual(error);
});

it('模拟器命令使用稳定 ID，订阅取消仅执行一次', async () => {
 const client=createTauriClient(); invoke.mockResolvedValueOnce({}); await client.updateSoc(700);
 expect(invoke).toHaveBeenLastCalledWith('update_simulator_soc',{instanceId:'demo-bms',pointId:'soc',raw:700,unitId:1});
 invoke.mockResolvedValueOnce('42'); const cancel=await client.subscribe(()=>{});
 invoke.mockResolvedValueOnce(undefined); await cancel(); await cancel();
 expect(invoke).toHaveBeenLastCalledWith('unsubscribe_simulator',{subscriptionId:'42'});
 expect(invoke.mock.calls.filter(call=>call[0]==='unsubscribe_simulator')).toHaveLength(1);
});

it('添加单元和更新值均携带目标 Unit ID', async () => {
 const client = createTauriClient();
 invoke.mockResolvedValueOnce({}); await client.addUnit(2);
 expect(invoke).toHaveBeenLastCalledWith('add_simulator_unit', { instanceId: 'demo-bms', unitId: 2 });
 invoke.mockResolvedValueOnce({}); await client.updateSoc(700, 2);
 expect(invoke).toHaveBeenLastCalledWith('update_simulator_soc', { instanceId: 'demo-bms', pointId: 'soc', raw: 700, unitId: 2 });
});

it('RTU 配置与启动携带完整串口参数，失败直接交由调用者处理', async () => {
 const client=createTauriClient();
 const config={device:'/dev/test',baud:19200,parity:'N' as const,dataBits:8 as const,stopBits:2 as const,slave:7,receiveTimeoutMs:50,byteTimeoutMs:2};
 invoke.mockResolvedValueOnce({}); await client.configure({transport:'rtu',config});
 expect(invoke).toHaveBeenLastCalledWith('configure_simulator',{instanceId:'demo-bms',communication:{transport:'rtu',config}});
 invoke.mockRejectedValueOnce({code:'START_FAILED',message:'串口不可用'});
 await expect(client.startRtu(config)).rejects.toMatchObject({code:'START_FAILED'});
 expect(invoke).toHaveBeenLastCalledWith('start_rtu_simulator',{instanceId:'demo-bms',config});
});
