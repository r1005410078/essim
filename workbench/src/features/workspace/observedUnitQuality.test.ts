import {expect,it} from 'vitest';
import {WorkspaceStore} from './model';
import type {WorkspaceClient,WorkspaceView} from '../../shared/api/contracts';
it('质量只记录已有查询观测，空查询不伪造健康，相同结果不重复通知',()=>{
 const store=new WorkspaceStore({} as WorkspaceClient);store.view={sequence:'1'} as WorkspaceView;
 store.recordUnitQuality('u',{p:{quality:'stale'}});
 expect(store.observedUnitQuality.get('u')).toEqual({sequence:'1',quality:'stale'});
 const version=store.snapshot();store.recordUnitQuality('u',{});store.recordUnitQuality('u',{p:{quality:'stale'}});
 expect(store.snapshot()).toBe(version);
 store.recordUnitQuality('u',{p:{quality:'failed'}});expect(store.observedUnitQuality.get('u')?.quality).toBe('failed');
 store.view={sequence:'2'} as WorkspaceView;store.recordUnitQuality('u',{p:{quality:'valid'}});
 expect(store.observedUnitQuality.get('u')).toEqual({sequence:'2',quality:undefined});
});
