import {expect,it} from 'vitest';
import {treeStatuses,type TreeRuntime} from './treeStatus';
import type {EndpointConfig,SlaveView,MasterView} from '../../shared/api/contracts';
const endpoint={id:'e',name:'PCS',role:'slave',transport:{kind:'tcp'},units:[{id:'u',enabled:true}]} as EndpointConfig;
const slave=(state:SlaveView['state']):TreeRuntime=>({sequence:'1',slave:{state,clients:2,error:null} as SlaveView});
const masterView=(pausedUnits:string[]=[]):MasterView=>({endpoint:'e',state:'connected',polling:true,pausedUnits,busy:false,operation:null,completed:0,total:0,requests:0,error:null,revision:'1',points:{},observations:[],recordPath:null});
it('监听、待应用和未保存并列，从站Unit不推断在线',()=>{
 expect(treeStatuses({endpoint,runtime:slave('running'),sequence:'1',pending:true,dirty:true}).map(x=>x.kind)).toEqual(['listening','pending','dirty']);
 expect(treeStatuses({endpoint,unit:endpoint.units[0],runtime:slave('running'),sequence:'1'})).toEqual([]);
});
it('故障保留真实原因，查询失败或旧序号不沿用绿色',()=>{
 expect(treeStatuses({endpoint,runtime:{sequence:'1',slave:{state:'failed',error:'地址被占用'} as SlaveView},sequence:'1'})[0]?.description).toContain('地址被占用');
 expect(treeStatuses({endpoint,runtime:slave('running'),sequence:'2'})[0]?.kind).toBe('unknown');
 expect(treeStatuses({endpoint,runtime:{sequence:'1',error:'服务不可用'},sequence:'1'})[0]?.description).toContain('服务不可用');
});
it('主站Unit暂停、禁用、已观察过期与采集状态区分',()=>{
 const master={...endpoint,role:'master'} as EndpointConfig;
 const runtime:TreeRuntime={sequence:'1',master:masterView(['u'])};
 const input={endpoint:master,unit:master.units[0],runtime,sequence:'1'};
 expect(treeStatuses(input)[0]?.kind).toBe('paused');
 expect(treeStatuses({...input,unit:{...master.units[0]!,enabled:false}})[0]?.kind).toBe('disabled');
 expect(treeStatuses({...input,quality:'stale'})[0]?.kind).toBe('stale');
 runtime.master!.pausedUnits=[];expect(treeStatuses(input)[0]?.kind).toBe('polling');
 runtime.master!.state='reconnecting';expect(treeStatuses({...input,unit:undefined})[0]?.kind).toBe('reconnecting');
});

it('新增或未应用Unit不借父端点采集状态显示采集中',()=>{
 const master={...endpoint,role:'master'} as EndpointConfig;
 expect(treeStatuses({endpoint:master,unit:master.units[0],runtime:{sequence:'1',master:masterView()},sequence:'1',pollingEnabled:false})).toEqual([]);
});

it('RTU应答与TCP监听语义区分，启动和停止过渡均不显示运行正常',()=>{
 const serial={...endpoint,transport:{kind:'rtu',path:'/dev/test'}} as EndpointConfig;
 expect(treeStatuses({endpoint:serial,runtime:slave('running'),sequence:'1'})[0]?.kind).toBe('serial');
 for(const state of ['starting','stopping'] as const)expect(treeStatuses({endpoint,runtime:slave(state),sequence:'1'})[0]?.kind).toBe('connecting');
});
