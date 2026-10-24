import type {EndpointConfig,WorkspaceClient} from '../../shared/api/contracts';
import type {TreeRuntime} from './treeStatus';

/** 当前可见端点的轻量查询，最多4并发；失效后不提交、不继续派发。 */
export async function queryTreeMetadata(client:WorkspaceClient,endpoints:EndpointConfig[],sequence:string,accept:(id:string,value:TreeRuntime)=>void,valid:()=>boolean){
 let index=0;
 await Promise.all(Array.from({length:Math.min(4,endpoints.length)},async()=>{
  while(valid()){
   const endpoint=endpoints[index++];if(!endpoint)return;
   let result:TreeRuntime;
   try{
    const query={endpoint:endpoint.id,unit:null,points:[]};
    if(endpoint.role==='slave'){
     if(!client.querySlave)throw Error('从站状态服务不可用');
     result={sequence,slave:await client.querySlave(query)};
    }else{
     if(!client.queryMaster)throw Error('主站状态服务不可用');
     result={sequence,master:await client.queryMaster(query)};
    }
   }catch(error){result={sequence,error:typeof error==='object'&&error!==null&&'message' in error?String(error.message):String(error)};}
   if(valid())accept(endpoint.id,result);
  }
 }));
}
