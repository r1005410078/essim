import { expect, it, vi, beforeEach } from "vitest";
const { invoke, isTauri } = vi.hoisted(() => ({
  invoke: vi.fn(),
  isTauri: vi.fn(() => true),
}));
vi.mock("@tauri-apps/api/core", () => ({
  invoke,
  isTauri,
  Channel: class {
    onmessage: unknown;
  },
}));
import { createWorkspaceClient } from "./workspace";
beforeEach(() => {
  invoke.mockReset();
  isTauri.mockReturnValue(true);
});
it("无后端时查询、命令和订阅全部明确失败", async () => {
  isTauri.mockReturnValue(false);
  const client = createWorkspaceClient();
  await expect(client.query()).rejects.toMatchObject({
    code: "BACKEND_UNAVAILABLE",
  });
  await expect(
    client.execute({
      commandId: "c",
      action: { kind: "start", id: "endpoint" },
    }),
  ).rejects.toMatchObject({ code: "BACKEND_UNAVAILABLE" });
  await expect(client.subscribe(() => {})).rejects.toMatchObject({
    code: "BACKEND_UNAVAILABLE",
  });
  expect(invoke).not.toHaveBeenCalled();
});
it("稳定对象与字符串版本原样传递，后端拒绝不转为成功", async () => {
  const client = createWorkspaceClient();
  invoke.mockRejectedValueOnce({
    code: "CAPABILITY_UNAVAILABLE",
    message: "未接入",
  });
  const command = {
    commandId: "command-a",
    action: { kind: "start" as const, id: "endpoint-a" },
  };
  await expect(client.execute(command)).rejects.toMatchObject({
    code: "CAPABILITY_UNAVAILABLE",
  });
  expect(invoke).toHaveBeenCalledWith("execute_workspace", { command });
});
it("订阅取消幂等，停止向已关闭面板交付更新", async () => {
  const client = createWorkspaceClient();
  invoke.mockResolvedValueOnce("42");
  const listener = vi.fn();
  const cancel = await client.subscribe(listener);
  const channel = invoke.mock.calls[0]![1].channel;
  await cancel();
  await cancel();
  channel.onmessage({ sequence: "99" });
  expect(listener).not.toHaveBeenCalled();
  expect(
    invoke.mock.calls.filter((call) => call[0] === "unsubscribe_workspace"),
  ).toHaveLength(1);
});
it("点表预览保留64位文本和字段错误，导出指向明确保存版本", async () => {
  const client = createWorkspaceClient();
  const input = {
    id: "m",
    name: "M",
    version: "9007199254740993",
    points: [{ id: "p", initialEngineering: "18446744073709551615" }],
  };
  const error = {
    code: "INVALID_FIELD",
    message: "非法",
    pointId: "p",
    field: "initialEngineering",
  };
  invoke.mockResolvedValueOnce({ model: null, errors: [error], previews: [] });
  expect((await client.previewPoints!(input)).errors[0]).toEqual(error);
  expect(invoke).toHaveBeenCalledWith("preview_point_table", { input });
  invoke.mockResolvedValueOnce(false);
  expect(await client.exportPointFile!("m", "9007199254740993", "xlsx")).toBe(
    false,
  );
  expect(invoke).toHaveBeenLastCalledWith("export_point_file", {
    id: "m",
    version: "9007199254740993",
    format: "xlsx",
  });
});
it("文件取消发送任务令牌并丢弃后台迟到结果", async () => {
  let complete!: (value: unknown) => void;
  invoke.mockImplementation((command) =>
    command === "open_point_file"
      ? new Promise((resolve) => {
          complete = resolve;
        })
      : Promise.resolve(),
  );
  const client = createWorkspaceClient();
  const controller = new AbortController();
  const task = client.openPointFile!(controller.signal);
  controller.abort();
  const jobId = invoke.mock.calls.find(
    (call) => call[0] === "open_point_file",
  )![1].jobId;
  expect(invoke).toHaveBeenCalledWith("cancel_point_file", { jobId });
  complete({ name: "late.csv", sheets: [] });
  expect(await task).toBeNull();
});
