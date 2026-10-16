import type { CellEditor } from "@zhiguang/novasheet-core";

/** 为NovaSheet公开编辑器注册表提供原生下拉，不复制表格引擎。 */
export function createSelectEditor(): CellEditor {
  let current: HTMLSelectElement | null = null;
  const close = () => {
    const node = current;
    current = null;
    node?.remove();
  };
  return {
    open(ctx) {
      close();
      const select = document.createElement("select");
      current = select;
      select.setAttribute("aria-label", `${ctx.field.name}选项`);
      const choices = ctx.field.options?.choices;
      const values = Array.isArray(choices)
        ? choices.filter((value): value is string => typeof value === "string")
        : [];
      for (const value of values) {
        const option = document.createElement("option");
        option.value = value;
        option.textContent = value;
        select.appendChild(option);
      }
      const original = String(ctx.value ?? "");
      if (original && !values.includes(original)) {
        const option = document.createElement("option");
        option.value = original;
        option.textContent = `${original}（当前非法值）`;
        select.prepend(option);
      }
      select.value = original || values[0] || "";
      Object.assign(select.style, {
        position: "absolute",
        left: `${ctx.rect.x}px`,
        top: `${ctx.rect.y}px`,
        width: `${ctx.rect.width}px`,
        height: `${ctx.rect.height}px`,
        zIndex: "50",
        background: "white",
        border: "1px solid #0875ff",
        font: "inherit",
        color: "#283650",
      });
      const commit = () => {
        if (current !== select) return;
        if (select.value === original) ctx.cancel();
        else ctx.commit(select.value);
      };
      select.addEventListener("change", commit);
      select.addEventListener("keydown", (event) => {
        event.stopPropagation();
        if (event.key === "Escape") {
          event.preventDefault();
          ctx.cancel();
        } else if (event.key === "Enter") {
          // WebKit先派发Enter，再提交原生选项；保留默认行为，等change更新value。
          setTimeout(commit, 0);
        }
      });
      select.addEventListener("pointerdown", (event) =>
        event.stopPropagation(),
      );
      select.addEventListener("blur", () => {
        // 原生菜单关闭时blur可能先于change；不能用旧值提前取消。
        setTimeout(commit, 0);
      });
      ctx.container.appendChild(select);
      select.focus();
      if (typeof select.showPicker === "function")
        try {
          select.showPicker();
        } catch {
          /* 部分WebView需用户点击原生箭头。 */
        }
    },
    close,
    destroy: close,
  };
}
