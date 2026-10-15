import { expect, it } from 'vitest';
import { createSocSheet, draftSoc } from './socSheet';
it('NovaSheet 草稿与已生效值分离，应用前拒绝非法内容', () => {
 const data = createSocSheet();
 expect(data.getRows(0, 0)[0]?.id).toBe('soc');
 data.updateCell(0, 'value', 700); expect(draftSoc(data)).toBe(700);
 expect(data.getCell(0, 'applied')).toBeNull();
 for (const invalid of [null, '', '700', 650.5, -1, 1001]) {
   data.updateCell(0, 'value', invalid); expect(() => draftSoc(data)).toThrow();
 }
});

it('切换 Unit ID 保留各自草稿，相同生效值也不会串用表格', async () => {
 const { UnitSocSheets } = await import('./socSheet');
 const sheets = new UnitSocSheets();
 sheets.sync({ unitId: 1, socRaw: 650, socPercent: 65 });
 sheets.sync({ unitId: 2, socRaw: 650, socPercent: 65 });
 sheets.get(1).updateCell(0, 'value', 700);
 expect(draftSoc(sheets.get(2))).toBe(650);
 sheets.get(2).updateCell(0, 'value', 800);
 sheets.sync({ unitId: 1, socRaw: 650, socPercent: 65 }); // 通讯日志快照不覆盖草稿
 expect(draftSoc(sheets.get(1))).toBe(700);
 expect(draftSoc(sheets.get(2))).toBe(800);
 sheets.sync({ unitId: 2, socRaw: 720, socPercent: 72 });
 expect(draftSoc(sheets.get(2))).toBe(720);
 expect(sheets.get(2).getCell(0, 'applied')).toBe(720);
 expect(draftSoc(sheets.get(1))).toBe(700);
});
it('同一站号的 TCP 与 RTU 草稿和生效值相互独立', async () => {
 const {UnitSocSheets}=await import('./socSheet'); const sheets=new UnitSocSheets();
 sheets.sync({unitId:1,socRaw:650,socPercent:65},'tcp');
 sheets.sync({unitId:1,socRaw:650,socPercent:65},'rtu');
 sheets.get(1,'tcp').updateCell(0,'value',700);
 expect(draftSoc(sheets.get(1,'rtu'))).toBe(650);
 sheets.sync({unitId:1,socRaw:810,socPercent:81},'rtu');
 expect(draftSoc(sheets.get(1,'tcp'))).toBe(700);
});
