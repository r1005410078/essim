import { forwardRef, useCallback, useEffect, useId, useImperativeHandle, useLayoutEffect, useRef, useState, type DialogHTMLAttributes, type HTMLAttributes, type PointerEvent, type ReactNode } from 'react';
import { GripHorizontal, Maximize2, PanelsTopLeft, X } from 'lucide-react';
import { constrainDialog, resizeDialog, type DialogEdge, type DialogRect } from '../lib/dialog-geometry';

type Props = Omit<DialogHTMLAttributes<HTMLDialogElement>, 'title' | 'open'> & {
  title: ReactNode;
  busy?: boolean;
  initialWidth?: number;
  variant?: "standard" | "command";
};
const viewport = () => ({ width: document.documentElement.clientWidth, height: window.innerHeight });
function activeControl(): HTMLElement | null {
  if (typeof document === 'undefined') return null;
  let element = document.activeElement;
  while (element?.shadowRoot?.activeElement) element = element.shadowRoot.activeElement;
  return element instanceof HTMLElement ? element : null;
}
const edges: DialogEdge[] = ['n', 's', 'e', 'w', 'ne', 'nw', 'se', 'sw'];
/** 紧凑工作台弹框。原生 modal 保留焦点限制、背景 inert 与表单语义。 */
export const WorkbenchDialog = forwardRef<HTMLDialogElement, Props>(function WorkbenchDialog({ title, busy = false, initialWidth = 472, variant = "standard", children, className = '', onCancel, onKeyDown, onPointerDown, onPointerUp, onPointerCancel, style, ...props }, forwardedRef) {
  const ref = useRef<HTMLDialogElement>(null);
  // 在子输入框autoFocus提交之前记住触发元素，不能等showModal的effect才读取。
  const initialFocus = useRef<HTMLElement | null>(activeControl());
  useImperativeHandle(forwardedRef, () => ref.current!, []);
  const titleId = useId();
  const [rect, setRect] = useState<DialogRect | null>(null);
  const manual = useRef(false);
  const outsidePointer = useRef<number | null>(null);
  const fitRef = useRef<() => void>(() => {});
  const interaction = useRef<{ pointer: number; x: number; y: number; rect: DialogRect; edge?: DialogEdge } | null>(null);
  const fit = useCallback(() => {
    const dialog = ref.current;
    if (!dialog) return;
    const view = viewport();
    const surface = dialog.querySelector<HTMLElement>('.workbench-dialog-surface')!;
    // 临时解除高度约束测量自然内容，允许动态预览出现、收起及复位。
    const oldHeight = dialog.style.height, oldWidth = dialog.style.width;
    dialog.style.height = 'auto';
    dialog.style.width = `${Math.min(initialWidth, view.width - 16)}px`;
    const height = surface.scrollHeight + 2;
    dialog.style.height = oldHeight;
    dialog.style.width = oldWidth;
    const fitted = constrainDialog({ x: 0, y: 0, width: initialWidth, height }, view);
    setRect({ ...fitted, x: Math.max(8, (view.width - fitted.width) / 2), y: variant === "command" ? Math.min(44, Math.max(8, view.height - fitted.height - 8)) : Math.max(8, (view.height - fitted.height) / 2) });
  }, [initialWidth, variant]);
  fitRef.current = fit;
  const restore = () => { manual.current = false; fit(); };
  useLayoutEffect(() => { if (!manual.current) fit(); }, [fit]);
  useEffect(() => {
    const dialog = ref.current!;
    const previous = initialFocus.current;
    if (!dialog.open) dialog.showModal();
    const target = dialog.querySelector<HTMLElement>('[autofocus], .workbench-dialog-body input:not(:disabled), .workbench-dialog-body textarea:not(:disabled), .workbench-dialog-body button:not(:disabled), .workbench-dialog-footer button:not(:disabled)');
    target?.focus();
    const frame = requestAnimationFrame(() => {
      if (!manual.current) fitRef.current();
    });
    const resize = () => { if (!manual.current) fitRef.current(); else setRect(current => current && constrainDialog(current, viewport())); };
    window.addEventListener('resize', resize);
    window.visualViewport?.addEventListener('resize', resize);
    return () => {
      cancelAnimationFrame(frame);
      window.removeEventListener('resize', resize);
      window.visualViewport?.removeEventListener('resize', resize);
      interaction.current = null;
      dialog.close();
      if (previous?.isConnected) previous.focus();
    };
  }, []);
  // 子组件内部状态改变（如导入预览）也会触发自然高度重算；手动缩放后保持用户尺寸。
  useEffect(() => {
    const body = ref.current?.querySelector('.workbench-dialog-body');
    if (!body) return;
    const observer = new MutationObserver(() => { if (!manual.current) fit(); });
    observer.observe(body, { childList: true, subtree: true, characterData: true, attributes: true, attributeFilter: ['open'] });
    return () => observer.disconnect();
  }, [fit]);
  const begin = (event: PointerEvent<HTMLElement>, edge?: DialogEdge) => {
    if (event.button !== 0 || (!edge && (event.target as HTMLElement).closest('button:not(.workbench-dialog-move), input, select, textarea'))) return;
    const bounds = ref.current!.getBoundingClientRect();
    interaction.current = { pointer: event.pointerId, x: event.clientX, y: event.clientY, rect: { x: bounds.x, y: bounds.y, width: bounds.width, height: bounds.height }, edge };
    manual.current = true;
    event.currentTarget.setPointerCapture(event.pointerId);
    event.preventDefault();
  };
  const move = (event: PointerEvent<HTMLElement>) => {
    const drag = interaction.current;
    if (!drag || drag.pointer !== event.pointerId) return;
    const dx = event.clientX - drag.x, dy = event.clientY - drag.y;
    setRect(drag.edge ? resizeDialog(drag.rect, drag.edge, dx, dy, viewport()) : constrainDialog({ ...drag.rect, x: drag.rect.x + dx, y: drag.rect.y + dy }, viewport()));
  };
  const end = () => { interaction.current = null; };
  const keyboard = (event: React.KeyboardEvent, edge?: DialogEdge) => {
    if (event.key === 'Home') { event.preventDefault(); restore(); return; }
    const delta = { ArrowLeft: [-8, 0], ArrowRight: [8, 0], ArrowUp: [0, -8], ArrowDown: [0, 8] }[event.key];
    if (!delta || !rect) return;
    event.preventDefault(); manual.current = true;
    setRect(edge ? resizeDialog(rect, edge, delta[0]!, delta[1]!, viewport()) : constrainDialog({ ...rect, x: rect.x + delta[0]!, y: rect.y + delta[1]! }, viewport()));
  };
  const trapFocus = (event: React.KeyboardEvent<HTMLDialogElement>) => {
    onKeyDown?.(event);
    if (event.defaultPrevented || event.key !== 'Tab') return;
    const controls = [...event.currentTarget.querySelectorAll<HTMLElement>('button, input, select, textarea, a[href], [tabindex]')].filter(element => element.tabIndex >= 0 && !element.matches(':disabled, [hidden]') && element.getClientRects().length > 0 && getComputedStyle(element).visibility !== 'hidden');
    const first = controls[0], last = controls.at(-1);
    // WebKit 原生 dialog 在末尾 Tab 可能短暂聚焦 document；显式封住首尾。
    if (event.shiftKey && document.activeElement === first) { event.preventDefault(); last?.focus(); }
    else if (!event.shiftKey && document.activeElement === last) { event.preventDefault(); first?.focus(); }
  };
  const cancel = () => { if (!busy) ref.current?.dispatchEvent(new Event('cancel', { cancelable: true })); };
  const isBackdrop = (event: PointerEvent<HTMLDialogElement>) => {
    if (event.target !== event.currentTarget) return false;
    const bounds = event.currentTarget.getBoundingClientRect();
    return event.clientX < bounds.left || event.clientX > bounds.right || event.clientY < bounds.top || event.clientY > bounds.bottom;
  };
  return <dialog {...props} onPointerDown={event => {
    onPointerDown?.(event);
    outsidePointer.current = !event.defaultPrevented && variant === 'command' && !busy && event.button === 0 && isBackdrop(event) ? event.pointerId : null;
  }} onPointerUp={event => {
    onPointerUp?.(event);
    const dismiss = !event.defaultPrevented && outsidePointer.current === event.pointerId && isBackdrop(event);
    outsidePointer.current = null;
    // 按下与释放都在外部才关闭，避免从内部选择文字或缩放时误关。
    if (dismiss) cancel();
  }} onPointerCancel={event => { outsidePointer.current = null; onPointerCancel?.(event); }} onKeyDown={trapFocus} ref={ref} aria-labelledby={props['aria-label'] ? undefined : titleId} aria-busy={busy || undefined} className={`workbench-dialog ${variant === "command" ? "workbench-dialog-command" : ""} ${className}`} style={{ ...style, width: Math.min(initialWidth, typeof window === 'undefined' ? initialWidth : viewport().width - 16), ...rect && { left: rect.x, top: rect.y, width: rect.width, height: rect.height } }} onCancel={event => { event.preventDefault(); event.stopPropagation(); if (!busy) onCancel?.(event); }}>
    <div className="workbench-dialog-surface">
      <header className="workbench-dialog-header" onPointerDown={event => begin(event)} onPointerMove={move} onPointerUp={end} onPointerCancel={end} onLostPointerCapture={end} onDoubleClick={event => { if (!(event.target as HTMLElement).closest('button')) restore(); }}>
        <PanelsTopLeft size={14} aria-hidden="true" className="workbench-dialog-icon" />
        <h2 id={titleId}>{title}</h2>
        <button type="button" className="workbench-dialog-move" aria-label="移动弹框（方向键移动，Home 复位）" onKeyDown={event => keyboard(event)}><GripHorizontal size={15} /></button>
        <button type="button" aria-label="恢复弹框位置和大小" title="恢复位置和大小" onClick={restore}><Maximize2 size={12} /></button>
        <button type="button" aria-label="关闭弹框" title="关闭" disabled={busy} onClick={cancel}><X size={15} /></button>
      </header>
      {children}
    </div>
    {edges.map(edge => <div key={edge} data-edge={edge} className={`workbench-dialog-resize workbench-dialog-resize-${edge}`} onPointerDown={event => begin(event, edge)} onPointerMove={move} onPointerUp={end} onPointerCancel={end} onLostPointerCapture={end} />)}
    <button type="button" className="workbench-dialog-size-grip" aria-label="调整弹框大小（方向键调整，Home 复位）" onPointerDown={event => begin(event, 'se')} onPointerMove={move} onPointerUp={end} onPointerCancel={end} onLostPointerCapture={end} onKeyDown={event => keyboard(event, 'se')} />
  </dialog>;
});
export function DialogBody({ className = '', ...props }: HTMLAttributes<HTMLDivElement>) { return <div {...props} className={`workbench-dialog-body ${className}`} />; }
export function DialogFooter({ className = '', ...props }: HTMLAttributes<HTMLDivElement>) { return <div {...props} className={`workbench-dialog-footer ${className}`} />; }
