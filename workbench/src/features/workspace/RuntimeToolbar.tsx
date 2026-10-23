import type { ReactNode } from "react";
import { Tooltip, TooltipContent, TooltipTrigger } from "@essim/ui/components/tooltip";

/** 端点上下文由页面提供，协议控制器保留动作和状态的所有权。 */
export interface RuntimeToolbarSlots {
  start?: ReactNode;
  capture?: ReactNode;
  model?: ReactNode;
  end?: ReactNode;
}

export function RuntimeToolbar({ label, slots, actions, state, metrics, settings }: {
  label: string;
  slots?: RuntimeToolbarSlots;
  actions: ReactNode;
  state: ReactNode;
  metrics: ReactNode;
  settings?: ReactNode;
}) {
  return <div className="toolbar compact-point-toolbar runtime-toolbar" role="toolbar" aria-label={label}>
    {slots?.start}<i className="toolbar-divider"/>{actions}{slots?.capture}{state}
    {slots?.model}<span className="spacer"/>{metrics}{settings}{slots?.end}
  </div>;
}

/** 状态仍可直接阅读，补充说明通过 hover 和键盘聚焦展开。 */
export function RuntimeHint({ label, children, className = "" }: { label: string; children: ReactNode; className?: string }) {
  return <Tooltip><TooltipTrigger asChild><span tabIndex={0} aria-label={label} className={`runtime-hint ${className}`}>{children}</span></TooltipTrigger>
    <TooltipContent side="bottom" sideOffset={5}>{label}</TooltipContent></Tooltip>;
}

export function ConfigurationStatus({ dirty, applied, hasSnapshot, master, modelDirty = false }: { dirty: boolean; applied: boolean; hasSnapshot: boolean; master: boolean; modelDirty?: boolean }) {
  const state = dirty ? "未保存" : applied ? "已应用" : hasSnapshot ? "待应用" : "未应用";
  const hint = (applied && !dirty ? "当前配置已应用。运行使用已应用快照。" :
    `${dirty ? "当前配置有未保存修改" : "当前配置尚未应用"}；${master ? "连接和采集" : "启动"}使用已应用快照。${hasSnapshot ? "请保存并应用后使修改生效。" : "首次运行前，请先保存并应用配置。"}`) + (modelDirty ? " 引用模型有未保存修改，请先保存模型；当前显示已保存点表。" : "");
  return <RuntimeHint label={hint} className={`configuration-badge ${dirty || !applied || modelDirty ? "pending" : ""}`}>{state}</RuntimeHint>;
}

/** 错误与未确认结果始终在工具栏下方明确显示，不收进状态提示。 */
export function RuntimeError({ error, runtimeError, uncertain = false }: { error?: string | null; runtimeError?: string | null; uncertain?: boolean }) {
  const message = [...new Set([error, runtimeError].filter(Boolean))].join("；");
  if (!message && !uncertain) return null;
  return <div className="panel-note error" role="alert">{message}{uncertain ? `${message ? "；" : ""}存在未确认结果，请勿据此重复写入。` : ""}</div>;
}
