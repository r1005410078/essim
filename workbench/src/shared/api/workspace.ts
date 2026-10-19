/** Tauri仅在通信适配层使用；业务组件只依赖WorkspaceClient。 */
import { Channel, invoke, isTauri } from "@tauri-apps/api/core";
import type { WorkspaceClient, WorkspaceView } from "./contracts";
export function createWorkspaceClient(): WorkspaceClient {
  if (!isTauri()) {
    const unavailable = async (): Promise<never> => {
      throw {
        code: "BACKEND_UNAVAILABLE",
        message: "浏览器未连接Rust核心，请使用Tauri桌面工作台。",
      };
    };
    return {
      exportObservations:unavailable,
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
    exportObservations:content=>invoke("export_observations",{content}),
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
