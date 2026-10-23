import type { ComponentProps } from "react";
import { SelectItem } from "@essim/ui/components/select";
import { MarkdownTooltip } from "@essim/ui/components/markdown-tooltip";
import { pointSelectOption, pointSelectLabel } from "./pointSelectOptions";

/** 点表枚举选项共用字段对应的展示布局与公共 Markdown 说明。 */
export function PointSelectItem({field, value, children, ...props}: ComponentProps<typeof SelectItem> & {field: string}) {
  const option = pointSelectOption(field, value);
  const item = <SelectItem {...props} value={value} textValue={option ? pointSelectLabel(field, value) : props.textValue}>
    {option && (field === "valueType" || field === "byteOrder" || field === "wordOrder") ? option.id : option ? <span className="point-select-option">
      <span className="point-select-option-title"><span>{option.name}</span><code>{option.id}</code></span>
      <span className="point-select-option-meta">{option.description}</span>
    </span> : children}
  </SelectItem>;
  return option ? <MarkdownTooltip side="right" align="start" markdown={option.markdown} source={option.source}>{item}</MarkdownTooltip> : item;
}
