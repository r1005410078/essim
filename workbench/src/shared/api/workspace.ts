/** Tauri仅在通信适配层使用；业务组件只依赖WorkspaceClient。 */
import { Channel, invoke as tauriInvoke, isTauri } from "@tauri-apps/api/core";
import type { WorkspaceClient, WorkspaceView } from "./contracts";
import { projectSession } from './project';
export function createWorkspaceClient(): WorkspaceClient {
  const session = projectSession();
  const invoke: typeof tauriInvoke = (command, args, options) => options ? tauriInvoke(command, { ...(args as object), projectSession: session }, options) : tauriInvoke(command, { ...(args as object), projectSession: session });
  if (!isTauri()) {
    const unavailable = async (): Promise<never> => {
      throw {
        code: "BACKEND_UNAVAILABLE",
        message: "浏览器未连接Rust核心，请使用Tauri桌面工作台。",
      };
    };
    return {
      resolveUnit:unavailable,previewUnitPromotion:unavailable,captureInitials:unavailable,exportUnitPoints:unavailable,
      loadModel:unavailable,loadEndpointValues:unavailable,
      queryResources:unavailable,
      previewBatch:unavailable,startBatch:unavailable,queryBatch:unavailable,cancelBatch:unavailable,discardBatch:unavailable,retryBatch:unavailable,
      previewPartition:unavailable,
      createPartition:unavailable,
      discardPartition:unavailable,
      listSerialPorts: unavailable,
      exportObservations:unavailable,
      executeMaster:unavailable,
      queryMaster:unavailable,
      executeSlave:unavailable,
      querySlave:unavailable,
      query: unavailable,
      execute: unavailable,
      subscribe: unavailable,
      previewPoints: unavailable,
      previewApply: unavailable,
      openPointFile: unavailable,
      exportPointTemplate: unavailable,
      exportPointFile: unavailable,
    };
  }
  return {
    resolveUnit:unit=>invoke("resolve_unit",{unit}),
    previewUnitPromotion:(endpoint,unit,selection)=>invoke("preview_unit_promotion",{endpoint,unit,selection}),
    captureInitials:(endpoint,unit,points)=>invoke("capture_initials",{endpoint,unit,points}),
    exportUnitPoints:(endpoint,unit,format)=>invoke("export_unit_points",{endpoint,unit,format}),
    loadModel:id=>invoke("load_model",{id}),
    loadEndpointValues:id=>invoke("load_endpoint_values",{id}),
    queryResources:()=>invoke("query_resources"),
    previewBatch:(request,drafts)=>invoke("preview_batch",{request,drafts}),
    startBatch:id=>invoke("start_batch",{id}),
    queryBatch:()=>invoke("query_batch"),
    cancelBatch:id=>invoke("cancel_batch",{id}),
    discardBatch:id=>invoke("discard_batch",{id}),
    retryBatch:(previous,id,drafts)=>invoke("retry_batch",{previous,id,drafts}),
    previewPartition:request=>invoke("preview_partition",{request}),
    createPartition:token=>invoke("create_partition",{token}),
    discardPartition:token=>invoke("discard_partition",{token}),
    listSerialPorts: () => invoke("list_serial_ports"),
    exportObservations:content=>invoke("export_observations",{content}),
    executeMaster:command=>invoke("execute_master",{command}),
    queryMaster:query=>invoke("query_master",{query}),
    executeSlave:command=>invoke("execute_slave",{command}),
    querySlave:query=>invoke("query_slave",{query}),
    query: () => invoke("query_workspace"),
    execute: (command) => invoke("execute_workspace", { command }),
    inspectEndpoint: endpoint => invoke("inspect_endpoint",{endpoint}),
    previewApply: (id, reset) =>
      invoke("preview_apply", { id, reset }),
    previewPoints: (input) => invoke("preview_point_table", { input }),
    openPointFile: async (signal) => {
      if (signal?.aborted) return null;
      const jobId = crypto.randomUUID();
      const cancel = () => {
        void invoke("cancel_point_file", { jobId }).catch(() => {});
      };
      signal?.addEventListener("abort", cancel, { once: true });
      try {
        const file = await invoke<import("./contracts").PointFile | null>(
          "open_point_file",
          { jobId },
        );
        return signal?.aborted ? null : file;
      } finally {
        signal?.removeEventListener("abort", cancel);
      }
    },
    exportPointTemplate: (format) =>
      invoke("export_point_template", { format }),
    exportPointFile: (id, format) =>
      invoke("export_point_file", { id, format }),
    subscribe: async (listener) => {
      const channel = new Channel<WorkspaceView>();
      channel.onmessage = listener;
      const subscriptionId = await invoke<string>("subscribe_workspace", {
        channel,
      });
      let cancelled = false;
      return async () => {
        if (cancelled) return;
        cancelled = true;
        channel.onmessage = () => {};
        await invoke("unsubscribe_workspace", { subscriptionId });
      };
    },
  };
}
