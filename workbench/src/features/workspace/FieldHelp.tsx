import { MarkdownTooltip } from "@essim/ui/components/markdown-tooltip";
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
    <MarkdownTooltip markdown={help[field] ?? labels[field] ?? field}>
        <button
          type="button"
          className="field-help"
          aria-label={`${labels[field] ?? field}说明`}
        >
          ?
        </button>
    </MarkdownTooltip>
  );
}
