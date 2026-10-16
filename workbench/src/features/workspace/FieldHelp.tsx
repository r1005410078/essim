import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from "@essim/ui/components/tooltip";
import content from "./point-fields.md?raw";
import { labels } from "./pointDraft";
const help = Object.fromEntries(
  content
    .split(/^## /m)
    .slice(1)
    .map((section) => {
      const newline = section.indexOf("\n");
      return [section.slice(0, newline).trim(), section.slice(newline + 1)];
    }),
);
/** 表头和Details复用同一个Markdown字段注册表。 */
export function FieldHelp({ field }: { field: string }) {
  return (
    <Tooltip>
      <TooltipTrigger asChild>
        <button
          type="button"
          className="field-help"
          aria-label={`${labels[field] ?? field}说明`}
        >
          ?
        </button>
      </TooltipTrigger>
      <TooltipContent className="markdown-help">
        <ReactMarkdown remarkPlugins={[remarkGfm]} skipHtml>
          {help[field] ?? labels[field] ?? field}
        </ReactMarkdown>
      </TooltipContent>
    </Tooltip>
  );
}
