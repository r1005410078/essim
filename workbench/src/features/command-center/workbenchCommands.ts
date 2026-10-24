import {CommandRegistry} from './commands';
export interface WorkbenchCommandContext {
 canCreate:boolean;
 canSave?:boolean;
 requestSave?:()=>void;
 toggleLayout?:(edge:"left"|"right"|"bottom")=>void;
 create:(kind:'model'|'endpoint'|'device')=>void;
 openTool:(tool:'batch'|'resources')=>void;
 openGuide:()=>void;
}
/** 菜单与未来命令检索共用同一命令，不通过DOM按钮模拟业务操作。 */
export function createWorkbenchCommands(){
 const registry=new CommandRegistry<WorkbenchCommandContext>();
 for(const [kind,title] of [['model','新建采集模型'],['endpoint','新建模拟器'],['device','新建设备']] as const)
  registry.register({id:`workbench.create.${kind}`,title,category:'配置',enabled:c=>c.canCreate,run:c=>c.create(kind)});
 registry.register({id:'workbench.open.batch',title:'打开批量任务',category:'工作台',run:c=>c.openTool('batch')});
 registry.register({id:'workbench.open.resources',title:'打开运行资源',category:'工作台',run:c=>c.openTool('resources')});
 registry.register({id:'workbench.open.modbusGuide',title:'打开 Modbus 指南',category:'帮助',run:c=>c.openGuide()});
 registry.register({id:'workbench.save',title:'保存当前配置',category:'文件',enabled:c=>!!c.canSave&&!!c.requestSave,run:c=>c.requestSave?.()});
 for(const [edge,title] of [['left','切换左侧导航'],['right','切换右侧属性'],['bottom','切换底部面板']] as const)
  registry.register({id:`workbench.layout.${edge}`,title,category:'视图',enabled:c=>!!c.toggleLayout,run:c=>c.toggleLayout?.(edge)});
 return registry;
}
