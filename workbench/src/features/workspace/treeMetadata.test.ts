import {expect,it,vi} from 'vitest';
import {queryTreeMetadata} from './treeMetadata';
import type {EndpointConfig,WorkspaceClient,SlaveView} from '../../shared/api/contracts';
it('只请求空点列表元数据，端点去重由可见层完成，错误不冒充运行失败',async()=>{
 const querySlave=vi.fn().mockResolvedValue({state:'running'});
 const queryMaster=vi.fn().mockRejectedValue(Error('状态服务断开'));
 const accept=vi.fn();
 await queryTreeMetadata({querySlave,queryMaster} as unknown as WorkspaceClient,[{id:'s',role:'slave'},{id:'m',role:'master'}] as EndpointConfig[],'12',accept,()=>true);
 expect(querySlave).toHaveBeenCalledWith({endpoint:'s',unit:null,points:[]});
 expect(queryMaster).toHaveBeenCalledWith({endpoint:'m',unit:null,points:[]});
 expect(accept).toHaveBeenCalledWith('m',{sequence:'12',error:'状态服务断开'});
});
it('最多4并发，失效时忽略在途响应并停止派发',async()=>{
 let active=true;const resolve:((value:SlaveView)=>void)[]=[];
 const querySlave=vi.fn(()=>new Promise<SlaveView>(done=>resolve.push(done)));
 const accept=vi.fn();
 const work=queryTreeMetadata({querySlave} as unknown as WorkspaceClient,Array.from({length:10},(_,i)=>({id:String(i),role:'slave'})) as EndpointConfig[],'1',accept,()=>active);
 expect(querySlave).toHaveBeenCalledTimes(4);
 active=false;for(const done of resolve)done({state:'running'} as SlaveView);
 await work;expect(accept).not.toHaveBeenCalled();expect(querySlave).toHaveBeenCalledTimes(4);
});
