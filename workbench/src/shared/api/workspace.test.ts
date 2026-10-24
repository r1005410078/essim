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
import { bindProjectSession } from "./project";
import { createWorkspaceClient } from "./workspace";
beforeEach(() => {
  invoke.mockReset();
  bindProjectSession("session-A");
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
it("稳定对象原样传递，后端拒绝不转为成功", async () => {
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
  expect(invoke).toHaveBeenCalledWith("execute_workspace", { command, projectSession:"session-A" });
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
it("点表预览保留64位文本和字段错误，导出当前已保存模型", async () => {
  const client = createWorkspaceClient();
  const input = {
    id: "m",
    name: "M",
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
  expect(invoke).toHaveBeenCalledWith("preview_point_table", { input, projectSession:"session-A" });
  invoke.mockResolvedValueOnce(false);
  expect(await client.exportPointFile!("m", "xlsx")).toBe(
    false,
  );
  expect(invoke).toHaveBeenLastCalledWith("export_point_file", {
    id: "m",
    format: "xlsx",
    projectSession:"session-A",
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
  expect(invoke).toHaveBeenCalledWith("cancel_point_file", { jobId, projectSession:"session-A" });
  complete({ name: "late.csv", sheets: [] });
  expect(await task).toBeNull();
});
it("串口发现仅调用查询，保留路径和设备名称", async () => {
  const ports = [{path:"/dev/cu.test",name:"RS485 · USB 1234:5678"}];
  invoke.mockResolvedValueOnce(ports);
  await expect(createWorkspaceClient().listSerialPorts!()).resolves.toEqual(ports);
  expect(invoke).toHaveBeenCalledWith("list_serial_ports",{projectSession:"session-A"});
});
it("浏览器串口发现明确失败，不伪造设备", async () => {
  isTauri.mockReturnValue(false);
  await expect(createWorkspaceClient().listSerialPorts!()).rejects.toMatchObject({code:"BACKEND_UNAVAILABLE"});
  expect(invoke).not.toHaveBeenCalled();
});

it("分组规划、提交与释放经统一适配传输，失败保留后端原因",async()=>{
 const client=createWorkspaceClient();
 const request={source:"m",batchId:"b",prefix:"组",groupSize:1000,startIp:"127.0.0.1",port:1080,station:1,folder:null};
 invoke.mockRejectedValueOnce({code:"IP_NOT_LOCAL",message:"未配置"});
 await expect(client.previewPartition!(request)).rejects.toMatchObject({code:"IP_NOT_LOCAL"});
 expect(invoke).toHaveBeenLastCalledWith("preview_partition",{request,projectSession:"session-A"});
 invoke.mockResolvedValueOnce({});await client.createPartition!("b");expect(invoke).toHaveBeenLastCalledWith("create_partition",{token:"b",projectSession:"session-A"});
 invoke.mockResolvedValueOnce(undefined);await client.discardPartition!("b");expect(invoke).toHaveBeenLastCalledWith("discard_partition",{token:"b",projectSession:"session-A"});
});

it("旧客户端捕获原工程凭证，不能在全局切换后伪装为新请求",async()=>{
 const old=createWorkspaceClient();bindProjectSession("session-B");const next=createWorkspaceClient();
 invoke.mockResolvedValue({});await old.query();expect(invoke).toHaveBeenLastCalledWith("query_workspace",{projectSession:"session-A"});
 await next.query();expect(invoke).toHaveBeenLastCalledWith("query_workspace",{projectSession:"session-B"});
});
