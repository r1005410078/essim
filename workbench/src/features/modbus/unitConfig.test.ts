import { expect, it } from 'vitest';
import { parseNewUnitId, nextUnitId } from './unitConfig';
it('新增单元先校验整数、范围及重复，且完整占用时不给出候选值', () => {
 for (const value of ['', ' ', '0', '248', '1.5', 'NaN', '-1']) {
   expect(() => parseNewUnitId(value, [])).toThrow();
 }
 expect(() => parseNewUnitId('2', [1, 2])).toThrow('已存在');
 expect(parseNewUnitId('247', [1, 2])).toBe(247);
 expect(nextUnitId([1, 3])).toBe(2);
 expect(nextUnitId(Array.from({length:247}, (_, i) => i + 1))).toBeUndefined();
});
