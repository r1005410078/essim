import { useEffect, useMemo, useState } from 'react';
import { createProjectClient, type ProjectInfo } from '../../shared/api/project';
import { ResourceDocument } from './ResourceDocument';
/** 工程 Markdown 复用现有只读文档渲染，不另建编辑器。 */
export function ProjectResourceDocument({ project, id, anchor, navigation }: { project: ProjectInfo | null; id: string; anchor?: string; navigation?: number }) {
  const client = useMemo(createProjectClient, []);
  const [content, setContent] = useState<string | null>(null), [error, setError] = useState('');
  const resourceId = id.slice('project-document:'.length), doc = project?.documents?.find(d => d.id === resourceId);
  useEffect(() => { let active = true; setContent(null); setError(''); if (project && doc) void client.loadDocument(project.session, doc.id).then(text => { if (active) setContent(text); }).catch(e => { if (active) setError(e?.message ?? String(e)); }); return () => { active = false; }; }, [project?.session, doc?.id, client]);
  if (!doc) return <div className="empty">工程文档不存在。</div>;
  if (error) return <div className="validation-banner" role="alert">{error}</div>;
  if (content === null) return <div className="empty">正在读取文档…</div>;
  return <ResourceDocument id={id} anchor={anchor} navigation={navigation} document={{ id, title: doc.name, folders: ['工程文档'], source: '', content }} />;
}
