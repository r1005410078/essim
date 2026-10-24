import {expect,it,vi} from 'vitest';
// 有意检查锁定版本的控制器边界；不将私有控制器接入产品代码。
const controllerUrl=new URL('../../../node_modules/@pierre/trees/dist/model/FileTreeController.js',import.meta.url);
const {FileTreeController}=await import(controllerUrl.href);
const paths=['a/one','a/two','b/three'];
it('installed tree retains toggle/range selection and selected drag sources',()=>{
 const controller=new FileTreeController({paths,initialExpansion:'open',dragAndDrop:{canDrop:()=>false}});
 controller.selectOnlyPath('a/one');controller.togglePathSelectionFromInput('a/two');
 expect(controller.getSelectedPaths()).toEqual(['a/one','a/two']);
 controller.startDrag('a/one');expect(controller.getDragSession().draggedPaths).toEqual(['a/one','a/two']);
 controller.cancelDrag();controller.togglePathSelectionFromInput('a/two');expect(controller.getSelectedPaths()).toEqual(['a/one']);
 controller.selectOnlyPath('a/one');controller.selectPathRange('a/two',false);expect(controller.getSelectedPaths()).toEqual(['a/one','a/two']);
 controller.startDrag('b/three');expect(controller.getDragSession().draggedPaths).toEqual(['b/three']);
 expect(controller.getSelectedPaths()).toEqual(['b/three']);
});
it('canDrop veto prevents the installed library from changing paths before the core transaction',()=>{
 const complete=vi.fn();const controller=new FileTreeController({paths,initialExpansion:'open',dragAndDrop:{canDrop:()=>false,onDropComplete:complete}});
 controller.selectOnlyPath('a/one');controller.startDrag('a/one');
 controller.setDragTarget({kind:'directory',directoryPath:'b/',hoveredPath:'b/',flattenedSegmentPath:null});
 expect(controller.completeDrag()).toBe(false);expect(controller.getItem('a/one')).not.toBeNull();expect(controller.getItem('b/one')).toBeNull();expect(complete).not.toHaveBeenCalled();
 controller.selectOnlyPath('a/');controller.selectPath('a/one');controller.startDrag('a/');expect(controller.getDragSession().draggedPaths).toEqual(['a/']);
 controller.cancelDrag();expect(controller.getDragSession()).toBeNull();
});
