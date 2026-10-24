/** 树库在挂载时同步创建 Shadow DOM；后台 WebView 不应依赖动画帧才接通事件。 */
export function attachTreeEvents(
  getHost:()=>HTMLElement|undefined,
  bind:(host:HTMLElement,root:HTMLElement|ShadowRoot)=>()=>void,
){
 let disposed=false,bound=false;
 let release:()=>void=()=>{};
 let observer:MutationObserver|undefined;
 const attach=()=>{
  if(disposed||bound)return;
  const host=getHost();
  if(!host?.isConnected)return;
  bound=true;
  observer?.disconnect();
  release=bind(host,host.shadowRoot??host);
 };
 attach();
 if(!bound){
  observer=new MutationObserver(attach);
  // React 包装先插入宿主，再在 layout effect 中给它标记挂载；Shadow DOM 内部变更不会穿透 body。
  observer.observe(document.body,{childList:true,subtree:true,attributes:true,attributeFilter:['data-file-tree-virtualized']});
  attach();
 }
 return()=>{disposed=true;observer?.disconnect();release();};
}
