import {expect,it,vi} from 'vitest';
import {CommandRegistry} from './commands';
import {ResourceSearchIndex,parseQuickQuery} from './resourceSearch';
import type {Catalog} from '../../shared/api/contracts';
it('命令注册拒绝重复，执行时重新检查可用条件，释放后不可执行',async()=>{
 const registry=new CommandRegistry<{enabled:boolean}>();const run=vi.fn();
 const dispose=registry.register({id:'test.run',title:'测试操作',category:'测试',enabled:c=>c.enabled,run});
 expect(()=>registry.register({id:'test.run',title:'重复',run})).toThrow();
 expect(registry.list({enabled:false})[0]?.enabled).toBe(false);
 await expect(registry.execute('test.run',{enabled:false})).rejects.toThrow();expect(run).not.toHaveBeenCalled();
 await registry.execute('test.run',{enabled:true});expect(run).toHaveBeenCalledOnce();dispose();
 await expect(registry.execute('test.run',{enabled:true})).rejects.toThrow();
});
it('资源检索支持中文、拼音、首字母和路径，独立于点值且不丢失同名身份',()=>{
 const catalog:Catalog={models:[{id:'a',name:'电池模型',points:[]},{id:'b',name:'电池模型',points:[]}],endpoints:[],devices:[],applied:[],values:{}};
 const index=new ResourceSearchIndex();index.update(catalog);
 for(const query of ['电池','dianchimo','dcmx','采集模型 电池'])expect(index.search(query).map(x=>x.target.id)).toEqual(['a','b']);
 expect(index.search('modbus')[0]?.target).toEqual({kind:'resource',id:'modbus-guide'});
 index.update({...catalog,models:[{...catalog.models[0]!,name:'逆变器'}]});
 expect(index.search('dcmx')).toEqual([]);expect(index.search('nbq')[0]?.target.id).toBe('a');
});
it('命令前缀不参与资源检索，普通空查询仍属于资源模式',()=>{
 expect(parseQuickQuery('  > 保存 ')).toEqual({mode:'commands',text:'保存'});
 expect(parseQuickQuery(' 电池 ')).toEqual({mode:'resources',text:'电池'});
 expect(parseQuickQuery('')).toEqual({mode:'resources',text:''});
});

it('工作台命令执行使用当前能力，复用业务回调并按标题或ID查找',async()=>{
 const {createWorkbenchCommands}=await import('./workbenchCommands');
 const registry=createWorkbenchCommands();let enabled=true;
 const context={get canCreate(){return enabled;},create:vi.fn(),openTool:vi.fn(),openGuide:vi.fn()};
 expect(registry.list(context,'新建 模型').map(c=>c.id)).toEqual(['workbench.create.model']);
 expect(registry.list(context,'workbench.open.modbusGuide')).toHaveLength(1);
 enabled=false;await expect(registry.execute('workbench.create.model',context)).rejects.toThrow();
 expect(context.create).not.toHaveBeenCalled();
 enabled=true;await registry.execute('workbench.create.model',context);expect(context.create).toHaveBeenCalledWith('model');
 await registry.execute('workbench.open.batch',context);expect(context.openTool).toHaveBeenCalledWith('batch');
 await registry.execute('workbench.open.modbusGuide',context);expect(context.openGuide).toHaveBeenCalledOnce();
});
it('异步业务错误原样返回，不误报执行成功',async()=>{
 const registry=new CommandRegistry<void>();const error=Error('后端断开');
 registry.register({id:'fail',title:'失败操作',run:async()=>{throw error;}});
 await expect(registry.execute('fail',undefined)).rejects.toBe(error);
});
it('拼音匹配保留英文前缀，同名设备与模型身份不混淆，清空目录移除旧项',()=>{
 const catalog:Catalog={models:[{id:'same',name:'PCS电池模型',points:[]}],devices:[{id:'same',name:'PCS电池模型',kind:'custom'}],endpoints:[],applied:[],values:{}};
 const index=new ResourceSearchIndex();index.update(catalog);
 expect(index.search('pcs dianchi').map(x=>x.target.kind)).toEqual(['model','device']);
 expect(index.search('PCS电池')[0]?.target.kind).toBe('model');
 expect(index.search('pcs',1)).toHaveLength(1);
 index.update(undefined);expect(index.search('pcs')).toEqual([]);
 expect(index.search('modbus')).toHaveLength(1);
});

it('保存与布局命令检查能力并复用既有入口',async()=>{
 const {createWorkbenchCommands}=await import('./workbenchCommands');const registry=createWorkbenchCommands();
 const context={canCreate:false,canSave:false,create:vi.fn(),openTool:vi.fn(),openGuide:vi.fn(),requestSave:vi.fn(),toggleLayout:vi.fn()};
 await expect(registry.execute('workbench.save',context)).rejects.toThrow();expect(context.requestSave).not.toHaveBeenCalled();
 context.canSave=true;await registry.execute('workbench.save',context);expect(context.requestSave).toHaveBeenCalledOnce();
 await registry.execute('workbench.layout.right',context);expect(context.toggleLayout).toHaveBeenCalledWith('right');
});
