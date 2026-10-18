import { useState } from "react";
import { createRoot } from "react-dom/client";
import type { CellEditor, CellEditorOpenContext } from "@zhiguang/novasheet-core";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@essim/ui/components/select";

function CellSelect({ ctx }: { ctx: CellEditorOpenContext }) {
  const [open, setOpen] = useState(true);
  const choices = ctx.field.options?.choices;
  const values = Array.isArray(choices)
    ? choices.filter((value): value is string => typeof value === "string" && value !== "")
    : [];
  const original = String(ctx.value ?? "");
  const options = [...new Set(values)];
  // 非法旧值保留在末尾供识别；有效选项始终遵循字段定义的顺序。
  if (original && !options.includes(original)) options.push(original);
  return (
    <Select open={open} value={original} onOpenChange={(next) => {
      setOpen(next);
      if (!next) ctx.cancel();
    }} onValueChange={(value) => {
      if (value === original) ctx.cancel();
      else ctx.commit(value);
    }}>
      <SelectTrigger aria-label={`${ctx.field.name}选项`} className="h-full min-h-0 rounded-none border-primary shadow-none">
        <SelectValue placeholder="请选择" />
      </SelectTrigger>
      <SelectContent onPointerDown={(event) => event.stopPropagation()} onPointerUp={(event) => event.stopPropagation()} onClick={(event) => event.stopPropagation()} onKeyDown={(event) => event.stopPropagation()} onCloseAutoFocus={(event) => event.preventDefault()}>
        {options.map((value) => <SelectItem key={value} value={value}
          onPointerUp={(event) => {
            // 在菜单关闭或焦点恢复前确认用户选项，避免WebView事件顺序吞掉提交。
            event.preventDefault();
            event.stopPropagation();
            ctx.commit(value);
          }}
          onClick={(event) => {
            event.preventDefault();
            event.stopPropagation();
            ctx.commit(value);
          }}
          onKeyDown={(event) => {
            if (event.key !== "Enter" && event.key !== " ") return;
            event.preventDefault();
            event.stopPropagation();
            ctx.commit(value);
          }}
        >{value}{!values.includes(value) ? "（当前非法值）" : ""}</SelectItem>)}
      </SelectContent>
    </Select>
  );
}

/** 在NovaSheet公开编辑器接口内挂载shadcn Select，生命周期由表格管理。 */
export function createSelectEditor(): CellEditor {
  let dispose: (() => void) | undefined;
  const close = () => {
    const cleanup = dispose;
    dispose = undefined;
    cleanup?.();
  };
  return {
    open(ctx) {
      close();
      const host = document.createElement("div");
      Object.assign(host.style, {
        position: "absolute", left: `${ctx.rect.x}px`, top: `${ctx.rect.y}px`,
        width: `${ctx.rect.width}px`, height: `${ctx.rect.height}px`, zIndex: "50",
      });
      host.addEventListener("pointerdown", (event) => event.stopPropagation());
      host.addEventListener("keydown", (event) => event.stopPropagation());
      ctx.container.appendChild(host);
      const root = createRoot(host);
      let active = true;
      dispose = () => {
        active = false;
        host.remove();
        // 提交可能发生在React事件或effect中，延后卸载避免嵌套卸载。
        queueMicrotask(() => root.unmount());
      };
      const finish = (value?: string) => {
        if (!active) return;
        if (value === undefined) {
          // Radix关闭通知可能先于同一事件中的选值；本轮事件结束后再取消。
          queueMicrotask(() => {
            if (!active) return;
            close();
            ctx.cancel();
          });
          return;
        }
        close();
        if (value === String(ctx.value ?? "")) ctx.cancel();
        else ctx.commit(value);
      };
      root.render(<CellSelect ctx={{ ...ctx, commit: (value) => finish(String(value ?? "")), cancel: () => finish() }} />);
    },
    close,
    destroy: close,
  };
}
