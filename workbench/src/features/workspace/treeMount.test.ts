import {afterEach,expect,it,vi} from 'vitest';
import {attachTreeEvents} from './treeMount';
afterEach(()=>vi.unstubAllGlobals());
it('attaches immediately to a mounted tree without waiting for a frame',()=>{
 const observe=vi.fn();vi.stubGlobal('MutationObserver',class{observe=observe;disconnect=vi.fn();});
 const host={isConnected:true,shadowRoot:{}} as HTMLElement;const release=vi.fn(),bind=vi.fn(()=>release);
 const dispose=attachTreeEvents(()=>host,bind);expect(bind).toHaveBeenCalledWith(host,host.shadowRoot);expect(observe).not.toHaveBeenCalled();dispose();expect(release).toHaveBeenCalledOnce();
});
it('waits for a connected host once and cancels pending observation on unmount',()=>{
 let changed=()=>{};const disconnect=vi.fn(),observe=vi.fn();vi.stubGlobal('document',{body:{}});
 vi.stubGlobal('MutationObserver',class{constructor(cb:()=>void){changed=cb;}observe=observe;disconnect=disconnect;});
 let host:HTMLElement|undefined;const release=vi.fn(),bind=vi.fn(()=>release);const dispose=attachTreeEvents(()=>host,bind);
 expect(observe).toHaveBeenCalledWith(document.body,expect.objectContaining({attributes:true,attributeFilter:['data-file-tree-virtualized']}));
 expect(bind).not.toHaveBeenCalled();host={isConnected:false} as HTMLElement;changed();expect(bind).not.toHaveBeenCalled();
 host={isConnected:true} as HTMLElement;changed();changed();expect(bind).toHaveBeenCalledOnce();dispose();expect(release).toHaveBeenCalledOnce();
 host=undefined;const cancelledBind=vi.fn(()=>release);const cancel=attachTreeEvents(()=>host,cancelledBind);cancel();host={isConnected:true} as HTMLElement;changed();expect(cancelledBind).not.toHaveBeenCalled();expect(disconnect).toHaveBeenCalled();
});
