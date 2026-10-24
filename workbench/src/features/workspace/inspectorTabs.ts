export type InspectorScope = 'point' | 'unit' | 'endpoint';
export const inspectorTabs = [
  {id:'inspector-point',scope:'point',title:'点'},
  {id:'inspector-unit',scope:'unit',title:'Unit'},
  {id:'inspector-endpoint',scope:'endpoint',title:'端点'},
] as const;
export const isInspectorPanel = (id:string) => inspectorTabs.some(tab=>tab.id===id);
/** 传输字段属于端点，其余带Unit身份或units字段的校验定位单元。 */
export function inspectorScopeForError(error:{field?:string;unitId?:string|null}):InspectorScope {
  if(error.field?.startsWith('transport'))return 'endpoint';
  return error.unitId || error.field==='units' || error.field?.startsWith('unit.') ? 'unit' : 'endpoint';
}
