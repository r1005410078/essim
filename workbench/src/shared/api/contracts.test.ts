import { expect, it } from 'vitest';
import { newerSnapshot, type SimulatorSnapshot } from './contracts';
const snapshot = (sequence: string, generation = '1') => ({ instanceId: 'demo-bms', sequence, generation } as SimulatorSnapshot);
it('忽略旧序号、旧代次及其他实例，保留超过 JS 数值精度的序号', () => {
 const current = snapshot('9007199254740993');
 expect(newerSnapshot(current, snapshot('9007199254740992'))).toBe(current);
 expect(newerSnapshot(current, snapshot('9007199254740994'))).not.toBe(current);
 expect(newerSnapshot(current, snapshot('99999999999999999', '0'))).toBe(current);
 expect(newerSnapshot(current, snapshot('1', '2'))?.generation).toBe('2');
 expect(newerSnapshot(current, { ...snapshot('999'), instanceId: 'other' })).toBe(current);
});
