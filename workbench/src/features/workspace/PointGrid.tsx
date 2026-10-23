import { useEffect, useRef, type RefObject } from "react";
import {
  Grid,
  serializeRowsToTsv,
  type GridSelection,
} from "@zhiguang/novasheet-core";
import { canvas2dBackend } from "@zhiguang/novasheet-canvas2d";
import { DraftSource } from "./DraftSource";
import { widths } from "./pointDraft";
import { readonlyTypes, sheetTheme } from "./sheets";
import { pastePointText } from "./pointClipboard";
import { formatPointOption } from "./pointSelectOptions";
import { createSelectEditor } from "./selectEditor";

/** NovaSheet React 0.4.14未转发isCellEditable，故使用同一组件的公开Grid入口。
 * 这里仅组装DOM生命周期；绘制、编辑器、粘贴、填充及滚动仍由NovaSheet处理。
 */
export function PointGrid({
  source,
  api,
  onSelection,
  onError,
}: {
  source: DraftSource;
  api: RefObject<Grid | null>;
  onSelection: (selection: GridSelection) => void;
  onError: (error: string) => void;
}) {
  const host = useRef<HTMLDivElement>(null);
  const keyboard = useRef<HTMLTextAreaElement | null>(null);
  const selection = useRef(onSelection);
  selection.current = onSelection;
  useEffect(() => {
    if (!host.current) return;
    const grid = new Grid(host.current, {
      data: source,
      theme: sheetTheme,
      cellTypes: readonlyTypes,
      formatters: { pointOption: formatPointOption },
      cellEditors: { singleSelect: createSelectEditor() },
      backend: canvas2dBackend(),
      excelHeaders: true,
      rowHeaderCornerLabel: "",
      interactions: { contextMenu: false, reorder: false },
      isCellEditable: (cell) => {
        const underlying =
          api.current
            ?.getViewPipeline()
            .getComposed()
            .resolveUnderlyingRow?.(cell.rowIndex) ?? cell.rowIndex;
        const type = String(source.getCell(underlying, "valueType") ?? "u16");
        return !(
          (type === "bool" &&
            ["byteOrder", "wordOrder", "scale"].includes(cell.fieldId)) ||
          ((widths[type] ?? 1) === 1 && cell.fieldId === "wordOrder")
        );
      },
      onSelectionChange: (value) => selection.current(value),
    });
    api.current = grid;
    // WKWebView只对可编辑的原生焦点目标派发系统paste命令。
    // 输入代理放在网格当前焦点容器内，键盘导航仍沿原有事件路径到NovaSheet。
    const proxy = document.createElement("textarea");
    proxy.setAttribute("aria-label", "点表键盘输入");
    proxy.dataset.pointKeyboard = "true";
    proxy.tabIndex = -1;
    proxy.value = " ";
    Object.assign(proxy.style, {
      position: "absolute",
      left: "0",
      top: "0",
      width: "1px",
      height: "1px",
      opacity: "0",
      pointerEvents: "none",
    });
    keyboard.current = proxy;
    const focusKeyboard = (event: PointerEvent) => {
      const target = event.target as HTMLElement;
      if (target.closest("input,textarea,select,button,[contenteditable=true]"))
        return;
      const focused = document.activeElement;
      if (
        focused instanceof HTMLElement &&
        host.current?.contains(focused) &&
        !focused.matches("input,textarea,select,[contenteditable=true]")
      ) {
        focused.appendChild(proxy);
        proxy.focus({ preventScroll: true });
        proxy.select();
      }
    };
    host.current.addEventListener("pointerup", focusKeyboard);
    return () => {
      host.current?.removeEventListener("pointerup", focusKeyboard);
      proxy.remove();
      keyboard.current = null;
      api.current = null;
      grid.destroy();
    };
  }, [source, api]);
  return (
    <div
      ref={host}
      className="h-full w-full"
      onKeyDownCapture={(event) => {
        const target = event.target as HTMLElement;
        if (
          target !== keyboard.current &&
          target.closest("input,textarea,[contenteditable=true]")
        )
          return;
        if (
          (event.metaKey || event.ctrlKey) &&
          ["c", "x", "v"].includes(event.key.toLowerCase())
        )
          event.stopPropagation();
      }}
      onPasteCapture={(event) => {
        const target = event.target as HTMLElement;
        if (
          target !== keyboard.current &&
          target.closest("input,textarea,[contenteditable=true]")
        )
          return;
        const grid = api.current;
        if (!grid) return;
        event.preventDefault();
        event.stopPropagation();
        const text = event.clipboardData.getData("text/plain");
        if (!text) return;
        void pastePointText(
          source,
          () => grid.getViewPipeline().getComposed(),
          grid.getSelection(),
          text,
          (r, c) => grid.getCellType(r, c),
        )
          .then((skipped) => {
            if (skipped) onError(`已跳过${skipped}个不适用或只读单元格`);
          })
          .catch((e) => onError(String(e.message ?? e)));
      }}
      onCopyCapture={(event) => {
        if (event.target !== keyboard.current) return;
        const grid = api.current;
        const range = grid?.getSelection().selectedRange;
        if (!grid || !range) return;
        const view = grid.getViewPipeline().getComposed();
        const rows = view.getRows(range.startRow, range.endRow);
        if (!Array.isArray(rows)) return;
        event.preventDefault();
        event.stopPropagation();
        const ids = view
          .getSchema()
          .fields.slice(range.startCol, range.endCol + 1)
          .map((f) => f.id);
        event.clipboardData.setData(
          "text/plain",
          serializeRowsToTsv(rows, ids, view.getSchema()),
        );
      }}
      onCutCapture={(event) => {
        if (event.target !== keyboard.current) return;
        const grid = api.current;
        const range = grid?.getSelection().selectedRange;
        if (!grid || !range) return;
        const view = grid.getViewPipeline().getComposed();
        const rows = view.getRows(range.startRow, range.endRow);
        if (!Array.isArray(rows)) return;
        event.preventDefault();
        event.stopPropagation();
        const ids = view
          .getSchema()
          .fields.slice(range.startCol, range.endCol + 1)
          .map((f) => f.id);
        event.clipboardData.setData(
          "text/plain",
          serializeRowsToTsv(rows, ids, view.getSchema()),
        );
        void pastePointText(
          source,
          () => grid.getViewPipeline().getComposed(),
          grid.getSelection(),
          "",
        ).catch((e) => onError(String(e.message ?? e)));
      }}
      onCompositionEnd={(event) => {
        if (event.target !== keyboard.current || !event.data) return;
        const grid = api.current;
        const active = grid?.getSelection().activeCell;
        if (!grid || !active) return;
        keyboard.current!.value = " ";
        keyboard.current!.select();
        const selection = {
          activeCell: active,
          anchorCell: active,
          extentCell: active,
          selectedRange: {
            startRow: active.rowIndex,
            endRow: active.rowIndex,
            startCol: active.colIndex,
            endCol: active.colIndex,
          },
        };
        void pastePointText(
          source,
          () => grid.getViewPipeline().getComposed(),
          selection,
          event.data,
        ).catch((e) => onError(String(e.message ?? e)));
      }}
    />
  );
}
