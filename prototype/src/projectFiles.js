export function folderDescendants(folders, id) {
  const ids = new Set([id]);
  let changed = true;
  while (changed) {
    changed = false;
    for (const folder of folders) if (ids.has(folder.parentId) && !ids.has(folder.id)) {
      ids.add(folder.id); changed = true;
    }
  }
  return ids;
}
export function exportProjectFile(project, kind, id) {
  const item = (kind === 'template' ? project.templates : project.devices).find(x => x.id === id);
  if (!item) throw new Error('文件不存在');
  if (kind === 'template') return { format: 'essim.modbus-model', version: 1, model: item };
  const refs = new Set(item.comms.map(c => c.templateId));
  return { format: 'essim.modbus-simulator', version: 1, simulator: item,
    models: project.templates.filter(t => refs.has(t.id)) };
}

export function withDefaultFolders(project) {
  // 已有目录结构（包括用户清空后的结构）保持原样。
  if (Array.isArray(project.folders)) return project;
  return {
    ...project,
    folders: [
      { id: 'folder-models', name: '点表模型', parentId: '' },
      { id: 'folder-simulators', name: '模拟器', parentId: '' },
    ],
    templates: project.templates.map(item => ({ ...item, folderId: 'folder-models' })),
    devices: project.devices.map(item => ({ ...item, folderId: 'folder-simulators' })),
  };
}
