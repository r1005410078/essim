import test from 'node:test';
import assert from 'node:assert/strict';
import { folderDescendants, exportProjectFile } from './projectFiles.js';
test('移动目录时排除自身和所有子目录', () => {
  assert.deepEqual([...folderDescendants([{id:'a'}, {id:'b',parentId:'a'}, {id:'c',parentId:'b'}, {id:'d'}], 'a')], ['a','b','c']);
});
test('模拟器导出只携带所引用模型，不携带运行状态', () => {
  const p = {devices:[{id:'s',comms:[{templateId:'m'}]}],templates:[{id:'m'},{id:'other'}],runtime:{s:123}};
  const file = exportProjectFile(p, 'device', 's');
  assert.equal(file.version, 1);
  assert.deepEqual(file.models, [{id:'m'}]);
  assert.equal(file.runtime, undefined);
});

test('首次分类及旧项目迁移保留文件 ID 和引用，已有目录不重新分类', async () => {
  const {withDefaultFolders} = await import('./projectFiles.js');
  const p = {templates:[{id:'m'}],devices:[{id:'s',comms:[{templateId:'m'}]}]};
  const next = withDefaultFolders(p);
  assert.deepEqual(next.folders.map(f => f.name), ['点表模型','模拟器']);
  assert.equal(next.templates[0].folderId, 'folder-models');
  assert.equal(next.devices[0].folderId, 'folder-simulators');
  assert.equal(next.devices[0].comms[0].templateId, 'm');
  assert.equal(withDefaultFolders(next), next);
  const empty = {...p,folders:[]};
  assert.equal(withDefaultFolders(empty), empty);
});
