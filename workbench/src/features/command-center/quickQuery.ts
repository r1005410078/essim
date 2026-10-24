export function parseQuickQuery(query:string){const text=query.trim();return text.startsWith('>')?{mode:'commands' as const,text:text.slice(1).trim()}:{mode:'resources' as const,text};}
