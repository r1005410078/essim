import type { Communication, RtuConfig } from '../../shared/api/contracts';
export const defaultRtuConfig: RtuConfig = { device: '', baud: 19200, parity: 'N', dataBits: 8, stopBits: 2, slave: 1, receiveTimeoutMs: 50, byteTimeoutMs: 2 };
export function validateRtuConfig(config: RtuConfig): RtuConfig {
  const { device, baud, parity, dataBits, stopBits, slave, receiveTimeoutMs, byteTimeoutMs } = config;
  if (!device.trim() || device.includes('\0')) throw new Error('串口路径不能为空或含 NUL');
  if (![9600,19200,38400,57600,115200].includes(baud) || !['N','E','O'].includes(parity) || dataBits !== 8 || ![1,2].includes(stopBits)) throw new Error('串口参数不在首版支持范围内');
  if (!Number.isInteger(slave) || slave < 1 || slave > 247) throw new Error('RTU 站号必须为 1–247 的整数');
  if (!Number.isInteger(receiveTimeoutMs) || receiveTimeoutMs < 10 || receiveTimeoutMs > 100 || !Number.isInteger(byteTimeoutMs) || byteTimeoutMs < 1 || byteTimeoutMs > 2) throw new Error('接收超时须为 10–100ms，字节间超时为 1–2ms');
  return {...config};
}
export function sameCommunication(a: Communication, b: Communication | undefined): boolean {
  if (!b || a.transport !== b.transport) return false;
  if (a.transport === 'tcp' && b.transport === 'tcp') return a.address === b.address && a.port === b.port;
  if (a.transport === 'rtu' && b.transport === 'rtu') return (Object.keys(defaultRtuConfig) as (keyof RtuConfig)[]).every(key => a.config[key] === b.config[key]);
  return false;
}
