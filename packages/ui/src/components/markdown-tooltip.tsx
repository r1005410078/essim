import { useState, type ComponentProps, type ReactElement, type ReactNode } from "react";
import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";
import { Tooltip, TooltipContent, TooltipProvider, TooltipTrigger } from "./tooltip";

type Placement = Pick<ComponentProps<typeof TooltipContent>, "side" | "align">;
export interface MarkdownTooltipProps extends Placement {
  children: ReactElement;
  markdown: string;
  source?: ReactNode;
}

/** 公共 Markdown 正文与可选来源栏；原始 HTML 不参与渲染。 */
export function MarkdownTooltipBody({ markdown, source }: Pick<MarkdownTooltipProps, "markdown" | "source">) {
  return <div className="markdown-tooltip-scroll">
    <ReactMarkdown remarkPlugins={[remarkGfm]} skipHtml>{markdown}</ReactMarkdown>
    {source && <div className="markdown-tooltip-source">{source}</div>}
  </div>;
}

/** 工程说明浮层：统一排版、边界避让、来源与焦点/悬停交互。 */
export function MarkdownTooltip({ children, markdown, source, side = "top", align = "center" }: MarkdownTooltipProps) {
  const [container, setContainer] = useState<HTMLElement | undefined>();
  const [narrow, setNarrow] = useState(false);
  const prepare = (element: HTMLElement) => {
    setContainer(element.closest("dialog") ?? undefined);
    setNarrow(window.innerWidth < 640);
  };
  return <TooltipProvider delayDuration={350}>
    <Tooltip>
      <TooltipTrigger asChild onPointerEnter={event => prepare(event.currentTarget)} onFocus={event => prepare(event.currentTarget)}>
        {children}
      </TooltipTrigger>
      <TooltipContent portalContainer={container} side={narrow ? "bottom" : side} align={align} sideOffset={8} collisionPadding={8} className="markdown-tooltip">
        <MarkdownTooltipBody markdown={markdown} source={source}/>
      </TooltipContent>
    </Tooltip>
  </TooltipProvider>;
}
