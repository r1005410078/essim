import {useEffect,useMemo,useRef,type ReactNode} from "react";
import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";
import {BookOpen} from "lucide-react";
import {markdownHeadings,resources,type DocumentResource} from "./catalog";

/** 内置Markdown只读渲染，滚动定位限于当前Dockview面板，不改变应用URL。 */
export function ResourceDocument({id,anchor,navigation,document}:{id:string;anchor?:string;navigation?:number;document?:DocumentResource}){
 const resource=document??resources.find(resource=>resource.id===id);const article=useRef<HTMLElement>(null);
 const headings=useMemo(()=>markdownHeadings(resource?.content??""),[resource]);
 const scroll=(section:string)=>{const target=article.current?.querySelector<HTMLElement>(`[id="${CSS.escape(section)}"]`);if(target){target.scrollIntoView({block:"start"});target.focus({preventScroll:true});}};
 useEffect(()=>{if(anchor){const frame=requestAnimationFrame(()=>scroll(anchor));return()=>cancelAnimationFrame(frame);}},[id,anchor,navigation]);
 if(!resource)return <div className="empty">文档资源不存在。</div>;
 const Heading=({line,children,level}:{line?:number;children?:ReactNode;level:1|2|3})=>{
  const entry=headings.find(heading=>heading.line===line);const Tag=`h${level}` as "h1"|"h2"|"h3";
  return <Tag tabIndex={-1} id={entry?.id} data-section-heading>{children}</Tag>;
 };
 return <div className="panel column resource-document">
  <div className="resource-document-bar"><BookOpen size={15}/><span>文档资源 / {resource.folders.join(" / ")}</span><span className="spacer"/><span>{document?"工程 · 只读":"内置 · 只读"}</span></div>
  <div className="resource-document-layout">
   <nav className="resource-document-toc" aria-label="文档章节目录"><strong>目录</strong>{headings.filter(heading=>heading.level===2).map(heading=><button key={heading.id} onClick={()=>scroll(heading.id)}>{heading.text}</button>)}</nav>
   <article ref={article} className="resource-markdown" aria-label={resource.title}>
    <ReactMarkdown remarkPlugins={[remarkGfm]} skipHtml components={{
     h1:({node,children})=><Heading level={1} line={node?.position?.start.line}>{children}</Heading>,
     h2:({node,children})=><Heading level={2} line={node?.position?.start.line}>{children}</Heading>,
     h3:({node,children})=><Heading level={3} line={node?.position?.start.line}>{children}</Heading>,
     a:({href,children})=>href?.startsWith("#")?<a href={href} onClick={event=>{event.preventDefault();scroll(decodeURIComponent(href.slice(1)));}}>{children}</a>:href&&/^https?:\/\//i.test(href)?<a href={href} target="_blank" rel="noopener noreferrer">{children}</a>:<span>{children}</span>,
    }}>{resource.content}</ReactMarkdown>
    {resource.source&&<div className="resource-source">资源文件：{resource.source}</div>}
   </article>
  </div>
 </div>;
}
