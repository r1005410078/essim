import { expect, it } from 'vitest';
import { defaultRtuConfig, validateRtuConfig, sameCommunication } from './rtuConfig';
it('RTU 参数整组校验，拒绝无法保证停机边界的等待参数', () => {
 const valid={...defaultRtuConfig,device:'/dev/serial-test'};
 expect(validateRtuConfig(valid)).toEqual(valid);
 for (const change of [{device:''},{device:'a\0b'},{baud:12345},{slave:0},{slave:248},{slave:1.5},{receiveTimeoutMs:0},{receiveTimeoutMs:101},{byteTimeoutMs:3},{parity:'X'},{dataBits:7},{stopBits:3}]) {
   expect(() => validateRtuConfig({...valid,...change} as typeof valid)).toThrow();
 }
 expect(sameCommunication({transport:'rtu',config:valid},{transport:'tcp',address:'127.0.0.1',port:1502})).toBe(false);
 expect(sameCommunication({transport:'rtu',config:valid},{transport:'rtu',config:{...valid,slave:2}})).toBe(false);
 expect(sameCommunication({transport:'rtu',config:valid},{transport:'rtu',config:{...valid}})).toBe(true);
});
