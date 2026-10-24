import {expect,it} from 'vitest';
import {inspectorScopeForError,inspectorTabs,isInspectorPanel} from './inspectorTabs';

it('端点及Unit校验定位对应范围，传输字段不受关联Unit干扰',()=>{
 expect(inspectorScopeForError({field:'transport.port',unitId:'unit-1'})).toBe('endpoint');
 expect(inspectorScopeForError({field:'name',unitId:'unit-1'})).toBe('unit');
 expect(inspectorScopeForError({field:'units'})).toBe('unit');
 expect(inspectorScopeForError({field:'name'})).toBe('endpoint');
});
it('三个属性标签均不属于文档历史，普通文档不被误判',()=>{
 expect(inspectorTabs.map(tab=>tab.scope)).toEqual(['point','unit','endpoint']);
 for(const tab of inspectorTabs)expect(isInspectorPanel(tab.id)).toBe(true);
 expect(isInspectorPanel('endpoint:inspector-unit')).toBe(false);
});
